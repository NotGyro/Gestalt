//! Voxel metaverse "game" you can have some fun in.
#![feature(drain_filter)]
#![feature(adt_const_params)]
#![feature(string_remove_matches)]

#[macro_use]
pub mod common;
pub mod client;
pub mod entity;
pub mod script;
pub mod world;


use hashbrown::HashSet;
use log::{LevelFilter, info, error};
use mlua::{MultiValue, LuaOptions};
use winit::event::{VirtualKeyCode, ElementState};
use std::{fs::File, sync::Arc, time::Instant};

use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger, ConfigBuilder};

use glam::{Vec3, Quat};

use client::camera as camera;

fn vertex(pos: [f32; 3]) -> Vec3 {
    Vec3::from(pos)
}

fn create_mesh() -> rend3::types::Mesh {
    let vertex_positions = [
        // far side (0.0, 0.0, 1.0)
        vertex([-1.0, -1.0, 1.0]),
        vertex([1.0, -1.0, 1.0]),
        vertex([1.0, 1.0, 1.0]),
        vertex([-1.0, 1.0, 1.0]),
        // near side (0.0, 0.0, -1.0)
        vertex([-1.0, 1.0, -1.0]),
        vertex([1.0, 1.0, -1.0]),
        vertex([1.0, -1.0, -1.0]),
        vertex([-1.0, -1.0, -1.0]),
        // right side (1.0, 0.0, 0.0)
        vertex([1.0, -1.0, -1.0]),
        vertex([1.0, 1.0, -1.0]),
        vertex([1.0, 1.0, 1.0]),
        vertex([1.0, -1.0, 1.0]),
        // left side (-1.0, 0.0, 0.0)
        vertex([-1.0, -1.0, 1.0]),
        vertex([-1.0, 1.0, 1.0]),
        vertex([-1.0, 1.0, -1.0]),
        vertex([-1.0, -1.0, -1.0]),
        // top (0.0, 1.0, 0.0)
        vertex([1.0, 1.0, -1.0]),
        vertex([-1.0, 1.0, -1.0]),
        vertex([-1.0, 1.0, 1.0]),
        vertex([1.0, 1.0, 1.0]),
        // bottom (0.0, -1.0, 0.0)
        vertex([1.0, -1.0, 1.0]),
        vertex([-1.0, -1.0, 1.0]),
        vertex([-1.0, -1.0, -1.0]),
        vertex([1.0, -1.0, -1.0]),
    ];

    let index_data: &[u32] = &[
        0, 1, 2, 2, 3, 0, // far
        4, 5, 6, 6, 7, 4, // near
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // top
        20, 21, 22, 22, 23, 20, // bottom
    ];

    rend3::types::MeshBuilder::new(vertex_positions.to_vec())
        .with_indices(index_data.to_vec())
        .build()
}

#[allow(unused_must_use)]
fn main() {
    let mut log_config_builder = ConfigBuilder::default();
    log_config_builder.set_target_level(LevelFilter::Error);
    let log_config = log_config_builder.build();

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Warn,
            log_config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Debug,
            log_config.clone(),
            File::create("latest.log").unwrap(),
        ),
    ]).unwrap();
    
    info!("Starting Gestalt");

    info!("sizeof Mat4: {}", std::mem::size_of::<glam::f32::Mat4>());
    info!("sizeof component parts: {}", std::mem::size_of::<glam::f32::Vec3>() + std::mem::size_of::<glam::f32::Quat>() + std::mem::size_of::<glam::f32::Vec3>()); 
    info!("sizeof Vec3: {}", std::mem::size_of::<glam::f32::Vec3>());

    let event_loop = winit::event_loop::EventLoop::new();
    let window = winit::window::WindowBuilder::new()
        .with_title("Gestalt")
        .build(&event_loop)
        .unwrap();
    
    let window_size = window.inner_size();

    // Create the Instance, Adapter, and Device. We can specify preferred backend,
    // device name, or rendering mode. In this case we let rend3 choose for us.
    let iad = pollster::block_on(rend3::create_iad(None, None, None)).unwrap();
    
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
        Some(window_size.width as f32 / window_size.height as f32),
    )
    .unwrap();
    
    // Create the pbr pipeline with the same internal resolution and 4x multisampling
    let mut routine = rend3_pbr::PbrRenderRoutine::new(
        &renderer,
        rend3_pbr::RenderTextureOptions {
            resolution: glam::UVec2::new(window_size.width, window_size.height),
            samples: rend3_pbr::SampleCount::Four,
        },
        format,
    );

    // Create mesh and calculate smooth normals based on vertices
    let mesh = create_mesh();

    // Add mesh to renderer's world.
    //
    // All handles are refcounted, so we only need to hang onto the handle until we make an object.
    let mesh_handle = renderer.add_mesh(mesh);

    // Add PBR material with all defaults except a single color.
    let material = rend3_pbr::material::PbrMaterial {
        albedo: rend3_pbr::material::AlbedoComponent::Value(glam::Vec4::new(0.0, 0.5, 0.5, 1.0)),
        ..rend3_pbr::material::PbrMaterial::default()
    };
    let material_handle = renderer.add_material(material);

    // Combine the mesh and the material with a location to give an object.
    let object = rend3::types::Object {
        mesh: mesh_handle,
        material: material_handle,
        transform: glam::Mat4::IDENTITY,
    };

    let cube_yaw_speed = 1.0f32;
    let mut cube_yaw = 0.0f32;

    // Creating an object will hold onto both the mesh and the material
    // even if they are deleted.
    //
    // We need to keep the object handle alive.
    let object_handle = renderer.add_object(object);

    let view_location = glam::Vec3::new(3.0, 3.0, -5.0);
    let mut camera = camera::Camera::new(view_location);

    camera.sensitivity = 1.0;
    camera.speed = 0.5;

    // Set camera's location
    renderer.set_camera_data(rend3::types::Camera {
        projection: rend3::types::CameraProjection::Projection { vfov: 90.0, near: 0.1 },
        view: camera.get_view_matrix(),
    });

    let _directional_handle = renderer.add_directional_light(rend3::types::DirectionalLight {
        color: glam::Vec3::new(0.2, 0.8, 1.0),
        intensity: 10.0,
        // Direction will be normalized
        direction: glam::Vec3::new(-1.0, -4.0, 2.0),
        distance: 400.0,
    });

    let mut previous_position: Option<winit::dpi::PhysicalPosition<f64>> = None;
    let mut current_down = HashSet::new();

    let mut prev_frame_time = Instant::now();

    let first_frame_time = Instant::now();

    event_loop.run(move |event, _, control| {
        let elapsed_time = prev_frame_time.elapsed();
        prev_frame_time = Instant::now();
        let elapsed_secs = elapsed_time.as_secs_f32();
        cube_yaw = cube_yaw + (cube_yaw_speed * elapsed_secs);
        
        let (scale, _rotation, translation) = glam::Mat4::IDENTITY.to_scale_rotation_translation();
        let new_scale = scale * 1.2 + ( (first_frame_time.elapsed().as_secs_f32() * 2.0).sin() / 4.0);
        let new_rotation = Quat::from_rotation_y(cube_yaw);

        let new_transform = glam::Mat4::from_scale_rotation_translation(new_scale, new_rotation, translation);

        renderer.set_object_transform(&object_handle, new_transform);

        for dir in current_down.iter() {
            camera.key_interact(*dir);
        }

        renderer.set_camera_data(rend3::types::Camera {
            projection: rend3::types::CameraProjection::Projection { vfov: 90.0, near: 0.1 },
            view: camera.get_view_matrix(),
        });
        
        match event {
            winit::event::Event::WindowEvent{ 
                event: winit::event::WindowEvent::CursorMoved{
                    position,
                    ..
                },
                ..
            } => {
                if let Some(pos) = previous_position {
                    let diff_x = pos.x - position.x;
                    let diff_y = pos.y - position.y;
                    camera.mouse_interact(diff_x as f32, -diff_y as f32);
                }
                previous_position = Some(position);
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
                    let dir_maybe = input.virtual_keycode.map(|k| camera::Directions::from_key(k)).flatten();
                    if let Some(dir) = dir_maybe { 
                        current_down.insert(dir);
                    }
                }
                else if input.state == ElementState::Released {
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
                event: winit::event::WindowEvent::Resized(size),
                ..
            } => {
                let size = glam::UVec2::new(size.width, size.height);
                // Reconfigure the surface for the new size.
                rend3::configure_surface(
                    &surface,
                    &renderer.device,
                    format,
                    glam::UVec2::new(size.x, size.y),
                    rend3::types::PresentMode::Mailbox,
                );
                // Tell the renderer about the new aspect ratio.
                renderer.set_aspect_ratio(size.x as f32 / size.y as f32);
                // Resize the internal buffers to the same size as the screen.
                routine.resize(
                    &renderer,
                    rend3_pbr::RenderTextureOptions {
                        resolution: size,
                        samples: rend3_pbr::SampleCount::One,
                    },
                );
            }
            // Render!
            winit::event::Event::MainEventsCleared => {
                // Get a frame
                let frame = rend3::util::output::OutputFrame::from_surface(&surface).unwrap();
                // Dispatch a render!
                let _stats = renderer.render(&mut routine, (), frame.as_view());
                // Present the frame on screen
                frame.present();
            }
            // Other events we don't care about
            _ => {}
        }
    });
    
    let lua_stdlibs = mlua::StdLib::BIT | mlua::StdLib::STRING | mlua::StdLib::TABLE | mlua::StdLib::IO | mlua::StdLib::OS | mlua::StdLib::JIT | mlua::StdLib::PACKAGE;
    let vm = mlua::Lua::new_with(lua_stdlibs, LuaOptions::default()).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));
}