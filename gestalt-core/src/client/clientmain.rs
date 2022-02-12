use std::{
    error::Error,
    io::{BufReader, Read, Write},
    sync::Arc,
    time::Instant,
};

use glam::Vec4;
use hashbrown::{HashMap, HashSet};
use image::RgbaImage;
use log::{error, warn};
use rend3::types::Handedness;
use serde::{Deserialize, Serialize};
use wgpu::Backend;
use winit::{
    event::{DeviceEvent, ElementState, VirtualKeyCode},
    event_loop::ControlFlow,
    window::Fullscreen, dpi::PhysicalPosition,
};

use crate::{common::{voxelmath::{VoxelPos, VoxelRange}, identity::IdentityKeyPair}, resource::ResourceKind, world::{ChunkPos, chunk::ChunkInner, tilespace::{TileSpace, TileSpaceError}}, client::render::TerrainRenderer};
use crate::{
    client::render::CubeArt,
    common::identity::NodeIdentity,
    resource::{
        image::{ImageProvider, InternalImage, RetrieveImageError},
        update_global_resource_metadata, ResourceInfo, ResourceId, ResourceStatus,
    },
    world::{
        chunk::{Chunk, CHUNK_SIZE},
        TileId, VoxelStorage, VoxelStorageBounded,
    },
};

use super::camera;

pub const WINDOW_TITLE: &'static str = "Gestalt";
pub const CLIENT_CONFIG_FILENAME: &'static str = "client_config.ron";

// Core / main part of the game client. Windowing and event dispatching lives here.
// Input events come in through here.
// Very important that input does not live on the same thread as any heavy compute tasks!
// We need to still be able to read input when the weird stuff is happening.

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WindowMode {
    Windowed {
        /// If windowed, can this be resized with the OS' drag-and-drop controls?
        resizable: bool,
        /// Maximized upon creation?
        maximized: bool,
    },
    BorderlessFullscreen,
    ExclusiveFullscreen,
}
impl Default for WindowMode {
    fn default() -> Self {
        WindowMode::Windowed {
            resizable: true,
            maximized: false,
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct DisplaySize {
    pub width: u32,
    pub height: u32,
}
impl Default for DisplaySize {
    fn default() -> Self {
        DisplaySize {
            width: 1024,
            height: 768,
        }
    }
}
impl From<DisplaySize> for winit::dpi::Size {
    fn from(size: DisplaySize) -> Self {
        winit::dpi::Size::Physical(winit::dpi::PhysicalSize {
            width: size.width,
            height: size.height,
        })
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub size: DisplaySize,
    pub window_mode: WindowMode,
    ///Corresponds to winit::MonitorHandle.name()
    pub monitor: Option<String>,
    /// Which graphics card?
    pub device: Option<String>,
}

impl DisplayConfig {
    pub fn to_window_builder(&self) -> winit::window::WindowBuilder {
        //TODO: Select device
        let builder = winit::window::WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(self.size);
        match self.window_mode {
            WindowMode::Windowed {
                resizable,
                maximized,
            } => builder
                .with_resizable(resizable)
                .with_maximized(maximized)
                .with_fullscreen(None),
            WindowMode::BorderlessFullscreen => {
                builder.with_fullscreen(Some(Fullscreen::Borderless(None)))
            }
            WindowMode::ExclusiveFullscreen => {
                todo!()
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    pub display_properties: DisplayConfig,
    pub mouse_sensitivity_x: f32,
    pub mouse_sensitivity_y: f32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            display_properties: Default::default(),
            mouse_sensitivity_x: 64.0,
            mouse_sensitivity_y: 64.0,
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StartClientError {
    #[error("could not read client config file, i/o error: {0:?}")]
    CouldntOpenConfig(#[from] std::io::Error),
    #[error("Attempted to create channel {0}, which exists already.")]
    CouldntParseConfig(#[from] ron::Error),
    #[error("Could not initialize display: {0:?}")]
    CreateWindowError(#[from] winit::error::OsError),
}

// Loads images for the purposes of testing in development.
pub struct DevImageLoader {
    pub(crate) images: HashMap<ResourceId, RgbaImage>,
    pub(crate) metadata: HashMap<ResourceId, ResourceInfo>,
}

impl ImageProvider for DevImageLoader {
    fn load_image(
        &mut self,
        image: &ResourceId,
    ) -> ResourceStatus<&InternalImage, RetrieveImageError> {
        match self.images.get(image) {
            Some(v) => ResourceStatus::Ready(v),
            None => {
                ResourceStatus::Errored(RetrieveImageError::DoesNotExist(resource_debug!(image)))
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
    fn preload_image_file(&mut self, filename: &str, creator_identity: IdentityKeyPair) -> Result<ResourceId, Box<dyn Error>> {
        let mut open_options = std::fs::OpenOptions::new();
        open_options.read(true).create(false);

        let mut file = open_options.open(filename)?;
        let mut buf: Vec<u8> = Vec::default();
        let _len = file.read_to_end(&mut buf)?;

        let rid = ResourceId::from_buf(buf.as_slice());

        let image = image::load_from_memory(buf.as_slice())?;

        rid.verify(buf.as_slice())?;

        let metadata = ResourceInfo {
            id: rid.clone(),
            filename: filename.to_string(),
            path: None,
            creator: creator_identity.public.clone(),
            resource_type: "image/png".to_string(),
            authors: "Gyro".to_string(),
            description: Some("Image for early testing purposes.".to_string()),
            kind: ResourceKind::PlainOldData,
            signature: creator_identity.sign(&buf)?,
        };

        update_global_resource_metadata(&rid, metadata.clone());

        self.images.insert(rid.clone(), image.into_rgba8());
        self.metadata.insert(rid, metadata);

        Ok(rid)
    }
}

/// Dirt simple worldgen for the sake of early testing / development
pub fn gen_test_chunk(chunk_position: ChunkPos) -> Chunk<TileId> {
    const AIR_ID: TileId = 0; 
    const STONE_ID: TileId = 1; 
    const DIRT_ID: TileId = 2; 
    const GRASS_ID: TileId = 3; 

    if chunk_position.y > 0 { 
        Chunk {
            revision: 0,
            inner: ChunkInner::Uniform(AIR_ID),
        }
    } else if chunk_position.y == 0 {
        let mut chunk = Chunk::new(STONE_ID);
        for pos in chunk.get_bounds() { 
            if pos.y == (CHUNK_SIZE as u16 - 1) {
                chunk.set(pos, GRASS_ID).unwrap();
            } else if pos.y > (CHUNK_SIZE as u16 - 4) { 
                chunk.set(pos, DIRT_ID).unwrap();
            }
            //Otherwise it stays stone. 
        }
        chunk
    } else /* chunk_position.y is less than zero */ { 
        Chunk {
            revision: 0,
            inner: ChunkInner::Uniform(STONE_ID),
        }
    }
}

pub fn run_client(identity_keys: IdentityKeyPair) {
    let event_loop = winit::event_loop::EventLoop::new();
    // Open config
    let mut open_options = std::fs::OpenOptions::new();
    open_options.read(true).append(true).create(true);

    let config_maybe: Result<ClientConfig, StartClientError> = open_options
        .open(CLIENT_CONFIG_FILENAME)
        .map_err(|e| StartClientError::from(e))
        .and_then(|file| {
            let mut buf_reader = BufReader::new(file);
            let mut contents = String::new();
            buf_reader
                .read_to_string(&mut contents)
                .map_err(|e| StartClientError::from(e))?;
            Ok(contents)
        })
        .and_then(|e| Ok(ron::from_str(e.as_str()).map_err(|e| StartClientError::from(e))?));
    //If that didn't load, just use built-in defaults.
    let config: ClientConfig = match config_maybe {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Couldn't open client config, using defaults. Error was: {:?}",
                e
            );
            ClientConfig::default()
        }
    };

    // Set up window and event loop.
    let window_builder = config.display_properties.to_window_builder();
    let window = window_builder.build(&event_loop).unwrap();

    let window_size = window.inner_size();
    let mut resolution = glam::UVec2::new(window_size.width, window_size.height);

    {
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let adapters: Vec<wgpu::AdapterInfo> = instance
            .enumerate_adapters(wgpu::Backends::all())
            .map(|a| a.get_info())
            .collect();
        println!("Available rendering adapters are: {:?}", adapters);
        drop(adapters);
        drop(instance);
    }
    
    // Create the Instance, Adapter, and Device. We can specify preferred backend,
    // device name, or rendering mode. In this case we let rend3 choose for us.
    let iad = pollster::block_on(rend3::create_iad(
        Some(Backend::Vulkan),
        config
            .display_properties
            .device
            .clone()
            .map(|name| name.to_lowercase()),
        Some(rend3::RendererMode::GpuPowered),
        None,
    ))
    .unwrap();

    // The one line of unsafe needed. We just need to guarentee that the window
    // outlives the use of the surface.
    let surface = Arc::new(unsafe { iad.instance.create_surface(&window) });
    // Get the preferred format for the surface.
    let format = surface.get_preferred_format(&iad.adapter).unwrap();
    // Configure the surface to be ready for rendering.
    rend3::configure_surface(
        &surface,
        &iad.device,
        format,
        glam::UVec2::new(window_size.width, window_size.height),
        rend3::types::PresentMode::Mailbox,
    );
    // Make us a renderer.
    let renderer = rend3::Renderer::new(
        iad,
        Handedness::Left,
        Some(window_size.width as f32 / window_size.height as f32),
    )
    .unwrap();

    println!(
        "Launching with rendering device: {:?}",
        &renderer.adapter_info
    );

    // Create the pbr pipeline with the same internal resolution and 4x multisampling
    let base_render_graph = rend3_routine::base::BaseRenderGraph::new(&renderer);

    let mut data_core = renderer.data_core.lock();
    let pbr_routine = rend3_routine::pbr::PbrRoutine::new(
        &renderer,
        &mut data_core,
        &base_render_graph.interfaces,
    );
    drop(data_core);
    let tonemapping_routine = rend3_routine::tonemapping::TonemappingRoutine::new(
        &renderer,
        &base_render_graph.interfaces,
        format,
    );

    //Set up some test art assets.
    let air_id = 0;
    let stone_id = 1;
    let dirt_id = 2;
    let grass_id = 3;
    let dome_thing_id = 4;

    let mut image_loader = DevImageLoader::new();

    let test_dome_thing_image_id = image_loader.preload_image_file("test.png", identity_keys.clone()).unwrap();
    let test_grass_image_id = image_loader.preload_image_file("testgrass.png", identity_keys.clone()).unwrap();
    let test_stone_image_id = image_loader.preload_image_file("teststone.png", identity_keys.clone()).unwrap();
    let test_dirt_image_id = image_loader.preload_image_file("testdirt.png", identity_keys.clone()).unwrap();

    let mut tiles_to_art: HashMap<TileId, CubeArt> = HashMap::new();

    tiles_to_art.insert(air_id, CubeArt::airlike());
    tiles_to_art.insert(stone_id, CubeArt::simple_solid_block(&test_stone_image_id));
    tiles_to_art.insert(dirt_id, CubeArt::simple_solid_block(&test_dirt_image_id));
    tiles_to_art.insert(grass_id, CubeArt::simple_solid_block(&test_grass_image_id));
    tiles_to_art.insert(
        dome_thing_id,
        CubeArt::simple_solid_block(&test_dome_thing_image_id),
    );

    // Set up our test world a bit 
    let mut world_space = TileSpace::new();
    let test_world_range: VoxelRange<i32> = VoxelRange{upper: vpos!(4,4,4), lower: vpos!(-3,-3,-3) };
    // Set up our voxel mesher.
    let mut terrain_renderer = TerrainRenderer::new(64);

    let worldgen_start = Instant::now();
    // Build chunks and then immediately let the mesher know they're new. 
    for chunk_position in test_world_range {
        let chunk = gen_test_chunk(chunk_position);
        world_space.ingest_loaded_chunk(chunk_position, chunk).unwrap();
        terrain_renderer.notify_chunk_remesh_needed(&chunk_position);
    }
    let worldgen_elapsed_millis = worldgen_start.elapsed().as_micros() as f32 / 1000.0; 
    println!("Took {} milliseconds to do worldgen", worldgen_elapsed_millis);

    //Remesh
    let meshing_start = Instant::now();
    terrain_renderer.process_remesh(&world_space, &tiles_to_art).unwrap();
    let meshing_elapsed_millis = meshing_start.elapsed().as_micros() as f32 / 1000.0; 
    println!("Took {} milliseconds to do meshing", meshing_elapsed_millis);
    terrain_renderer.push_to_gpu(&mut image_loader, renderer.clone()).unwrap();
    let meshing_elapsed_millis = meshing_start.elapsed().as_micros() as f32 / 1000.0; 
    println!("Took {} milliseconds to do meshing + \"push_to_gpu()\" step", meshing_elapsed_millis);

    let mut last_remesh_time = Instant::now();

    // Set up camera and view 
    const FAST_CAMERA_SPEED: f32 = 16.0;
    const SLOW_CAMERA_SPEED: f32 = 4.0;
    let view_location = glam::Vec3::new(3.0, 3.0, -5.0);
    let mut camera = camera::Camera::new(view_location);

    camera.speed = SLOW_CAMERA_SPEED;

    // Set camera's location
    renderer.set_camera_data(rend3::types::Camera {
        projection: rend3::types::CameraProjection::Perspective {
            vfov: 90.0,
            near: 0.1,
        },
        view: camera.get_view_matrix(),
    });

    let _directional_handle = renderer.add_directional_light(rend3::types::DirectionalLight {
        color: glam::Vec3::new(0.2, 0.8, 1.0),
        intensity: 10.0,
        // Direction will be normalized
        direction: glam::Vec3::new(-1.0, -4.0, 2.0),
        distance: 400.0,
    });

    let mut current_down = HashSet::new();

    let game_start_time = Instant::now();
    let mut prev_frame_time = Instant::now();

    let ambient_light = Vec4::new(0.0, 0.4, 0.1, 0.0);

    let mut total_frames: u64 = 0;
    let mut fps_counter_print_times: u64 = 0;

    let mut is_alt_down = false;
    let mut is_tab_down = false;

    let mut has_focus = true; 

    window.focus_window();

    let mut window_center = { 
        let window_top_left = window.inner_position().unwrap(); 
        let window_size = window.inner_size();
        PhysicalPosition::new(window_top_left.x + (window_size.width as i32 / 2), window_top_left.y + (window_size.height as i32 / 2) ) 
    };

    event_loop.run(move |event, _, control| {
        let elapsed_secs = prev_frame_time.elapsed().as_secs_f64() as f32;
        match event {
            //WindowEvent::MouseInput is more useful for GUI input 
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CursorMoved{ 
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
            },
            //DeviceEvent is better for gameplay-related movement / camera controls 
            winit::event::Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { 
                    delta,
                    ..
                },
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
                let player_voxel_position: VoxelPos<i32> = vpos!(camera.get_position().x.floor() as i32, camera.get_position().y.floor() as i32, camera.get_position().z.floor() as i32);
                match world_space.set(player_voxel_position, air_id) {
                    Ok(()) => {
                        terrain_renderer.notify_changed(&player_voxel_position);
                    },
                    Err(TileSpaceError::NotYetLoaded(pos) ) => println!("Tried to set a block on chunk {:?}, which is not yet loaded. Ignoring.", pos),
                    Err(e) => panic!("Tile access error: {:?}", e),
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
                let player_voxel_position: VoxelPos<i32> = vpos!(camera.get_position().x.floor() as i32, camera.get_position().y.floor() as i32, camera.get_position().z.floor() as i32);
                match world_space.set(player_voxel_position, stone_id) {
                    Ok(()) => {
                        terrain_renderer.notify_changed(&player_voxel_position);
                    },
                    Err(TileSpaceError::NotYetLoaded(pos) ) => println!("Tried to set a block on chunk {:?}, which is not yet loaded. Ignoring.", pos),
                    Err(e) => panic!("Tile access error: {:?}", e),
                }
            },
            winit::event::Event::WindowEvent{
                event: winit::event::WindowEvent::Focused(focus_status),
                ..
            } => {
                has_focus = focus_status; 
                window.set_cursor_grab(focus_status).unwrap();
                window.set_cursor_visible(!focus_status);
            },
            winit::event::Event::WindowEvent{
                event: winit::event::WindowEvent::KeyboardInput{
                    input,
                    is_synthetic: false,
                    ..
                },
                ..
            } => { 
                if input.state == ElementState::Pressed {
                    if input.virtual_keycode == Some(VirtualKeyCode::LShift) { 
                        camera.speed = FAST_CAMERA_SPEED;
                    }
                    else if (input.virtual_keycode == Some(VirtualKeyCode::LAlt) ) || (input.virtual_keycode == Some(VirtualKeyCode::RAlt) ){ 
                        is_alt_down = true; 
                    }
                    else if input.virtual_keycode == Some(VirtualKeyCode::Tab) { 
                        is_tab_down = true; 
                    }
                    let dir_maybe = input.virtual_keycode.map(|k| camera::Directions::from_key(k)).flatten();
                    if let Some(dir) = dir_maybe { 
                        current_down.insert(dir);
                    }

                    if is_alt_down && is_tab_down && has_focus { 
                        window.set_cursor_visible(true);
                        window.set_cursor_grab(false).unwrap();
                    }
                }
                else if input.state == ElementState::Released {
                    if input.virtual_keycode == Some(VirtualKeyCode::LShift) { 
                        camera.speed = SLOW_CAMERA_SPEED;
                    }
                    else if (input.virtual_keycode == Some(VirtualKeyCode::LAlt) ) || (input.virtual_keycode == Some(VirtualKeyCode::RAlt) ){ 
                        is_alt_down = false; 
                    }
                    else if input.virtual_keycode == Some(VirtualKeyCode::Tab) { 
                        is_tab_down = false; 
                    }
                    let dir_maybe = input.virtual_keycode.map(|k| camera::Directions::from_key(k)).flatten();
                    if let Some(dir) = dir_maybe { 
                        current_down.remove(&dir);
                    }
                }
            },
            // Close button was clicked, we should close.
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                *control = winit::event_loop::ControlFlow::Exit;
            },
            // Window was resized, need to resize renderer.
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::Resized(physical_size),
                ..
            } => {
                resolution = glam::UVec2::new(physical_size.width, physical_size.height);
                // Reconfigure the surface for the new size.
                rend3::configure_surface(
                    &surface,
                    &renderer.device,
                    format,
                    glam::UVec2::new(resolution.x, resolution.y),
                    rend3::types::PresentMode::Mailbox,
                );
                // Tell the renderer about the new aspect ratio.
                renderer.set_aspect_ratio(resolution.x as f32 / resolution.y as f32);

                window_center = { 
                    let window_top_left = window.inner_position().unwrap(); 
                    let window_size = window.inner_size();
                    PhysicalPosition::new(window_top_left.x + (window_size.width as i32 / 2), window_top_left.y + (window_size.height as i32 / 2) ) 
                };
            },
            // Render!
            winit::event::Event::MainEventsCleared => {
                total_frames +=1;
                //All input handled, do per-frame behavior. 
                let elapsed_time = prev_frame_time.elapsed();
                prev_frame_time = Instant::now();
        
                if has_focus { 
                    //Move camera
                    for dir in current_down.iter() {
                        camera.key_interact(*dir, elapsed_time.clone());
                    }
                    //Update camera
                    renderer.set_camera_data(rend3::types::Camera {
                        projection: rend3::types::CameraProjection::Perspective { vfov: 90.0, near: 0.1 },
                        view: camera.get_view_matrix(),
                    });
                }

                // Remesh if it's not too spammy. 
                if last_remesh_time.elapsed().as_millis() > 64 { 
                    let meshing_start = Instant::now();
                    let was_remesh_needed = terrain_renderer.process_remesh(&world_space, &tiles_to_art).unwrap();
                    if was_remesh_needed { 
                        let meshing_elapsed_millis = meshing_start.elapsed().as_micros() as f32 / 1000.0; 
                        println!("Took {} milliseconds to do meshing", meshing_elapsed_millis);
                        terrain_renderer.push_to_gpu(&mut image_loader, renderer.clone()).unwrap();
                        let meshing_elapsed_millis = meshing_start.elapsed().as_micros() as f32 / 1000.0; 
                        println!("Took {} milliseconds to do meshing + \"push_to_gpu()\" step", meshing_elapsed_millis);
    
                        last_remesh_time = Instant::now();
                    }
                }

                // Present the frame on screen
                let draw_start = Instant::now();
                // Get a frame
                let frame = rend3::util::output::OutputFrame::Surface {
                    surface: Arc::clone(&surface),
                };
                // Ready up the renderer
                let (cmd_bufs, ready) = renderer.ready();

                // Build a rendergraph
                let mut graph = rend3::RenderGraph::new();

                // Add the default rendergraph without a skybox
                base_render_graph.add_to_graph(
                    &mut graph,
                    &ready,
                    &pbr_routine,
                    None,
                    &tonemapping_routine,
                    resolution,
                    rend3::types::SampleCount::One,
                    ambient_light,
                );

                // Dispatch a render using the built up rendergraph!
                graph.execute(&renderer, frame, cmd_bufs, &ready);
                
                //Tell us some about it. 
                let draw_time = draw_start.elapsed();

                let total_time = game_start_time.elapsed(); 
                let current_fps = (total_frames as f64)/(total_time.as_secs_f64());
                if (total_time.as_secs() % 5 == 0) && (fps_counter_print_times < (total_time.as_secs()/5) ) {
                    println!("Render device is: {:?}", &renderer.adapter_info);
                    println!("Average frames per second, total runtime of the program, is {}", current_fps);
                    println!("Last frame took {} millis", elapsed_time.as_millis());
                    println!("{} millis were spent drawing the frame.", draw_time.as_millis());
                    fps_counter_print_times += 1; 
                }

                if has_focus { 
                    window.set_cursor_position(window_center).unwrap();
                }
            },
            winit::event::Event::LoopDestroyed => {
                // Cleanup on quit. 
                let cfg_string = ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default() ).unwrap();
                let mut open_options = std::fs::OpenOptions::new();
                open_options
                    .write(true)
                    .truncate(true)
                    .create(true);
                
                match open_options.open(CLIENT_CONFIG_FILENAME) {
                    Ok(mut file) => { 
                        file.write_all(cfg_string.as_bytes()).unwrap();
                        file.flush().unwrap();
                    },
                    Err(err) => {
                        error!("Could not write config file at exit! Reason is {:?}. Your configs were {}", err, cfg_string)
                    }
                }
            },
            // Other events we don't care about
            _ => {}
        }
        if *control != ControlFlow::Exit {
            //Make sure we keep looping until we're done.
            *control = winit::event_loop::ControlFlow::Poll;
        }
    });
}