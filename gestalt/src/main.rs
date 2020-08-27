//! Voxel metaverse "game" you can have some fun in.

#![feature(drain_filter)]
#![feature(seek_convenience)]

#[macro_use] extern crate hemlock;

extern crate anyhow;
#[macro_use] extern crate arr_macro;
extern crate bincode;
extern crate crossbeam_channel;
#[macro_use] extern crate custom_error;
#[macro_use] extern crate enum_dispatch;
#[macro_use] extern crate glium;
extern crate glutin;
extern crate hashbrown;
#[macro_use] extern crate lazy_static;
//extern crate linear_map;
extern crate log;
extern crate nalgebra as na;
extern crate num;
extern crate parking_lot;
extern crate rand;
extern crate rusty_v8;
extern crate semver;
extern crate serde;
extern crate ustr;
extern crate uuid;

use cgmath::{Angle, Matrix4, Vector3, /*Vector4,*/ Point3, InnerSpace, Rotation, Rotation3, Quaternion, Rad};

use glium::backend::glutin::Display;
use glutin::window::*;
use glutin::event::*;
use glutin::event_loop::*;
use glutin::ContextBuilder;
//use glutin::dpi::LogicalPosition;
//use glutin::dpi::PhysicalPosition;
use glium::Surface;

use logger::hemlock_scopes;

use serde::{Serialize, Deserialize};
use std::fs::OpenOptions;
use std::fs::File;
use std::io::prelude::*;
use std::error::Error;
//use std::time::Duration;
use core::ops::Neg;
use num::Zero;

use ron::ser::{to_string_pretty, PrettyConfig};
use ron::de::from_reader;
//use rusty_v8 as v8;
use ustr::*;
use std::time::*;

#[macro_use] pub mod util;
pub mod client;
pub mod world;

use crate::util::voxelmath::*;
use crate::world::*;

/// The main purpose of the Logger module is to define our Hemlock scopes. 
/// It also contains a https://crates.io/crates/log proxy into Hemlock, so anything 
/// logged using that crate's macros will show up as coming from the "Library" scope.
pub mod logger;


#[derive(Clone, Debug, Serialize, Deserialize)]
struct ClientConfig {
    pub resolution: (u32, u32),
}

fn make_display(conf : ClientConfig) -> std::result::Result<(Display, EventLoop<()>), Box<dyn Error>> {
    let el = EventLoop::new();

    let wb = WindowBuilder::new()
        .with_title("Gestalt")
        .with_inner_size(glutin::dpi::LogicalSize{ width: conf.resolution.0, height: conf.resolution.1});

    let cb = ContextBuilder::new().with_depth_buffer(24);
    
    Ok( (Display::new(wb, cb, &el)?, el))
}

impl Default for ClientConfig {
    fn default() -> Self { ClientConfig {resolution: (800,600)} }
}

#[allow(unused_must_use)]
fn main() -> anyhow::Result<()> {
    let air = ustr("air");
    let stone = ustr("stone");
    let dirt = ustr("dirt");
    let grass = ustr("grass");

    let mut space = Space::new();

    for x in -2 .. 2 {
        for y in -2 .. 2 {
            for z in -2 .. 2 {
                space.load_or_gen_chunk(vpos!(x,y,z)).unwrap();
            }
        }
    }

    match logger::init_logger() {
        Ok(_) => info!(Core, "Logger initialized."),
        Err(e) => panic!("Could not initialize logger! Reason: {}", e),
    };

    let client_config_filename = "client.ron";

    let client_config_result = OpenOptions::new().read(true)
                                                .write(true)
                                                .truncate(false)
                                                .open(client_config_filename);
    let mut create_conf_flag = false;
    let client_config: ClientConfig = match client_config_result {
        Ok(file) => {
            match from_reader(file) {
                Ok(x) => x,
                Err(e) => {
                    error!(Core, "Failed to load client config: {}", e);
                    error!(Core, "Using default client config values.");
                    ClientConfig::default()
                }
            }
        }, 
        Err(e) => {
            warn!(Core, "Failed to open {} (client config file): {}", client_config_filename, e);
            warn!(Core, "Using default client config values.");
            create_conf_flag = true;
            ClientConfig::default()
        }
    };

    // Client.ron wasn't there, create it. 
    if create_conf_flag { 
        info!(Core, "Creating {}, since it wasn't there before.", client_config_filename);
        let mut f = File::create(client_config_filename)?;
        let pretty = PrettyConfig::new().with_depth_limit(16)
                                        .with_enumerate_arrays(true);
        let s = to_string_pretty(&client_config, pretty).expect("Serialization failed");
        f.write_all(s.as_bytes())?;
        f.flush()?;
        drop(f);
    }

    //---- Set up window ----

    let (display, event_loop) = make_display(client_config.clone()).unwrap();

    let mut vshaderfile = File::open("vertexshader.glsl").unwrap();
    let mut fshaderfile = File::open("fragmentshader.glsl").unwrap();
    let mut vertex_shader_src = String::new();
    let mut fragment_shader_src = String::new();
    vshaderfile.read_to_string(&mut vertex_shader_src).unwrap();
    fshaderfile.read_to_string(&mut fragment_shader_src).unwrap();

    let program = glium::Program::from_source(&display, vertex_shader_src.as_ref(), fragment_shader_src.as_ref(), None).unwrap();

    let params = glium::DrawParameters {
        depth: glium::Depth {
            test: glium::draw_parameters::DepthTest::IfLess,
            write: true,
            .. Default::default()
        },
        backface_culling : glium::draw_parameters::BackfaceCullingMode::CullClockwise,
        .. Default::default()
    };
    
    //---- Set up our camera ----
    
	let mut camera_pos : Point3<f32> = Point3 {x : 0.0, y : 0.0, z : 10.0}; 
	
	let mouse_sensitivity : f32 = 2.0;
	let move_speed : f32 = 16.0;
	let mut horz_angle : Rad<f32> = Rad::zero();
	let mut vert_angle : Rad<f32> = Rad::zero();

    //let mut perspective_matrix : cgmath::Matrix4<f32> = cgmath::perspective(cgmath::deg(45.0), 1.333, 0.0001, 100.0);
    //let mut view_matrix : Matrix4<f32> = Matrix4::look_at(view_eye, view_center, view_up);
    //let model_matrix : Matrix4<f32> = Matrix4::from_scale(1.0);
    
    let mut mouse_prev_x : i32 = 0;
    let mut mouse_prev_y : i32 = 0;
    
    
    let perspective : cgmath::PerspectiveFov<f32> = cgmath::PerspectiveFov { fovy : cgmath::Rad {0 : 1.22173 }, aspect : 4.0 / 3.0, near : 0.1, far : 100.0}; 
    
    //---- Set up our texture(s) and chunk verticies ----

    let mut renderer : client::renderer::Renderer = client::renderer::Renderer::new();
    let air_art = client::tileart::TileArtSimple { texture_name : ustr(""), visible: false };
    let stone_art = client::tileart::TileArtSimple { texture_name : ustr("teststone.png"), visible: true };
    let dirt_art = client::tileart::TileArtSimple { texture_name : ustr("testdirt.png"), visible: true };
    let grass_art = client::tileart::TileArtSimple { texture_name : ustr("testgrass.png"), visible: true };

    renderer.texture_manager.associate_tile(&display, air, air_art);
    renderer.texture_manager.associate_tile(&display, stone, stone_art);
    renderer.texture_manager.associate_tile(&display, dirt, dirt_art);
    renderer.texture_manager.associate_tile(&display, grass, grass_art);

    renderer.texture_manager.rebuild(&display);

    for chunk in space.get_loaded_chunks() {
        info!(Mesher, "Forcing mesh of {}...", chunk);
        if chunk.y > 0 { 
            info!(Mesher, "- (This should be all air!)");
        }
        let start = Instant::now();
        renderer.force_mesh(&space, chunk, &display);
        let elapsed = start.elapsed();
        info!(Mesher, "- Meshing {} took {} microseconds", chunk, elapsed.as_micros());
    }

    //---- Some movement stuff ----

    let mut w_down : bool = false;
    let mut a_down : bool = false;
    let mut s_down : bool = false;
    let mut d_down : bool = false;

    /*let mut set_action : bool = false;
    let mut delete_action : bool = false;

    
    let screen_center_x : i32 = client_config.resolution.0 as i32 /2;
    let screen_center_y : i32 = client_config.resolution.1 as i32 /2;*/
    
    let mut mouse_first_moved : bool = false;
    let mut grabs_mouse : bool = true;
    //---- A mainloop ----
    let mut lastupdate = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        
        *control_flow = ControlFlow::Poll;

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit
                },
                /*
                WindowEvent::CursorMoved{device_id, position, modifiers} => {
                    //println!("Mouse moved ({}, {})", position.x, position.y);
                    let x = position.x;
                    let y = position.y;
                    if grabs_mouse {
                        if !mouse_first_moved {
                            //horz_angle.0 += ((x - mouse_prev_x as f64) as f32) * (mouse_sensitivity * lastupdate.elapsed().as_secs_f32());
                            //vert_angle.0 += ((y - mouse_prev_y as f64) as f32) * (mouse_sensitivity * lastupdate.elapsed().as_secs_f32());
                            //println!("Angle is now ({}, {})", horz_angle.0, vert_angle.0);
                        }
                        else {
                            mouse_first_moved = true;
                        }
                        //mouse_prev_x = x as i32;
                        //mouse_prev_y = y as i32;
                        //let gl_window = display.gl_window();
                        //let window = gl_window.window();
                        //let window_position = window.inner_position().unwrap(); 
                        //window.set_cursor_position(LogicalPosition{x: window_position.x + (x as i32), y: window_position.y + (y as i32)}).unwrap();
                    }
                },*/
                WindowEvent::KeyboardInput{device_id, input, is_synthetic} => {
                    if !is_synthetic {
                        match input.virtual_keycode {
                            Some(VirtualKeyCode::Escape) => {
                                *control_flow = ControlFlow::Exit;
                            },
                            Some(VirtualKeyCode::Tab) => grabs_mouse = false,
                            Some(VirtualKeyCode::W) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => w_down = true,
                                    glutin::event::ElementState::Released => w_down = false,
                                }
                            },
                            Some(VirtualKeyCode::A) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => a_down = true,
                                    glutin::event::ElementState::Released => a_down = false,
                                }
                            },
                            Some(VirtualKeyCode::S) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => s_down = true,
                                    glutin::event::ElementState::Released => s_down = false,
                                }
                            },
                            Some(VirtualKeyCode::D) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => d_down = true,
                                    glutin::event::ElementState::Released => d_down = false,
                                }
                            },
                            _ => {}
                        }
                    }
                }
                _ => (),
            },
            _ => (),
        }
        horz_angle = horz_angle.normalize();
        vert_angle = vert_angle.normalize();
        
        //Remember: Z is our vertical axis here.
        let yaw : Quaternion<f32> = Quaternion::from_angle_z(horz_angle.neg());
        let pitch : Quaternion<f32> = Quaternion::from_angle_y(vert_angle);
        let rotation = (yaw * pitch).normalize();

        let mut forward : Vector3<f32> = Vector3::new(0.0, 0.0, -1.0);
        let mut right : Vector3<f32> = Vector3::new(-1.0, 0.0, 0.0);
        forward = rotation.rotate_vector(forward);
        right = rotation.rotate_vector(right);
        let up = forward.cross( right ).neg();
        
        //Movement
        if w_down {
            camera_pos += forward * (lastupdate.elapsed().as_secs_f32() * move_speed);
        }
        if d_down {
            camera_pos += right * (lastupdate.elapsed().as_secs_f32() * move_speed);
        }
        if s_down {
            camera_pos += (forward * (lastupdate.elapsed().as_secs_f32() * move_speed)).neg();
        }
        if a_down {
            camera_pos += (right * (lastupdate.elapsed().as_secs_f32() * move_speed)).neg();
        }

        //Drawing
        let view_matrix = Matrix4::look_at(camera_pos, camera_pos + forward, up);
        let perspective_matrix = Matrix4::from(perspective);
        
        let before_remesh = Instant::now();
        renderer.process_remesh(&space, &display);
        let remesh_time = before_remesh.elapsed().as_secs_f32();
        if remesh_time > 0.01 {
            info!(Mesher, "Took {} seconds to remesh chunks.", remesh_time);
        }

        let mut target = display.draw();
        target.clear_color_and_depth((0.43, 0.7, 0.82, 1.0), 1.0);
        renderer.draw(perspective_matrix, view_matrix, &mut target, &program, &params);
        target.finish().unwrap();
        lastupdate = Instant::now();
    });

    //Ok(())
}