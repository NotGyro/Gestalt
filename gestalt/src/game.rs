//! Main type for the game. `Game::new().run()` runs the game.

use std::sync::Arc;
use std::time::Instant;
use std::sync::atomic::Ordering;
use std::thread;

use cgmath::{Point3, MetricSpace, Vector3, EuclideanSpace};
use vulkano::buffer::BufferUsage;
use vulkano::instance::Instance;
use vulkano::swapchain::Surface;
use winit::{Event, WindowEvent, DeviceEvent, ElementState, Window, WindowBuilder, EventsLoop};

use crate::vulkano_win::VkSurfaceBuild;
use crate::buffer::CpuAccessibleBufferAutoPool;
use crate::geometry::VertexPositionColorAlpha;
use crate::renderer::Renderer;
use crate::input::InputState;
use crate::registry::DimensionRegistry;
use crate::player::Player;
use crate::world::{Dimension, Chunk, CHUNK_SIZE_F32};
use crate::world::chunk::{CHUNK_STATE_DIRTY, CHUNK_STATE_WRITING, CHUNK_STATE_CLEAN, CHUNK_STATE_GENERATING};
use crate::util::{view_to_frustum, aabb_frustum_intersection, cube};
use crate::pipeline::text_pipeline::TextData;


const MAX_CHUNK_GEN_THREADS: u32 = 1;
const MAX_CHUNK_MESH_THREADS: u32 = 1;


/// Main type for the game. `Game::new().run()` runs the game.
pub struct Game {
    event_loop: EventsLoop,
    surface: Arc<Surface<Window>>,
    renderer: Renderer,
    prev_time: Instant,
    input_state: InputState,
    player: Player,
    dimension_registry: DimensionRegistry,
    chunk_generating_threads: Arc<std::sync::atomic::AtomicU32>,
    chunk_meshing_threads: Arc<std::sync::atomic::AtomicU32>,
}


impl Game {
    /// Creates a new `Game`.
    pub fn new() -> Game {
        let event_loop = EventsLoop::new();
        let instance = Instance::new(None, &crate::vulkano_win::required_extensions(), None).expect("failed to create instance");
        let surface = WindowBuilder::new().build_vk_surface(&event_loop, instance.clone()).unwrap();
        let renderer = Renderer::new(instance.clone(), surface.clone());

        let input_state = InputState::new();

        let mut player = Player::new();
        player.position = Point3::new(16.0, 32.0, 16.0);
        player.yaw = 135.0;
        player.pitch = -30.0;

        let mut dimension_registry = DimensionRegistry::new();
        let dimension = Dimension::new();
        dimension_registry.dimensions.insert(0, dimension);

        Game {
            event_loop,
            surface,
            renderer,
            prev_time: Instant::now(),
            input_state,
            player,
            dimension_registry,
            chunk_generating_threads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            chunk_meshing_threads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
        }
    }


    pub fn run(&mut self) {
        let mut keep_running = true;
        while keep_running {
            keep_running = self.update();
        }
    }


    pub fn update(&mut self) -> bool {
        let mut keep_running = true;

        let elapsed = Instant::now() - self.prev_time;
        let dt: f64 = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 * 1e-9;
        // dt capped for laggy frames to prevent mouse swinging like 180° around on the next frame
        let dt_clamped: f64 = if dt > 0.1 { 0.05 } else { dt };
        self.prev_time = Instant::now();

        self.input_state.mouse_delta = (0.0, 0.0);

        let mut events = Vec::new();
        self.event_loop.poll_events(|event| { events.push(event) });

        for event in events {
            match event {
                Event::WindowEvent { event, .. } => {
                    match event {
                        WindowEvent::CloseRequested => { keep_running = false; },
                        WindowEvent::KeyboardInput { input, .. } => {
                            self.input_state.update_key(input);
                        },
                        _ => {}
                    }
                },
                Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta }, .. } => {
                    let delta = (delta.0 as f32, delta.1 as f32);
                    self.input_state.add_mouse_delta(delta);
                    if self.input_state.right_mouse_pressed {
                        let dimensions = match self.surface.window().get_inner_size() {
                            Some(logical_size) => [logical_size.width as u32, logical_size.height as u32],
                            None => [800, 600]
                        };
                        match self.surface.window().set_cursor_position(::winit::dpi::LogicalPosition::new(dimensions[0] as f64 / 2.0, dimensions[1] as f64 / 2.0)) {
                            Ok(_) => {},
                            Err(err) => { println!("Couldn't set cursor position: {:?}", err); }
                        }
                    }
                },
                Event::DeviceEvent { event: DeviceEvent::Button { button, state }, .. } => {
                    if button == 3 {
                        match state {
                            ElementState::Pressed => {
                                self.surface.window().hide_cursor(true);
                                self.input_state.right_mouse_pressed = true;
                            },
                            ElementState::Released => {
                                self.surface.window().hide_cursor(false);
                                self.input_state.right_mouse_pressed = false;
                            }
                        }
                    }
                },
                _ => {}
            };
        }

        // general updates

        self.player.update(dt_clamped, &self.input_state);

        self.dimension_registry.get(0).unwrap().unload_chunks(self.player.position.clone(), self.renderer.render_queue.clone());
        if self.chunk_generating_threads.load(Ordering::Relaxed) < MAX_CHUNK_GEN_THREADS {
            self.dimension_registry.get(0).unwrap().load_chunks(self.player.position.clone(), self.renderer.render_queue.clone());
        }

        {
            let mut lock = self.renderer.render_queue.write().unwrap();
            (*lock).text.clear();
            lock.text.push(TextData {
                text: format!("{} FPS ({}ms)", (1.0/dt).floor(), (dt*100000.0).floor()/100.0),
                position: (10, 10),
                ..TextData::default()
            });
            lock.text.push(TextData {
                text: format!("Player pos: x: {}, y: {}, z: {}",
                              (self.player.position.x * 10.0).round() / 10.0,
                              (self.player.position.y * 10.0).round() / 10.0,
                              (self.player.position.z * 10.0).round() / 10.0),
                position: (10, 100),
                ..TextData::default()
            });

            let line_queue = &mut lock.lines;
            if line_queue.chunks_changed {
                let mut verts = Vec::new();
                let mut idxs = Vec::new();
                let mut index_offset = 0;
                {
                    let chunks = self.dimension_registry.get(0).unwrap().chunks.read().unwrap();
                    for (pos, (_, _)) in chunks.iter() {
                        verts.append(&mut cube::generate_chunk_debug_line_vertices(pos.0, pos.1, pos.2, 0.25f32).to_vec());
                        idxs.append(&mut cube::generate_chunk_debug_line_indices(index_offset).to_vec());
                        index_offset += 1;
                    }
                }
                line_queue.chunk_lines_vertex_buffer =
                    CpuAccessibleBufferAutoPool::<[VertexPositionColorAlpha]>::from_iter(self.renderer.device.clone(),
                                                                                         self.renderer.memory_pool.clone(),
                                                                                         BufferUsage::all(),
                                                                                         verts.iter().cloned())
                        .expect("failed to create buffer");
                line_queue.chunk_lines_index_buffer =
                    CpuAccessibleBufferAutoPool::<[u32]>::from_iter(self.renderer.device.clone(),
                                                                    self.renderer.memory_pool.clone(),
                                                                    BufferUsage::all(),
                                                                    idxs.iter().cloned())
                        .expect("failed to create buffer");
                line_queue.chunks_changed = false;
            }
        }
        {
            let mut lock = self.renderer.render_queue.write().unwrap();
            lock.chunk_meshes.clear();
        }
        {
            if self.chunk_meshing_threads.load(Ordering::Relaxed) < MAX_CHUNK_MESH_THREADS {
                let mut chunks = self.dimension_registry.get(0).unwrap().chunks.write().unwrap();
                let mut chunk_positions: Vec< (i32, i32, i32) > = chunks.keys().cloned().collect();

                let player_pos = self.player.position.clone();
                chunk_positions.sort_by(|a, b| {
                    let a_world = Chunk::chunk_pos_to_center_ws(*a);
                    let b_world = Chunk::chunk_pos_to_center_ws(*b);
                    let pdist_a = Point3::distance(Point3::new(a_world.0, a_world.1, a_world.2), player_pos);
                    let pdist_b = Point3::distance(Point3::new(b_world.0, b_world.1, b_world.2), player_pos);
                    pdist_a.partial_cmp(&pdist_b).unwrap()
                });



                for chunk_pos in chunk_positions {
                    match chunks.get_mut(&chunk_pos) {
                        Some((ref mut chunk, ref mut state)) => {
                            let status = state.load(Ordering::Relaxed);
                            if status == CHUNK_STATE_DIRTY {
                                self.chunk_meshing_threads.fetch_add(1, Ordering::Relaxed);
                                state.store(CHUNK_STATE_WRITING, Ordering::Relaxed);
                                let chunk_arc = chunk.clone();
                                let device_arc = self.renderer.device.clone();
                                let memory_pool_arc = self.renderer.memory_pool.clone();
                                let state_arc = state.clone();
                                let thread_count_clone = self.chunk_meshing_threads.clone();
                                thread::spawn(move || {
                                    let mut chunk_lock = chunk_arc.write().unwrap();
                                    (*chunk_lock).generate_mesh(device_arc, memory_pool_arc);
                                    state_arc.store(CHUNK_STATE_CLEAN, Ordering::Relaxed);
                                    thread_count_clone.fetch_sub(1, Ordering::Relaxed);
                                });
                                break;
                            }
                        },
                        None => { continue; }
                    }
                }
            }
        }

        {
            let chunks = self.dimension_registry.get(0).unwrap().chunks.read().unwrap();
            let mut num_generated = 0;

            for (_, (_, state))  in chunks.iter() {
                let status = state.load(Ordering::Relaxed);
                if status != CHUNK_STATE_GENERATING {
                    num_generated += 1;
                }
            }

            {
                let mut lock = self.renderer.render_queue.write().unwrap();
                lock.text.push(TextData {
                    text: format!("chunks generated: {}", num_generated),
                    position: (10, 40),
                    ..TextData::default()
                });
            }
        }

        // queueing chunks and drawing
        {
            let chunks = self.dimension_registry.get(0).unwrap().chunks.read().unwrap();
            let frustum = view_to_frustum(self.player.pitch, self.player.yaw, self.player.camera.fov, 4.0/3.0, 1.0, 10000.0);

            for (pos, (chunk, state)) in chunks.iter() {
                let aabb_min = Point3::new(pos.0 as f32, pos.1 as f32, pos.2 as f32) * CHUNK_SIZE_F32 - self.player.position.to_vec();
                let aabb_max = aabb_min + Vector3::new(CHUNK_SIZE_F32, CHUNK_SIZE_F32, CHUNK_SIZE_F32);
                let is_ready = state.load(Ordering::Relaxed) == CHUNK_STATE_CLEAN;
                let is_in_view = aabb_frustum_intersection(aabb_min, aabb_max, frustum.clone());
                if is_ready && is_in_view {
                    let chunk_lock = chunk.read().unwrap();
                    let mut queue_lock = self.renderer.render_queue.write().unwrap();
                    queue_lock.chunk_meshes.append(&mut chunk_lock.mesh.queue());
                }
            }
        }

        self.renderer.draw(&self.player.camera, self.player.get_transform());

        keep_running
    }
}