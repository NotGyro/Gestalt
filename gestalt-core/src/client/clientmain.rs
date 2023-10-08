//! This needs a refactor more than life iteself.

use std::{
	error::Error,
	fs::OpenOptions,
	io::{BufReader, Read, Write},
	time::{Duration, Instant},
};

use glam::Vec3;
use image::RgbaImage;
use log::{error, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use winit::{
	dpi::PhysicalPosition,
	event::{DeviceEvent, ElementState, VirtualKeyCode},
	event_loop::ControlFlow,
	window::{CursorGrabMode, Fullscreen},
};

use crate::{
	client::{client_config::ClientConfig, render::{Renderer, drawable::{BillboardDrawable, BillboardStyle}, voxel_art::{VoxelArt, CubeArt, CubeTex}, voxel_mesher::make_mesh_completely}},
	common::{
		identity::{IdentityKeyPair, NodeIdentity},
		voxelmath::{VoxelPos, VoxelRange, VoxelRaycast, VoxelSide, SidesArray}, DegreeAngle, Color,
	},
	message::{self, MessageSender},
	message_types::{
		voxel::{VoxelChangeAnnounce, VoxelChangeRequest},
		JoinDefaultEntry,
	},
	net::net_channels::{net_recv_channel::NetMsgReceiver, net_send_channel, NetSendChannel},
	resource::{ResourceKind, image::ID_MISSING_TEXTURE},
	world::{
		chunk::ChunkInner, fsworldstorage::WorldDefaults,
		/*tilespace::{TileSpace, TileSpaceError}, fsworldstorage::{path_local_worlds, WorldDefaults, self, StoredWorldRole},*/
		voxelstorage::VoxelSpace, ChunkPos, TilePos, WorldId, TickLength, tilespace::{TileSpace, TileSpaceError},
	}, entity::{EntityPos, EntityVec3, EntityRot, EntityScale, EntityVelocity, tick_movement_system, LastPos},
};
use crate::{
	//client::render::CubeArt,
	resource::{
		image::{InternalImage},
		update_global_resource_metadata, ResourceId, ResourceInfo, ResourcePoll,
	},
	world::{
		chunk::{Chunk, CHUNK_SIZE},
		TileId, VoxelStorage, VoxelStorageBounded,
	},
};

use super::camera::{self, Camera};

pub const WINDOW_TITLE: &str = "Gestalt";
pub const CLIENT_CONFIG_FILENAME: &str = "client_config.ron";

// Core / main part of the game client. Windowing and event dispatching lives here.
// Input events come in through here.
// Very important that input does not live on the same thread as any heavy compute tasks!
// We need to still be able to read input when the weird stuff is happening.

// Loads images for the purposes of testing in development.
pub struct DevImageLoader {
	pub(crate) images: HashMap<ResourceId, RgbaImage>,
	pub(crate) metadata: HashMap<ResourceId, ResourceInfo>,
}

impl ImageProvider for DevImageLoader {
	fn load_image(
		&mut self,
		image: &ResourceId,
	) -> ResourcePoll<&InternalImage, RetrieveImageError> {
		match self.images.get(image) {
			Some(v) => ResourcePoll::Ready(v),
			None => {
				ResourcePoll::Errored(RetrieveImageError::DoesNotExist(resource_debug!(image)))
			}
		}
	}

	fn get_metadata(&self, image: &ResourceId) -> Option<&ResourceInfo> {
		self.metadata.get(image)
	}
}

impl DevImageLoader {
	pub fn new() -> Self {
		Self {
			images: HashMap::default(),
			metadata: HashMap::default(),
		}
	}

	//A simple function for the purposes of testing in development
	fn preload_image_file(
		&mut self,
		filename: &str,
		creator_identity: IdentityKeyPair,
	) -> Result<ResourceId, Box<dyn Error>> {
		let mut open_options = std::fs::OpenOptions::new();
		open_options.read(true).create(false);

		let mut file = open_options.open(filename)?;
		let mut buf: Vec<u8> = Vec::default();
		let _len = file.read_to_end(&mut buf)?;

		let rid = ResourceId::from_buf(buf.as_slice());

		let image = image::load_from_memory(buf.as_slice())?;

		rid.verify(buf.as_slice())?;

		info!("Loaded {filename} as resource ID {rid}");

		let metadata = ResourceInfo {
			id: rid,
			filename: filename.to_string(),
			creator: creator_identity.public,
			resource_type: "image/png".to_string(),
			authors: "Gyro".to_string(),
			description: Some("Image for early testing purposes.".to_string()),
			kind: ResourceKind::PlainOldData,
			signature: creator_identity.sign(&buf)?,
		};
		update_global_resource_metadata(&rid, metadata.clone());

		self.images.insert(rid, image.into_rgba8());
		self.metadata.insert(rid, metadata);

		Ok(rid)
	}
}
impl Default for DevImageLoader {
	fn default() -> Self {
		Self::new()
	}
}

#[derive(thiserror::Error, Debug)]
pub enum StartClientError {
	#[error("Could not read client config file, i/o error: {0:?}")]
	CouldntOpenConfig(#[from] std::io::Error),
	#[error("Could not parse server config file due to: {0}")]
	CouldntParseConfig(#[from] ron::error::SpannedError),
	#[error("Could not initialize display: {0:?}")]
	CreateWindowError(#[from] winit::error::OsError),
}

// Dirt simple worldgen for the sake of early testing / development
pub fn gen_test_chunk(chunk_position: ChunkPos) -> Chunk<TileId> {
	const AIR_ID: TileId = 0;
	const STONE_ID: TileId = 1;
	const DIRT_ID: TileId = 2;
	const GRASS_ID: TileId = 3;

	match chunk_position.y {
		value if value > -1 => Chunk {
			revision: 0,
			tiles: ChunkInner::Uniform(AIR_ID),
		},
		-1 => {
			let mut chunk = Chunk::new(STONE_ID);
			for pos in chunk.get_bounds() {
				if pos.y == (CHUNK_SIZE as u8 - 1) {
					chunk.set(pos, GRASS_ID).unwrap();
				} else if pos.y > (CHUNK_SIZE as u8 - 4) {
					chunk.set(pos, DIRT_ID).unwrap();
				}
				//Otherwise it stays stone.
			}
			chunk
		},
		_ => {
			/* chunk_position.y is less than zero */
			Chunk {
				revision: 0,
				tiles: ChunkInner::Uniform(STONE_ID),
			}
		}
	}
}

pub fn click_voxel(world_space: &TileSpace, camera: &Camera, ignore: &[TileId], max_steps: u32) -> Result<(TilePos, TileId, VoxelSide), TileSpaceError> {
	let mut raycast = VoxelRaycast::new(*camera.get_position(), *camera.get_front());
	for _i in 0..max_steps {
		let resl = world_space.get(raycast.pos)?;
		if !ignore.contains(resl) {
			return Ok((raycast.pos, *resl, raycast.hit_side()));
		}
		raycast.step();
	}
	todo!()
}

/*
pub fn get_lobby_world_id(pubkey: &NodeIdentity) -> WorldId {

	// Figure out lobby world ID
	let world_defaults_path = path_local_worlds().join("world_defaults.ron");

	let lobby_world_id: Uuid = match world_defaults_path.exists() {
		true => {
			let mut defaults_file = OpenOptions::new()
				.read(true)
				.create(false)
				.open(&world_defaults_path).unwrap();

			let mut buf_reader = BufReader::new(&mut defaults_file);
			let mut contents = String::new();
			buf_reader.read_to_string(&mut contents).unwrap();
			let mut world_defaults: WorldDefaults = ron::from_str(contents.as_str()).unwrap();
			drop(buf_reader);
			drop(defaults_file);
			drop(contents);
			match world_defaults.lobby_world_id {
				Some(uuid) => uuid,
				None => {
					let uuid = Uuid::new_v4();
					world_defaults.lobby_world_id = Some(uuid);
					let cfg_string = ron::ser::to_string_pretty(&world_defaults, ron::ser::PrettyConfig::default() ).unwrap();

					let mut defaults_file = OpenOptions::new()
						.write(true)
						.truncate(true)
						.open(&world_defaults_path).unwrap();
					defaults_file.write_all(cfg_string.as_bytes()).unwrap();
					defaults_file.flush().unwrap();
					drop(defaults_file);
					uuid
				},
			}
		},
		false => {
			let world_uuid = Uuid::new_v4();
			let world_defaults = WorldDefaults {
				lobby_world_id: Some(world_uuid),
			};
			let cfg_string = ron::ser::to_string_pretty(&world_defaults, ron::ser::PrettyConfig::default() ).unwrap();
			let mut output_file = OpenOptions::new()
				.create(true)
				.write(true)
				.truncate(true)
				.open(world_defaults_path).unwrap();

			output_file.write_all(cfg_string.as_bytes()).unwrap();
			output_file.flush().unwrap();

			world_uuid
		},
	};
	WorldId {
		uuid: lobby_world_id,
		host: pubkey.clone(),
	}
}

pub fn load_or_generate_dev_world(world: &mut TileSpace, world_id: &WorldId, chunk_range: VoxelRange<i32>, mut terrain_notify: Option<&mut TerrainRenderer>) -> Result<(), Box<dyn Error>> {
	let worldgen_start = Instant::now();
	// Build chunks and then immediately let the mesher know they're new.
	for chunk_position in chunk_range {
		let chunk_file_path = fsworldstorage::path_for_chunk(&world_id, StoredWorldRole::Local, &chunk_position);
		let chunk = if chunk_file_path.exists() {
			fsworldstorage::load_chunk(&world_id, StoredWorldRole::Local, &chunk_position)?
		}
		else {
			gen_test_chunk(chunk_position)
		};
		world.ingest_loaded_chunk(chunk_position, chunk)?;
		if let Some(terrain_renderer) = terrain_notify.as_mut() {
			terrain_renderer.notify_chunk_remesh_needed(&chunk_position);
		}
	}
	let worldgen_elapsed_millis = worldgen_start.elapsed().as_micros() as f32 / 1000.0;
	info!("Took {} milliseconds to do worldgen", worldgen_elapsed_millis);
	Ok(())
}*/

// Never returns. Unfortunately the event loop's exit functionality does not just destroy the event loop, it closes the program.
pub fn run_client(
	identity_keys: IdentityKeyPair,
	voxel_event_sender: NetSendChannel<VoxelChangeRequest>,
	mut voxel_event_receiver: NetMsgReceiver<VoxelChangeAnnounce>,
	server_identity: Option<NodeIdentity>,
	async_runtime: tokio::runtime::Runtime,
) {
	let event_loop = winit::event_loop::EventLoop::new();
	// Open config
	let mut open_options = std::fs::OpenOptions::new();
	open_options.read(true).append(true).create(true);

	let config_maybe: Result<ClientConfig, StartClientError> = open_options
		.open(CLIENT_CONFIG_FILENAME)
		.map_err(StartClientError::from)
		.and_then(|file| {
			let mut buf_reader = BufReader::new(file);
			let mut contents = String::new();
			buf_reader
				.read_to_string(&mut contents)
				.map_err(StartClientError::from)?;
			Ok(contents)
		})
		.and_then(|e| ron::from_str(e.as_str()).map_err(StartClientError::from));
	//If that didn't load, just use built-in defaults.
	let config: ClientConfig = match config_maybe {
		Ok(c) => c,
		Err(e) => {
			warn!("Couldn't open client config, using defaults. Error was: {:?}", e);
			ClientConfig::default()
		}
	};

	// Let the server know we're joining if they're there.
	if let Some(server) = server_identity.as_ref() {
		let join_msg = JoinDefaultEntry {
			display_name: config.your_display_name.clone(),
		};
		net_send_channel::send_to(join_msg, &server).unwrap();
	}

	//let world_id = get_lobby_world_id(&identity_keys.public);

	// Set up camera and view.
	const FAST_CAMERA_SPEED: f32 = 16.0;
	const SLOW_CAMERA_SPEED: f32 = 4.0;
	let view_location = glam::Vec3::new(0.0, 1.0, 2.0);
	let mut camera = camera::Camera::new(view_location, 16.0 / 9.0);

	camera.speed = SLOW_CAMERA_SPEED;

	// Set up window and event loop.
	let window_builder = config.display_properties.to_window_builder();
	let window = window_builder.build(&event_loop).unwrap();

	//let window_size = window.inner_size();
	//let mut resolution = glam::UVec2::new(window_size.width, window_size.height);

	let mut renderer = async_runtime
		.block_on(Renderer::new(&window, &camera, &config))
		.unwrap();

	camera.set_aspect_ratio(renderer.get_aspect_ratio());

	//camera.look_at(glam::Vec3::new(0.0, 0.0, 0.0));

	//Set up some test art assets.
	let air_id = 0;
	let stone_id = 1;
	let dirt_id = 2;
	let grass_id = 3;
	let dome_thing_id = 4;

	let mut image_loader = DevImageLoader::new();

	let test_dome_thing_image_id = image_loader
		.preload_image_file("test.png", identity_keys)
		.unwrap();
	let test_grass_image_id = image_loader
		.preload_image_file("testgrass.png", identity_keys)
		.unwrap();
	let test_stone_image_id = image_loader
		.preload_image_file("teststone.png", identity_keys)
		.unwrap();
	let test_dirt_image_id = image_loader
		.preload_image_file("testdirt.png", identity_keys)
		.unwrap();

	let testlet_image_id = image_loader
		.preload_image_file("testlet.png", identity_keys)
		.unwrap();
	let testlet_2_image_id = image_loader
		.preload_image_file("testvesaria.png", identity_keys)
		.unwrap();
	let testlet_3_image_id = image_loader
		.preload_image_file("testpoak.png", identity_keys)
		.unwrap();

	
	let test_posi_x_image_id = image_loader
		.preload_image_file("test_posi_x.png", identity_keys)
		.unwrap();
	let test_posi_y_image_id = image_loader
		.preload_image_file("test_posi_y.png", identity_keys)
		.unwrap();
	let test_posi_z_image_id = image_loader
		.preload_image_file("test_posi_z.png", identity_keys)
		.unwrap();
	let test_nega_x_image_id = image_loader
		.preload_image_file("test_nega_x.png", identity_keys)
		.unwrap();
	let test_nega_y_image_id = image_loader
		.preload_image_file("test_nega_y.png", identity_keys)
		.unwrap();
	let test_nega_z_image_id = image_loader
		.preload_image_file("test_nega_z.png", identity_keys)
		.unwrap();
	
	let mut sides = SidesArray::new_uniform(&ID_MISSING_TEXTURE);
	sides.set(test_posi_x_image_id, VoxelSide::PosiX);
	sides.set(test_posi_y_image_id, VoxelSide::PosiY);
	sides.set(test_posi_z_image_id, VoxelSide::PosiZ);
	sides.set(test_nega_x_image_id, VoxelSide::NegaX);
	sides.set(test_nega_y_image_id, VoxelSide::NegaY);
	sides.set(test_nega_z_image_id, VoxelSide::NegaZ);

	let sides_art = VoxelArt::SimpleCube(CubeArt {
		textures: CubeTex::AllSides(Box::new(sides)),
		cull_self: true, 
		cull_others: true, 
	});
	// Set up our test world a bit

	let mut last_remesh_time = Instant::now();

	let mut tiles_to_art: HashMap<TileId, VoxelArt> = HashMap::new();

	tiles_to_art.insert(air_id, VoxelArt::Invisible);
	tiles_to_art.insert(stone_id, VoxelArt::simple_solid_block(&test_stone_image_id));
	tiles_to_art.insert(dirt_id, VoxelArt::simple_solid_block(&test_dirt_image_id));
	tiles_to_art.insert(grass_id, VoxelArt::simple_solid_block(&test_grass_image_id));
	tiles_to_art.insert(dome_thing_id, VoxelArt::simple_solid_block(&test_dome_thing_image_id));
	tiles_to_art.insert(
		dome_thing_id,
		sides_art,
		//VoxelArt::simple_solid_block(&test_dome_thing_image_id),
	);

    let mut test_chunk: Chunk<u32> = Chunk::new(air_id);
    for i in test_chunk.get_bounds() {
		let vec = Vec3::new(
			(i.x as i32 - (CHUNK_SIZE as i32) / 2) as f32 + 0.5,
			(i.y) as f32 + 0.5,
			(i.z as i32 - (CHUNK_SIZE as i32) / 2) as f32 + 0.5,
		);
		if vec.length_squared() <= 6.5f32 * 6.5f32 {
			test_chunk.set(i, dome_thing_id).unwrap();
		}
    }

	let mut world_space = TileSpace::new();
	world_space.ingest_loaded_chunk(vpos!(0,0,0), test_chunk).unwrap();
	let test_world_range: VoxelRange<i32> = VoxelRange{upper: vpos!(2,2,2), lower: vpos!(-1,-2,-1) };
	for chunk_pos in test_world_range {
		renderer.terrain_renderer.notify_chunk_remesh_needed(&chunk_pos);
		if chunk_pos != vpos!(0,0,0) { 
			world_space.ingest_loaded_chunk(chunk_pos, gen_test_chunk(chunk_pos)).unwrap();
		}
	}
	renderer.terrain_renderer.process_remesh(&world_space, &tiles_to_art).unwrap();
	renderer.process_terrain_mesh_uploads(&mut image_loader).unwrap();

	// Input and time
	let mut current_down = HashSet::new();

	let game_start_time = Instant::now();
	let mut prev_frame_time = Instant::now();

	//let ambient_light = Vec4::new(0.0, 0.4, 0.1, 0.0);

	let mut total_frames: u64 = 0;
	let mut fps_counter_print_times: u64 = 0;

	let mut is_alt_down = false;
	let mut is_tab_down = false;

	let mut has_focus = true;

	let mut entity_world = crate::entity::EcsWorld::default();
	
	let tick_length = TickLength::from_tps(30.0);

	let test_entity = entity_world.spawn((
		EntityPos::new(EntityVec3::new(0.0, 1.0, 0.0)), 
		EntityRot::new_from_euler(DegreeAngle(90.0), DegreeAngle(0.0), DegreeAngle(0.0)),
		BillboardDrawable::new(testlet_image_id.clone(), BillboardStyle::Cylindrical)
	));
	let test_entity_2 = entity_world.spawn((
		EntityPos::new(EntityVec3::new(5.0, 4.0, 0.0)),
		EntityVelocity::new(Vec3::new(0.0, 0.0, 0.5)),
		LastPos::new(EntityVec3::new(5.0, 0.0, 0.0)),
		EntityRot::new_from_euler(DegreeAngle(30.0), DegreeAngle(0.0), DegreeAngle(0.0)),
		BillboardDrawable::new(testlet_2_image_id.clone(), BillboardStyle::Cylindrical)
	));
	let test_entity_2_y = 2.0;
	let test_entity_3 = entity_world.spawn((
		EntityPos::new(EntityVec3::new(0.0, 4.0, -10.0)),
		EntityScale::new(EntityVec3::new(8.0, 8.0, 8.0)),
		BillboardDrawable::new(testlet_3_image_id.clone(), BillboardStyle::Cylindrical)
	));

	renderer.ingest_image(&testlet_image_id, &mut image_loader);
	renderer.ingest_image(&testlet_2_image_id, &mut image_loader);
	renderer.ingest_image(&testlet_3_image_id, &mut image_loader);

	window.focus_window();

	let mut window_center = {
		let window_top_left = window.inner_position().unwrap();
		let window_size = window.inner_size();
		PhysicalPosition::new(
			window_top_left.x + (window_size.width as i32 / 2),
			window_top_left.y + (window_size.height as i32 / 2),
		)
	};

	let clear_color = Color { 
		r: 89,
		g: 102, 
		b: 204
	};

	let mut accumulated_tick_time: f32 = 0.0;
	let mut game_tick: u64 = 0;
	//let mut last_tick = Instant::now();

	event_loop.run(move |event, _, control| {
		let elapsed_secs = prev_frame_time.elapsed().as_secs_f64() as f32;
		accumulated_tick_time = accumulated_tick_time + elapsed_secs;
		while accumulated_tick_time >= tick_length.get() { 
			game_tick +=1; 
			accumulated_tick_time -= tick_length.get();
			if (game_tick % 300) == 0 {
				info!("Ticking game for the {game_tick}th time."); 
			}
			tick_movement_system(&mut entity_world, tick_length);
			//last_tick = Instant::now(); 
		}
		if let Ok(events) = voxel_event_receiver.recv_poll() {
			for (_ident, announce) in events {
				let old_value = world_space.get(announce.pos).unwrap();
				if announce.new_tile != *old_value {
					world_space.set(announce.pos, announce.new_tile).unwrap();
					renderer.terrain_renderer.notify_changed(&announce.pos);
				}
			}
		}
		match event {
			//WindowEvent::MouseInput is more useful for GUI input
			winit::event::Event::WindowEvent {
				event:
					winit::event::WindowEvent::CursorMoved {
						position: _position,
						..
					},
				..
			} => {
				//Handle GUI mouse movement here.
				/*
				if let Some(pos) = previous_position {

					if has_focus {
						let diff_x = pos.x - position.x;
						let diff_y = pos.y - position.y;
						camera.mouse_interact(diff_x as f32, -diff_y as f32);
					}
				}
				previous_position = Some(position);*/
			}
			//DeviceEvent is better for gameplay-related movement / camera controls
			winit::event::Event::DeviceEvent {
				event: DeviceEvent::MouseMotion { delta, .. },
				..
			} => {
				//Handle gameplay-related / character controller mouse input
				let (dx, dy) = delta;
				let dx = dx as f32;
				let dy = dy as f32;
				let adjusted_dx = dx * elapsed_secs * config.mouse_sensitivity_x;
				let adjusted_dy = dy * elapsed_secs * config.mouse_sensitivity_y;
				if has_focus {
					camera.mouse_interact(adjusted_dx as f32, adjusted_dy as f32);
				}
			},
			winit::event::Event::DeviceEvent {
				event: DeviceEvent::Button {
					button: 1, // Left-click
					state: ElementState::Released,
					..
				},
				..
			} => {
				let hit = match click_voxel(&world_space, &camera, &[air_id], 1024) {
					Ok((result_position, result_id, _)) => {
						Some((result_position, result_id))
					},
					Err(TileSpaceError::NotYetLoaded(pos) ) => {
						info!("Tried to set a block on chunk {:?}, which is not yet loaded. Ignoring.", pos);
						None
					},
					Err(e) => {
						error!("Tile access error: {:?}", e);
						None
					},
				};
				if let Some((result_position, _result_id)) = hit {
					match world_space.set(result_position, air_id) {
						Ok(()) => {

							if let Some(_server) = server_identity.as_ref() {
								let voxel_msg = VoxelChangeRequest {
									pos: result_position.clone(),
									new_tile: air_id,
								};
								voxel_event_sender.send_one(voxel_msg).unwrap();
							}

							renderer.terrain_renderer.notify_changed(&result_position);
						},
						Err(TileSpaceError::NotYetLoaded(pos) ) => info!("Tried to set a block on chunk {:?}, which is not yet loaded. Ignoring.", pos),
						Err(e) => error!("Tile access error: {:?}", e),
					}
				}
			},
			winit::event::Event::DeviceEvent {
				event: DeviceEvent::Button {
					button: 3, // Right-click
					state: ElementState::Released,
					..
				},
				..
			} => {
				let hit = match click_voxel(&world_space, &camera, &[air_id], 1024) {
					Ok((result_position, result_id, side)) => {
						Some((result_position, result_id, side))
					},
					Err(TileSpaceError::NotYetLoaded(pos) ) => {
						println!("Tried to set a block on chunk {:?}, which is not yet loaded. Ignoring.", pos);
						None
					},
					Err(e) => {
						panic!("Tile access error: {:?}", e);
						//None
					},
				};
				if let Some((result_position, _result_id, hit_side)) = hit {
					let placement_position = result_position.get_neighbor(hit_side);

					trace!("Placement position is {}", placement_position);
					if let Ok(placement_id) = world_space.get(placement_position) {
						//Don't waste time setting stone to stone.
						if *placement_id != stone_id {
							match world_space.set(placement_position, stone_id) {
								Ok(()) => {

									if let Some(_server) = server_identity.as_ref() {
										let voxel_msg = VoxelChangeRequest {
											pos: result_position.clone(),
											new_tile: stone_id,
										};
										voxel_event_sender.send_one(voxel_msg).unwrap();
									}

									renderer.terrain_renderer.notify_changed(&placement_position);
								},
								Err(TileSpaceError::NotYetLoaded(pos) ) => info!("Tried to set a block on chunk {:?}, which is not yet loaded. Ignoring.", pos),
								Err(e) => error!("Tile access error: {:?}", e),
							}
						}
					}
				}
			},
			winit::event::Event::WindowEvent {
				event: winit::event::WindowEvent::Focused(focus_status),
				..
			} => {
				has_focus = focus_status;
				if focus_status {
					window.set_cursor_grab(CursorGrabMode::Confined).unwrap();
				} else {
					window.set_cursor_grab(CursorGrabMode::None).unwrap();
				}
				window.set_cursor_visible(!focus_status);
			}
			winit::event::Event::WindowEvent {
				event:
					winit::event::WindowEvent::KeyboardInput {
						input,
						is_synthetic: false,
						..
					},
				..
			} => {
				if input.state == ElementState::Pressed {
					if input.virtual_keycode == Some(VirtualKeyCode::LShift) {
						camera.speed = FAST_CAMERA_SPEED;
					} else if (input.virtual_keycode == Some(VirtualKeyCode::LAlt))
						|| (input.virtual_keycode == Some(VirtualKeyCode::RAlt))
					{
						is_alt_down = true;
					} else if input.virtual_keycode == Some(VirtualKeyCode::Tab) {
						is_tab_down = true;
					}
					let dir_maybe = input.virtual_keycode.and_then(camera::Directions::from_key);
					if let Some(dir) = dir_maybe {
						current_down.insert(dir);
					}

					if is_alt_down && is_tab_down && has_focus {
						window.set_cursor_visible(true);
						window.set_cursor_grab(CursorGrabMode::None).unwrap();
					}
				} else if input.state == ElementState::Released {
					if input.virtual_keycode == Some(VirtualKeyCode::LShift) {
						camera.speed = SLOW_CAMERA_SPEED;
					} else if (input.virtual_keycode == Some(VirtualKeyCode::LAlt))
						|| (input.virtual_keycode == Some(VirtualKeyCode::RAlt))
					{
						is_alt_down = false;
					} else if input.virtual_keycode == Some(VirtualKeyCode::Tab) {
						is_tab_down = false;
					} else if input.virtual_keycode == Some(VirtualKeyCode::Escape) {
						async_runtime
							.block_on(message::quit_game(Duration::from_secs(5)))
							.unwrap();
						*control = ControlFlow::Exit;
					}
					let dir_maybe = input.virtual_keycode.and_then(camera::Directions::from_key);
					if let Some(dir) = dir_maybe {
						current_down.remove(&dir);
					}
				}
			}
			// Close button was clicked, we should close.
			winit::event::Event::WindowEvent {
				event: winit::event::WindowEvent::CloseRequested,
				..
			} => {
				*control = winit::event_loop::ControlFlow::Exit;
			}
			// Window was resized, need to resize renderer.
			winit::event::Event::WindowEvent {
				event: winit::event::WindowEvent::Resized(physical_size),
				..
			} => {
				//resolution = glam::UVec2::new(physical_size.width, physical_size.height);
				renderer.resize(physical_size.into());
				window_center = {
					let window_top_left = window.inner_position().unwrap();
					let window_size = window.inner_size();
					PhysicalPosition::new(
						window_top_left.x + (window_size.width as i32 / 2),
						window_top_left.y + (window_size.height as i32 / 2),
					)
				};
				camera.set_aspect_ratio(renderer.get_aspect_ratio());
			}
			// Render!
			winit::event::Event::MainEventsCleared => {
				total_frames += 1;
				//All input handled, do per-frame behavior.
				let elapsed_time = prev_frame_time.elapsed();
				prev_frame_time = Instant::now();

				if has_focus {
					//Move camera
					for dir in current_down.iter() {
						camera.key_interact(*dir, elapsed_time);
					}
				}
				match entity_world.query_one_mut::<&mut EntityPos>(test_entity_2) {
					Ok(position) => {
						let mut inner = position.get();
						inner.y = test_entity_2_y + game_start_time.elapsed().as_secs_f32().sin(); 
						position.set(inner);
					},
					Err(_) => todo!(),
				}

				
				// Remesh if it's not too spammy.
				if last_remesh_time.elapsed().as_millis() > 64 {
					let meshing_start = Instant::now();
					let was_remesh_needed = renderer.terrain_renderer.process_remesh(&world_space, &tiles_to_art).unwrap();
					if was_remesh_needed {
						let meshing_elapsed_millis = meshing_start.elapsed().as_micros() as f32 / 1000.0;
						info!("Took {meshing_elapsed_millis} milliseconds to do meshing");

						let start_upload_gpu = Instant::now();
						renderer.process_terrain_mesh_uploads(&mut image_loader).unwrap();
						let elapsed_millis = start_upload_gpu.elapsed().as_micros() as f32 / 1000.0;
						info!("Took {elapsed_millis} milliseconds to do push_to_gpu step");

						last_remesh_time = Instant::now();
					}
				}

				let draw_start = Instant::now();

				//Tell us some about it.
				let draw_time = draw_start.elapsed();

				renderer.render_frame(&camera,
					&entity_world, 
					&clear_color, 
					accumulated_tick_time as f32).unwrap();

				let total_time = game_start_time.elapsed();
				let current_fps = (total_frames as f64) / (total_time.as_secs_f64());
				if (total_time.as_secs() % 5 == 0)
					&& (fps_counter_print_times < (total_time.as_secs() / 5))
				{
					info!(
						"Average frames per second, total runtime of the program, is {}",
						current_fps
					);
					info!("Last frame took {} millis", elapsed_time.as_millis());
					info!("{} millis were spent drawing the frame.", draw_time.as_millis());
					fps_counter_print_times += 1;
				}

				if has_focus {
					window.set_cursor_position(window_center).unwrap();
				}
			}
			winit::event::Event::LoopDestroyed => {
				// Cleanup on quit.
				let cfg_string =
					ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default()).unwrap();
				let mut open_options = std::fs::OpenOptions::new();
				open_options.write(true).truncate(true).create(true);

				match open_options.open(CLIENT_CONFIG_FILENAME) {
					Ok(mut file) => {
						file.write_all(cfg_string.as_bytes()).unwrap();
						file.flush().unwrap();
					}
					Err(err) => {
						error!("Could not write config file at exit! Reason is {:?}. Your configs were {}", err, cfg_string)
					}
				}
				// Save world files
				/*
				let chunks = world_space.get_loaded_chunks();
				for chunk_pos in chunks {
					let chunk = world_space.borrow_chunk(chunk_pos).unwrap();
					fsworldstorage::save_chunk(&world_id,
						StoredWorldRole::Local,
						chunk_pos,
						chunk).unwrap();
				};*/
			}
			// Other events we don't care about
			_ => {}
		}
		if *control != ControlFlow::Exit {
			//Make sure we keep looping until we're done.
			*control = winit::event_loop::ControlFlow::Poll;
		}
	});
}
