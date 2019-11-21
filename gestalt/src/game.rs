//! Main type for the game. `Game::new().run()` runs the game.

use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicU64};
use std::thread;

use cgmath::{Point3, MetricSpace, Vector3, EuclideanSpace};
use winit::{Event, WindowEvent, DeviceEvent, ElementState, EventsLoop, VirtualKeyCode};

use phosphor::pipeline::text::TextData;
use phosphor::geometry::VertexGroup;
use phosphor::renderer::Renderer;
use toolbox::{view_to_frustum, aabb_frustum_intersection};

use crate::input::InputState;
use crate::world::dimension::DimensionRegistry;
use crate::metrics::FrameMetrics;
use crate::player::Player;
use crate::world::{Dimension, Chunk, CHUNK_SIZE_F32};
use crate::world::chunk::{CHUNK_STATE_DIRTY, CHUNK_STATE_MESHING, CHUNK_STATE_CLEAN, CHUNK_STATE_GENERATING};


const MAX_CHUNK_GEN_THREADS: u32 = 1;
const MAX_CHUNK_MESH_THREADS: u32 = 2;


/// Main type for the game. `Game::new().run()` runs the game.
pub struct Game {
    event_loop: EventsLoop,
    renderer: Renderer,
    frame_metrics: FrameMetrics,
    input_state: InputState,
    player: Player,
    dimension_registry: DimensionRegistry,
    chunk_generating_threads: Arc<std::sync::atomic::AtomicU32>,
    chunk_meshing_threads: Arc<std::sync::atomic::AtomicU32>,
    visible_ids: [bool; 65536],
    tick: Arc<AtomicU64>,
}


impl Game {
    /// Creates a new `Game`.
    pub fn new() -> Game {
        let event_loop = EventsLoop::new();
        let renderer = Renderer::new(&event_loop);

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
            renderer,
            frame_metrics: FrameMetrics::new(),
            input_state,
            player,
            dimension_registry,
            chunk_generating_threads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            chunk_meshing_threads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            visible_ids: [false; 65536],
            tick: Arc::new(AtomicU64::new(0)),
        }
    }


    pub fn run(&mut self) {
        lumberjack::set_tick_arc(self.tick.clone());

        let mut keep_running = true;
        while keep_running {
            keep_running = self.update();
        }
    }


    pub fn update(&mut self) -> bool {
        self.tick.fetch_add(1, Ordering::Relaxed);

        let mut keep_running = true;

        let elapsed = self.frame_metrics.start_frame();
        let dt: f64 = elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 * 1e-9;
        // dt capped for laggy frames to prevent mouse swinging like 180° around on the next frame
        let dt_clamped: f64 = if dt > 0.1 { 0.05 } else { dt };

        self.input_state.mouse_delta = (0.0, 0.0);

        let mut events = Vec::new();
        self.event_loop.poll_events(|event| { events.push(event) });

        // clear first_frame on old keypresses
        self.input_state.update_keys();
        for event in events {
            match event {
                Event::WindowEvent { event, .. } => {
                    match event {
                        WindowEvent::CloseRequested => {
                            keep_running = false;
                        },
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
                        let dimensions = match self.renderer.surface.window().get_inner_size() {
                            Some(logical_size) => [logical_size.width as u32, logical_size.height as u32],
                            None => [800, 600]
                        };
                        match self.renderer.surface.window().set_cursor_position(::winit::dpi::LogicalPosition::new(dimensions[0] as f64 / 2.0, dimensions[1] as f64 / 2.0)) {
                            Ok(_) => {},
                            Err(err) => { warn!(Game, "Couldn't set cursor position: {:?}", err); }
                        }
                    }
                },
                Event::DeviceEvent { event: DeviceEvent::Button { button, state }, .. } => {
                    if button == 3 {
                        match state {
                            ElementState::Pressed => {
                                self.renderer.surface.window().hide_cursor(true);
                                self.input_state.right_mouse_pressed = true;
                            },
                            ElementState::Released => {
                                self.renderer.surface.window().hide_cursor(false);
                                self.input_state.right_mouse_pressed = false;
                            }
                        }
                    }
                },
                _ => {}
            };
        }

        // general updates

        if self.input_state.get_key_just_pressed(&VirtualKeyCode::V) {
            if self.input_state.get_key_down(&VirtualKeyCode::LShift) {
                if self.renderer.info.debug_visualize_setting == 0 {
                    self.renderer.info.debug_visualize_setting = phosphor::renderer::DEBUG_VISUALIZE_MAX - 1;
                }
                else {
                    self.renderer.info.debug_visualize_setting -= 1;
                }
            }
            else {
                self.renderer.info.debug_visualize_setting += 1;
                if self.renderer.info.debug_visualize_setting == phosphor::renderer::DEBUG_VISUALIZE_MAX {
                    self.renderer.info.debug_visualize_setting = 0;
                }
            }
        }

        self.player.update(dt_clamped, &self.input_state);

        self.dimension_registry.get(0).unwrap().unload_chunks(self.player.position.clone(), &self.renderer.info);
        if self.chunk_generating_threads.load(Ordering::Relaxed) < MAX_CHUNK_GEN_THREADS {
            self.dimension_registry.get(0).unwrap().load_chunks(self.player.position.clone(), &self.renderer.info);
        }

        {
            let mut lock = self.renderer.info.render_queues.write().unwrap();
            (*lock).text.clear();
            lock.text.append(&mut self.frame_metrics.get_text((5, 5)));
            lock.text.push(TextData {
                text: format!("Player pos: x: {}, y: {}, z: {}",
                              (self.player.position.x * 10.0).round() / 10.0,
                              (self.player.position.y * 10.0).round() / 10.0,
                              (self.player.position.z * 10.0).round() / 10.0),
                position: (5, 140),
                ..TextData::default()
            });
            let debug_vis_text = match self.renderer.info.debug_visualize_setting {
                phosphor::renderer::DEBUG_VISUALIZE_DISABLED => "Disabled",
                phosphor::renderer::DEBUG_VISUALIZE_POSITION_BUFFER => "Position Buffer",
                phosphor::renderer::DEBUG_VISUALIZE_NORMAL_BUFFER => "Normal Buffer",
                phosphor::renderer::DEBUG_VISUALIZE_ALBEDO_BUFFER => "Albedo Buffer",
                phosphor::renderer::DEBUG_VISUALIZE_ROUGHNESS_BUFFER => "Roughness Buffer",
                phosphor::renderer::DEBUG_VISUALIZE_METALLIC_BUFFER => "Metallic Buffer",
                phosphor::renderer::DEBUG_VISUALIZE_DEFERRED_LIGHTING_ONLY => "Deferred Lighting Only",
                phosphor::renderer::DEBUG_VISUALIZE_NO_POST_PROCESSING => "No Post Processing",
                phosphor::renderer::DEBUG_VISUALIZE_OCCLUSION_BUFFER => "Occlusion Buffer",
                _ => unreachable!()
            };
            lock.text.push(TextData {
                text: format!("Debug visualization: {}", debug_vis_text),
                position: (5, 165),
                ..TextData::default()
            });

            // line queue (disabled)

//            let line_queue = &mut lock.lines;
//            if line_queue.chunks_changed {
//                let mut verts = Vec::new();
//                let mut idxs = Vec::new();
//                let mut index_offset = 0;
//                {
//                    let chunks = self.dimension_registry.get(0).unwrap().chunks.read().unwrap();
//                    for (pos, (_, _)) in chunks.iter() {
//                        verts.append(&mut cube::generate_chunk_debug_line_vertices(pos.0, pos.1, pos.2, 0.25f32).to_vec());
//                        idxs.append(&mut cube::generate_chunk_debug_line_indices(index_offset).to_vec());
//                        index_offset += 1;
//                    }
//                }
//
//                line_queue.chunk_lines_vg = Arc::new(VertexGroup::new(verts.iter().cloned(), idxs.iter().cloned(), 0, self.renderer.info.device.clone(), self.renderer.info.memory_pool.clone()));
//                line_queue.chunks_changed = false;
//            }
        }
        {
            let mut lock = self.renderer.info.render_queues.write().unwrap();
            lock.meshes.clear();
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
                                state.store(CHUNK_STATE_MESHING, Ordering::Relaxed);
                                let chunk_arc = chunk.clone();
                                let device_arc = self.renderer.info.device.clone();
                                let memory_pool_arc = self.renderer.info.memory_pool.clone();
                                let state_arc = state.clone();
                                let thread_count_clone = self.chunk_meshing_threads.clone();
                                thread::spawn(move || {
                                    let mut chunk_lock = chunk_arc.write().unwrap();

                                    // TODO: fix this
                                    //(*chunk_lock).generate_mesh(&self.renderer);

//                                    let chunk_center = Chunk::chunk_pos_to_center_ws(chunk_pos);
//                                    let dist = Point3::distance(Point3::new(chunk_center.0, chunk_center.1, chunk_center.2), player_pos);
                                    let occluder_scale = 2;
//                                    if dist > 128.0 {
//                                        occluder_scale = 3;
//                                        if dist > 192.0 {
//                                            occluder_scale = 4;
//                                        }
//                                    }
                                    (*chunk_lock).generate_occlusion_mesh(occluder_scale);
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
                let mut lock = self.renderer.info.render_queues.write().unwrap();
                lock.text.push(TextData {
                    text: format!("Chunks generated: {}", num_generated),
                    position: (5, 80),
                    ..TextData::default()
                });
            }
        }

        // queueing chunks and drawing

        match self.dimension_registry.get(0).unwrap().chunks.try_read() {
            Ok(chunks) => {
                for (_, (chunk, _)) in chunks.iter() {
                    if let Ok(c) = chunk.try_read() {
                        self.visible_ids[c.id as usize] = false;
                    }
                }
            }
            Err(_) => {}
        }
        {
            let queue_lock = self.renderer.info.render_queues.read().unwrap();
            let buffer_lock = queue_lock.occluders.output_cpu_buffer.read().unwrap();
            for u in buffer_lock.iter() {
                self.visible_ids[*u as usize] = true;
            }
        }
        // 0 is the clear value for the buffer. no chunks should have id 0
        self.visible_ids[0] = false;

        let mut occlusion_verts = Vec::new();
        let mut occlusion_indices = Vec::new();
        let mut offset = 0;
        {
            let mut num_chunks_before_culling = 0;
            let chunks = self.dimension_registry.get(0).unwrap().chunks.read().unwrap();
            let frustum = view_to_frustum(self.player.pitch, self.player.yaw, self.player.camera.fov, 4.0/3.0, 1.0, 10000.0);

            for (pos, (chunk, state)) in chunks.iter() {
                let aabb_min = Point3::new(pos.0 as f32, pos.1 as f32, pos.2 as f32) * CHUNK_SIZE_F32 - self.player.position.to_vec();
                let aabb_max = aabb_min + Vector3::new(CHUNK_SIZE_F32, CHUNK_SIZE_F32, CHUNK_SIZE_F32);
                let status = state.load(Ordering::Relaxed);
                let is_ready = status == CHUNK_STATE_CLEAN;
                if is_ready {
                    num_chunks_before_culling += 1;
                }
                let is_in_view = aabb_frustum_intersection(aabb_min, aabb_max, frustum.clone());
                if status == CHUNK_STATE_CLEAN {
                    match chunk.try_write() {
                        Ok(mut chunk_lock) => {
                            chunk_lock.get_occluder_geometry(&mut occlusion_verts, &mut occlusion_indices, &mut offset);
                        },
                        Err(_) => {}
                    }

                }
                if is_ready && is_in_view {
                    let chunk_lock = chunk.read().unwrap();
                    let mut draw = false;
                    if self.visible_ids[chunk_lock.id as usize] {
                        draw = true;
                    }
                    else {
                        let chunk_center = Chunk::chunk_pos_to_center_ws(*pos);
                        let dist = Point3::distance(Point3::new(chunk_center.0, chunk_center.1, chunk_center.2), self.player.position.clone());
                        if dist < 48.0 {
                            draw = true;
                        }
                    }
                    if draw {
                        let mut queue_lock = self.renderer.info.render_queues.write().unwrap();
                        queue_lock.meshes.append(&mut chunk_lock.mesh.queue());
                    }
                }
            }
            let mut lock = self.renderer.info.render_queues.write().unwrap();
            lock.text.push(TextData {
                text: format!("Chunks meshed: {}", num_chunks_before_culling),
                position: (5, 95),
                ..TextData::default()
            });
            lock.occluders.vertex_group = Arc::new(VertexGroup::new(occlusion_verts.into_iter(), occlusion_indices.into_iter(), 0, self.renderer.info.device.clone(), self.renderer.info.memory_pool.clone()));
        }

        self.frame_metrics.end_game();

        match self.renderer.draw(&self.player.camera, self.player.get_transform()) {
            Ok(img_future) => {
                self.frame_metrics.end_draw();
                self.renderer.submit(img_future);
            },
            Err(e) => {
                error!(Renderer, "{:?}", e);
                self.frame_metrics.end_draw();
            }
        }

        self.frame_metrics.end_gpu();
        self.frame_metrics.end_frame();

        keep_running
    }
}