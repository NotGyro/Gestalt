//! Main type for the game. `Game::new().run()` runs the game.

use std::sync::Arc;
use std::sync::atomic::{Ordering, AtomicU64};
use std::thread;

use cgmath::{Point3, MetricSpace, Vector3, EuclideanSpace};
use winit::{Event, WindowEvent, DeviceEvent, ElementState, EventsLoop, VirtualKeyCode, MouseButton, KeyboardInput};
use winit::dpi::LogicalSize;

use phosphor::geometry::VertexGroup;
use phosphor::renderer::Renderer;
use toolbox::{view_to_frustum, aabb_frustum_intersection};

use crate::input::InputState;
use crate::world::dimension::DimensionRegistry;
use crate::metrics::{FrameMetrics, ChunkMetrics};
use crate::player::Player;
use crate::world::{Dimension, Chunk, CHUNK_SIZE_F32};
use crate::world::chunk::{CHUNK_STATE_DIRTY, CHUNK_STATE_MESHING, CHUNK_STATE_CLEAN, CHUNK_STATE_GENERATING};
use imgui::{FontSource, FontConfig, FontGlyphRanges, Condition, ImString, im_str, WindowFlags, StyleColor};


const MAX_CHUNK_GEN_THREADS: u32 = 1;
const MAX_CHUNK_MESH_THREADS: u32 = 2;


/// Main type for the game. `Game::new().run()` runs the game.
pub struct Game {
    event_loop: EventsLoop,
    renderer: Renderer,
    frame_metrics: FrameMetrics,
    chunk_metrics: ChunkMetrics,
    input_state: InputState,
    player: Player,
    dimension_registry: DimensionRegistry,
    chunk_generating_threads: Arc<std::sync::atomic::AtomicU32>,
    chunk_meshing_threads: Arc<std::sync::atomic::AtomicU32>,
    visible_ids: [bool; 65536],
    tick: Arc<AtomicU64>,
    imgui: imgui::Context
}


impl Game {
    /// Creates a new `Game`.
    pub fn new() -> Game {
        let mut imgui = imgui::Context::create();
        imgui.set_ini_filename(None);
        imgui.io_mut().config_flags |= imgui::ConfigFlags::DOCKING_ENABLE;
        imgui.io_mut().docking_with_shift = true;

        if let Some(backend) = crate::clipboard_backend::init() {
            imgui.set_clipboard_backend(Box::new(backend));
        } else {
            eprintln!("Failed to initialize clipboard");
        }

        let font_size = 13.0;
        imgui.fonts().add_font(&[
            FontSource::DefaultFontData {
                config: Some(FontConfig {
                    size_pixels: font_size,
                    ..FontConfig::default()
                }),
            },
            FontSource::TtfData {
                data: include_bytes!("../../fonts/mplus-1p-regular.ttf"),
                size_pixels: font_size,
                config: Some(FontConfig {
                    rasterizer_multiply: 1.75,
                    glyph_ranges: FontGlyphRanges::japanese(),
                    ..FontConfig::default()
                }),
            },
        ]);

        let event_loop = EventsLoop::new();
        let renderer = Renderer::new(&event_loop)
            .with_imgui(&mut imgui);

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
            chunk_metrics: ChunkMetrics::default(),
            input_state,
            player,
            dimension_registry,
            chunk_generating_threads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            chunk_meshing_threads: Arc::new(std::sync::atomic::AtomicU32::new(0)),
            visible_ids: [false; 65536],
            tick: Arc::new(AtomicU64::new(0)),
            imgui
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
                        WindowEvent::CursorMoved { position, .. } => {
                            self.imgui.io_mut().mouse_pos = [position.x as f32, position.y as f32];
                        },
                        WindowEvent::MouseInput { state, button, .. } => {
                            let pressed = state == ElementState::Pressed;
                            match button {
                                MouseButton::Left => self.imgui.io_mut().mouse_down[0] = pressed,
                                MouseButton::Right => self.imgui.io_mut().mouse_down[1] = pressed,
                                MouseButton::Middle => self.imgui.io_mut().mouse_down[2] = pressed,
                                MouseButton::Other(idx @ 0..=4) => self.imgui.io_mut().mouse_down[idx as usize] = pressed,
                                _ => (),
                            }
                        },
                        WindowEvent::Resized(LogicalSize { width, height }) => {
                            self.imgui.io_mut().display_size = [width as f32, height as f32];
                        }
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
                Event::DeviceEvent { event: DeviceEvent::Key(KeyboardInput { state, virtual_keycode: Some(key), .. }), .. } => {
                    let io = self.imgui.io_mut();
                    let pressed = state == ElementState::Pressed;
                    io.keys_down[key as usize] = pressed;
                    match key {
                        VirtualKeyCode::LShift | VirtualKeyCode::RShift => io.key_shift = pressed,
                        VirtualKeyCode::LControl | VirtualKeyCode::RControl => io.key_ctrl = pressed,
                        VirtualKeyCode::LAlt | VirtualKeyCode::RAlt => io.key_alt = pressed,
                        VirtualKeyCode::LWin | VirtualKeyCode::RWin => io.key_super = pressed,
                        _ => (),
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
                                let state_arc = state.clone();
                                let thread_count_clone = self.chunk_meshing_threads.clone();
                                let info = self.renderer.info.clone();
                                thread::spawn(move || {
                                    let mut chunk_lock = chunk_arc.write().unwrap();

                                    // TODO: fix this
                                    (*chunk_lock).generate_mesh(&info);

                                    //let chunk_center = Chunk::chunk_pos_to_center_ws(chunk_pos);
                                    //let dist = Point3::distance(Point3::new(chunk_center.0, chunk_center.1, chunk_center.2), player_pos);
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
            self.chunk_metrics.generated = 0;

            for (_, (_, state))  in chunks.iter() {
                let status = state.load(Ordering::Relaxed);
                if status != CHUNK_STATE_GENERATING {
                    self.chunk_metrics.generated += 1;
                }
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
            self.chunk_metrics.meshed = 0;
            let chunks = self.dimension_registry.get(0).unwrap().chunks.read().unwrap();
            let frustum = view_to_frustum(self.player.pitch, self.player.yaw, self.player.camera.fov, 4.0/3.0, 1.0, 10000.0);

            for (pos, (chunk, state)) in chunks.iter() {
                let aabb_min = Point3::new(pos.0 as f32, pos.1 as f32, pos.2 as f32) * CHUNK_SIZE_F32 - self.player.position.to_vec();
                let aabb_max = aabb_min + Vector3::new(CHUNK_SIZE_F32, CHUNK_SIZE_F32, CHUNK_SIZE_F32);
                let status = state.load(Ordering::Relaxed);
                let is_ready = status == CHUNK_STATE_CLEAN;
                if is_ready {
                    self.chunk_metrics.meshed += 1;
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
            let mut queue_lock = self.renderer.info.render_queues.write().unwrap();
            queue_lock.occluders.vertex_group = Arc::new(VertexGroup::new(occlusion_verts.into_iter(), occlusion_indices.into_iter(), 0, self.renderer.info.device.clone()));
        }

        self.frame_metrics.end_game();

        {
            self.chunk_metrics.drawing = self.renderer.info.render_queues.read().unwrap().meshes.len() as u32;
        }

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

        // TODO: fix imgui mutable borrow issue?
        let metrics = self.frame_metrics.last_frame_text.clone();
        let chunk_metrics = self.chunk_metrics.clone();
        let pos = self.player.position;
        let tonemap_info = self.renderer.info.tonemapping_info.clone();
        let mut histogram_values = [0f32; 128];
        for (i, u) in self.renderer.info.histogram_compute.lock().bins.iter().enumerate() {
            histogram_values[i] = *u as f32;
        }
        let mut exp_adj_slider_val = tonemap_info.exposure_adjustment;
        //let dimensions = [self.renderer.info.dimensions[0] as f32, self.renderer.info.dimensions[1] as f32];
        let mut ui = self.imgui.frame();
        let mut run_ui = |ui: &mut imgui::Ui| {
//            {
//                let tokens = vec![
//                    ui.push_style_var(StyleVar::WindowPadding([0.0, 0.0])),
//                    ui.push_style_var(StyleVar::WindowRounding(0.0)),
//                    ui.push_style_var(StyleVar::WindowBorderSize(0.0)),
//                ];
//                imgui::Window::new(im_str!("DockSpace"))
//                    .position([0.0, 0.0], Condition::Always)
//                    .size(dimensions, Condition::Always)
//                    .flags(WindowFlags::NO_DOCKING | WindowFlags::NO_TITLE_BAR | WindowFlags::NO_COLLAPSE | WindowFlags::NO_RESIZE
//                           | WindowFlags::NO_MOVE | WindowFlags::NO_BRING_TO_FRONT_ON_FOCUS | WindowFlags::NO_NAV_FOCUS)
//                    .build(&ui, || {
//                        let dockspace_id = imgui::get_id_str(im_str!("DockSpace"));
//                        match dockspace_id {
//                            imgui::Id::Int(i) => { println!("{}", i); },
//                            _ => {}
//                        }
//                        imgui::dock_space(dockspace_id, [0.0, 0.0], imgui::ImGuiDockNodeFlags::PASSTHRU_CENTRAL_NODE);
//                    });
//                for t in tokens { t.pop(ui) }
//            }

            imgui::Window::new(im_str!("Metrics"))
                .size([200.0, 350.0], Condition::FirstUseEver)
                .position([0.0, 0.0], Condition::FirstUseEver)
                .flags(WindowFlags::empty())
                .build(&ui, || {
                    ui.text(ImString::new(metrics.fps.clone()));
                    ui.text(ImString::new(metrics.game_time.clone()));
                    ui.text(ImString::new(metrics.draw_time.clone()));
                    ui.text(ImString::new(metrics.gpu_time.clone()));
                    ui.separator();
                    ui.text(ImString::new(format!("Chunks generated: {}", chunk_metrics.generated)));
                    ui.text(ImString::new(format!("Chunks meshed:    {}", chunk_metrics.meshed)));
                    ui.text(ImString::new(format!("Chunks drawing:   {}", chunk_metrics.drawing)));
                    ui.separator();
                    ui.text(ImString::new(format!("Scene luma avg:  {:>5.2}", tonemap_info.avg_scene_luma)));
                    ui.text(ImString::new(format!("Scene EV100:     {:>5.2}", tonemap_info.scene_ev100)));
                    ui.text(ImString::new(format!("Exposure:        {:>5.2}", tonemap_info.exposure)));
                    ui.text(ImString::new(format!("Exposure adjust: {:>5.2}", tonemap_info.exposure_adjustment)));
                    ui.drag_float(im_str!("Exp Adj"), &mut exp_adj_slider_val)
                        .min(-2.0)
                        .max(2.0)
                        .speed(0.05)
                        .build();
                    ui.text(ImString::new(format!("Adjust speed:    {:>5.2}", tonemap_info.adjust_speed)));
                    ui.separator();
                    ui.text(ImString::new(format!("Position: {:3.1}, {:3.1}, {:3.1}", pos.x, pos.y, pos.z)));
                    ui.text(ImString::new(format!("Visualization: {}", debug_vis_text)));
                });

            const HISTOGRAM_BAR_WIDTH: f32 = 1190.0 / 128.0;

            imgui::Window::new(im_str!("Histogram"))
                .size([1200.0, 180.0], Condition::FirstUseEver)
                .position([0.0, 590.0], Condition::FirstUseEver)
                .flags(WindowFlags::empty())
                .build(&ui, || {
                    use std::f32::consts::E;

                    ui.plot_histogram(&ImString::new(format!("histogram")), &histogram_values)
                        .graph_size([1190.0, 145.0])
                        .scale_min(0.0)
                        .scale_max(20000.0)
                        .build();

                    let mut values = [0f32; 128];
                    let a = tonemap_info.hist_low_percentile_bin;
                    let b = tonemap_info.hist_high_percentile_bin;
                    let h = (a + b) / 2.0;
                    for (i, v) in values.iter_mut().enumerate() {
                        let x = i as f32;
                        let exp = ((x - h) / (b - a)) * -2.0;
                        let value = 1.0 / (1.0 + E.powf(exp));
                        *v = value;
                    }
                    let t = ui.push_style_color(StyleColor::FrameBg, [0.0, 0.0, 0.0, 0.0]);
                    let t2 = ui.push_style_color(StyleColor::PlotLines, [1.0, 1.0, 1.0, 1.0]);
                    ui.set_cursor_pos([0.0, 25.0]);
                    ui.plot_lines(&ImString::new(format!("histogram")), &values)
                        .graph_size([1190.0, 145.0])
                        .scale_min(0.0)
                        .scale_max(1.0)
                        .build();
                    t2.pop(ui);
                    t.pop(ui);

                    let draw_list = ui.get_window_draw_list();
                    let [x, y] = ui.cursor_screen_pos();
                    let middle = (tonemap_info.hist_low_percentile_bin + tonemap_info.hist_high_percentile_bin) / 2.0;
                    draw_list.add_line::<[f32; 4]>([x + (tonemap_info.hist_low_percentile_bin * HISTOGRAM_BAR_WIDTH), y - 145.0],
                                                   [x + (tonemap_info.hist_low_percentile_bin * HISTOGRAM_BAR_WIDTH), y],
                                                   [0.5, 0.6, 1.0, 1f32].into())
                        .thickness(2.0)
                        .build();
                    draw_list.add_line::<[f32; 4]>([x + (middle * HISTOGRAM_BAR_WIDTH), y - 145.0],
                                                   [x + (middle * HISTOGRAM_BAR_WIDTH), y],
                                                   [0.5, 1.0, 0.6, 1f32].into())
                        .thickness(2.0)
                        .build();
                    draw_list.add_line::<[f32; 4]>([x + (tonemap_info.hist_high_percentile_bin as f32 * HISTOGRAM_BAR_WIDTH), y - 145.0],
                                                   [x + (tonemap_info.hist_high_percentile_bin as f32 * HISTOGRAM_BAR_WIDTH), y],
                                                   [1.0, 0.6, 0.5, 1f32].into())
                        .thickness(2.0)
                        .build();
                });
        };
        run_ui(&mut ui);

        self.renderer.info.tonemapping_info.exposure_adjustment = exp_adj_slider_val;

        match self.renderer.draw(&self.player.camera, dt_clamped as f32, self.player.get_transform()) {
            Ok(img_future) => {
                self.renderer.draw_imgui(ui);
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