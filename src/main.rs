#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_parens)]
#![allow(unused_assignments)]
#![allow(unused_mut)]
#![allow(unused_variables)]
#![allow(unused_must_use)]

//#![feature(collections)]
pub mod util;
pub mod voxel;
pub mod client;

#[macro_use] extern crate glium;
#[macro_use] extern crate cgmath;
#[macro_use] extern crate serde_derive;
#[macro_use] extern crate string_cache;
#[macro_use] extern crate lazy_static;
extern crate num;

extern crate time;
extern crate image;

use time::*;

use std::vec::Vec;
use voxel::voxelstorage::*;
use voxel::voxelarray::*;
use voxel::vspalette::*;
use voxel::material::*;
use voxel::voxelspace::*;

use util::voxelutil::*;

use client::simplerenderer::*;
use client::materialart::MatArtSimple;

use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Cursor;
use std::io;
use std::cmp;
use std::f32::consts::*;
use std::ops::Neg;
use std::collections::HashMap;
use std::collections::HashSet;
use num::Zero;

use cgmath::{Angle, Matrix4, Vector3, Vector4, Point3, InnerSpace, Rotation, Rotation3, Quaternion, Rad};

use glium::{DisplayBuild, Surface};
use glium::glutin;
use glium::glutin::Event;
use glium::glutin::VirtualKeyCode;
use glium::glutin::CursorState;
use glium::texture::Texture2dArray;
use glium::backend::glutin_backend::GlutinFacade;

// This function only gets compiled if the target OS is linux
#[cfg(target_os = "linux")]
fn are_you_on_linux() {
        println!("You are running linux!")
}

// And this function only gets compiled if the target OS is *not* linux
#[cfg(not(target_os = "linux"))]
fn are_you_on_linux() {
        println!("You are *not* running linux!")
}

fn make_display(screen_width : u32, screen_height : u32) -> GlutinFacade {
    are_you_on_linux();
    let display_maybe = glutin::WindowBuilder::new()
        .with_dimensions(screen_width, screen_height)
        .with_depth_buffer(24)
        .build_glium();
    let display = match display_maybe {
        Ok(v) => v,
        Err(e) => {
            println!("Error while creating display in main.rs:");
            println!("{}", e); //e is a glium::GliumCreationError<glutin::CreationError>
            match e {
                glium::GliumCreationError::IncompatibleOpenGl(s) => println!("{}", s),
                glium::GliumCreationError::BackendCreationError(ee) => {
                    //ee should be a glutin::CreationError.
                    match ee {
                        glutin::CreationError::OsError(s) => println!("OS Error: {}", s),
                        glutin::CreationError::NotSupported => println!("BackendCreationError is \"NotSupported.\""),
                        glutin::CreationError::NoBackendAvailable(eee) => {
                            println!("No Backend error: {}", eee);
                            println!("{}", eee.cause().unwrap());
                        },
                        glutin::CreationError::RobustnessNotSupported => println!("Robustness not supported."),
                        glutin::CreationError::OpenGlVersionNotSupported => println!("OpenGL version not supported."),
                        glutin::CreationError::NoAvailablePixelFormat => println!("No available pixel format."),
                    };
                },
            };
            panic!();
        },
    };
    return display;
}

fn main() {

    println!("{:?}", std::env::current_exe());
    
    let mat_idx : MaterialIndex = MaterialIndex::new();

    let air_id : MaterialID = mat_idx.for_name(&String::from("test.air"));
    let stone_id : MaterialID = mat_idx.for_name(&String::from("test.stone"));
    let dirt_id : MaterialID = mat_idx.for_name(&String::from("test.dirt"));
    let grass_id : MaterialID = mat_idx.for_name(&String::from("test.grass"));
    
    let mut space = VoxelSpace::new(mat_idx);
    
    let lower_x : i32 = -2;
    let upper_x : i32 = 2;
    let lower_y : i32 = -2;
    let upper_y : i32 = 2;
    let lower_z : i32 = -2;
    let upper_z : i32 = 2;

    for x in lower_x .. upper_x {
        for y in lower_y .. upper_y {
            for z in lower_z .. upper_z {
                space.load_or_create_c(x,y,z);
            }
        }
    }
    
    //---- Set up window ----
    let screen_width : u32 = 1024;
    let screen_height : u32 = 768;
    let display = make_display(screen_width, screen_height);
    let mut keeprunning = true;
    let mut window = display.get_window().unwrap();
    //window.set_cursor_state(glutin::CursorState::Grab);

    //---- Set up screen and some basic graphics stuff ----
    let mut vshaderfile = File::open("vertexshader.glsl").unwrap();
    let mut fshaderfile = File::open("fragmentshader.glsl").unwrap();
    let mut vertex_shader_src = String::new();
    let mut fragment_shader_src = String::new();
    vshaderfile.read_to_string(&mut vertex_shader_src);
    fshaderfile.read_to_string(&mut fragment_shader_src);
    
    println!(line!());
    let program = glium::Program::from_source(&display, vertex_shader_src.as_ref(), fragment_shader_src.as_ref(), None).unwrap();

    

    let mut t: f32 = -0.5;
    
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
	
	let mouse_sensitivity : f32 = 4.0;
	let move_speed : f32 = 16.0;
	let mut horz_angle : Rad<f32> = Rad::zero();
	let mut vert_angle : Rad<f32> = Rad::zero();

    //let mut perspective_matrix : cgmath::Matrix4<f32> = cgmath::perspective(cgmath::deg(45.0), 1.333, 0.0001, 100.0);
    //let mut view_matrix : Matrix4<f32> = Matrix4::look_at(view_eye, view_center, view_up);
    let mut model_matrix : Matrix4<f32> = Matrix4::from_scale(1.0);
    
    let mut mouse_prev_x : i32 = 0;
    let mut mouse_prev_y : i32 = 0;
    
    
    let perspective : cgmath::PerspectiveFov<f32> = cgmath::PerspectiveFov { fovy : cgmath::Rad {s : 1.22173 }, aspect : 4.0 / 3.0, near : 0.1, far : 100.0}; 
    
    //---- Set up our texture(s) and chunk verticies ----
    let stone_art = MatArtSimple { texture_name : String::from("teststone.png") };
    let dirt_art = MatArtSimple { texture_name : String::from("testdirt.png") };
    let grass_art = MatArtSimple { texture_name : String::from("testgrass.png") };

    let mut mat_art_manager = MatArtMapping::new();

    let mut renderer : SimpleVoxelMesher = SimpleVoxelMesher::new();

    mat_art_manager.insert(grass_id.clone(), grass_art.clone());
    mat_art_manager.insert(stone_id.clone(), stone_art.clone());
    mat_art_manager.insert(dirt_id.clone(), dirt_art.clone());

    for chunk in space.get_regions() { 
        renderer.force_mesh(&space, &display, chunk, &mat_art_manager);
    }

    //---- Some movement stuff ----
    
    let mut w_down : bool = false;
    let mut a_down : bool = false;
    let mut s_down : bool = false;
    let mut d_down : bool = false;

    let mut set_action : bool = false;
    let mut delete_action : bool = false;
    let mut pick_action : bool = false;
    let mut current_block : MaterialID = MaterialID::from_name(&String::from("test.stone"));
    
    
    let screen_center_x : i32 = screen_width as i32 /2;
    let screen_center_y : i32 = screen_height as i32 /2;
    
    let mut mouse_first_moved : bool = false;
    let mut grabs_mouse : bool = true;
    //---- A mainloop ----
    let mut lastupdate = precise_time_s();
    let mut elapsed = 0.01 as f32;
    while keeprunning {
        
        for ev in display.poll_events() {
            match ev {
                Event::Closed => {keeprunning = false},   // The window has been closed by the user, external to our game (hitting x in corner, for example)
                Event::MouseMoved(x, y) => {
                    if(grabs_mouse) {
                    if mouse_first_moved {
                        horz_angle.s += ((x - screen_center_x) as f32) * (mouse_sensitivity * elapsed);
                        vert_angle.s += ((y - screen_center_y) as f32) * (mouse_sensitivity * elapsed);
                    }
                    else {
                        mouse_first_moved = true;
                    }
                    mouse_prev_x = x;
                    mouse_prev_y = y;
                    window.set_cursor_position(screen_center_x, screen_center_y);
                    }
                },
                Event::KeyboardInput(state, sc, keyopt) => {
                    match keyopt {
                        Some(key) => match key {
                            VirtualKeyCode::W => { 
                                match state {
                                    glutin::ElementState::Pressed => w_down = true,
                                    glutin::ElementState::Released => w_down = false,
                                }
                            },
                            VirtualKeyCode::A => { 
                                match state {
                                    glutin::ElementState::Pressed => a_down = true,
                                    glutin::ElementState::Released => a_down = false,
                                }
                            },
                            VirtualKeyCode::S => { 
                                match state {
                                    glutin::ElementState::Pressed => s_down = true,
                                    glutin::ElementState::Released => s_down = false,
                                }
                            },
                            VirtualKeyCode::D => { 
                                match state {
                                    glutin::ElementState::Pressed => d_down = true,
                                    glutin::ElementState::Released => d_down = false,
                                }
                            },
                            VirtualKeyCode::C => { 
                                match state {
                                    glutin::ElementState::Pressed => (),
                                    glutin::ElementState::Released => grabs_mouse = !grabs_mouse,
                                }
                            },
                            VirtualKeyCode::Q => { 
                                match state {
                                    glutin::ElementState::Pressed => (),
                                    glutin::ElementState::Released => println!(" Vertical angle: {}", vert_angle.s),
                                }
                            },
                            VirtualKeyCode::Escape => { 
                                keeprunning = false;
                            },
                            _ => ()
                        },
                        None => ()
                    }
                },
                Event::MouseInput(state, btn) => {
                    if(state == glutin::ElementState::Pressed) {
                        match btn {
                            glutin::MouseButton::Left => {
                                delete_action = true; //Replace a voxel with air
                            },
                            glutin::MouseButton::Right => {
                                set_action = true; //Replace a voxel with stone.
                            },
                            glutin::MouseButton::Middle => {
                                pick_action = true; //Replace a voxel with stone.
                            },
                            _ => ()
                        }
                    }
                },
                _ => (), //println!("Mystery event: {:?}", ev), 
            }
        }

        if(vert_angle.s < 3.14) {
            if(vert_angle.s > 1.57) {
                vert_angle.s = 1.57;
            }
        }
        else if(vert_angle.s >= 3.14) {
            if(vert_angle.s < 4.712) {
                vert_angle.s = 4.712;
            }
        }

        horz_angle = horz_angle.normalize();
        vert_angle = vert_angle.normalize();
        
        //Clockwise to counter-clockwise.
        let yaw : Quaternion<f32> = Quaternion::from_angle_z(horz_angle.neg());
        let pitch : Quaternion<f32> = Quaternion::from_angle_y(vert_angle);
        let rotation = (yaw * pitch).normalize();

        let mut forward : Vector3<f32> = Vector3::new(1.0, 0.0, 0.0);
        let mut right : Vector3<f32> = Vector3::new(0.0, -1.0, 0.0);
        forward = rotation.rotate_vector(forward);
        right = rotation.rotate_vector(right);
        //Remember: Z is our vertical axis here. Cross product would get our downward vector by the right-hand rule.
        let up = forward.cross( right ).neg();

        //Process input 
        if w_down {
            camera_pos += forward * (elapsed * move_speed);
        }
        if d_down {
            camera_pos += right * (elapsed * move_speed);
        }
        if s_down {
            camera_pos += (forward * (elapsed * move_speed)).neg();
        }
        if a_down {
            camera_pos += (right * (elapsed * move_speed)).neg();
        }
        

        let click_point = camera_pos + forward;

        let click_point_vx : VoxelPos<i32> = VoxelPos{x: click_point.x.floor() as i32, y: click_point.y.floor() as i32, z: click_point.z.floor() as i32};
        
        if delete_action { 
            let old_material = space.getv(click_point_vx).unwrap();
            let set_material = air_id.clone();
            space.setv(click_point_vx, set_material.clone());
            if(old_material != set_material.clone()) {
                renderer.notify_remesh(click_point_vx);
            }
            delete_action = false;
        }
        else if set_action {
            let old_material = space.getv(click_point_vx).unwrap();
            let set_material = current_block;
            space.setv(click_point_vx, set_material.clone());
            if(old_material != set_material.clone()) {
                renderer.notify_remesh(click_point_vx);
            }
            set_action = false;
        }
        else if pick_action {
            current_block = space.getv(click_point_vx).unwrap();
            pick_action = false;
        }
        let view_matrix = Matrix4::look_at(camera_pos, camera_pos + forward, up);
        let perspective_matrix = Matrix4::from(perspective);

        //Remesh chunks if necessary.
        
        let before_remesh = precise_time_s();
        renderer.process_remesh(&space, &display, &mat_art_manager);
        let remesh_time = precise_time_s() - before_remesh;
        if(remesh_time > 0.001) {
            println!("Took {} seconds to remesh chunks.", remesh_time);
        } //Remeshing is one to two whole orders of magnitude (!!) faster after compiling optimized rather than debug.

        let mut target = display.draw();
        target.clear_color_and_depth((0.43, 0.7, 0.82, 1.0), 1.0);
        renderer.draw(perspective_matrix, view_matrix, &mut target, &program, &params);
        target.finish().unwrap();
        elapsed = (precise_time_s() - lastupdate) as f32;
        lastupdate = precise_time_s();
    }
    //--------- Save our file on closing --------------
    space.unload_all();
}
