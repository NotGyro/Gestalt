#![allow(unused_imports)]
#![allow(dead_code)]
#![allow(unused_parens)]
#![allow(unused_assignments)]
#![allow(unused_mut)]
#![allow(unused_variables)]
#![allow(unused_must_use)]


#![feature(zero_one)]
//#![feature(collections)]
pub mod util;
pub mod voxel;
pub mod client;

#[macro_use] extern crate glium;
#[macro_use] extern crate cgmath;
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

use cgmath::Matrix4;
use cgmath::Vector3;
use cgmath::Vector4;
use cgmath::Point3;
use cgmath::InnerSpace;

use glium::{DisplayBuild, Surface};
use glium::glutin;
use glium::glutin::Event;
use glium::glutin::VirtualKeyCode;
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
                        glutin::CreationError::OsError(s) => println!("{}", s),
                        glutin::CreationError::NotSupported => println!("BackendCreationError is  NotSupported."),
                        glutin::CreationError::NoBackendAvailable(eee) => println!("{}", eee),
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
    /*println!("Watch the pretty numbers go by.");
    {
        let side = 4;
        let sz = side * side * side;

        let low : VoxelPos<i32> = VoxelPos{x: 0, y: 0, z: 0};
        let high : VoxelPos<i32> = VoxelPos{x: side as i32, y: side as i32, z: side as i32};
        let ran : VoxelRange<i32> = VoxelRange{lower: low, upper: high};

        for i in ran {
            println!("x : {}, y : {}, z : {}", i.x,i.y,i.z);
        }
    }*/
    /*let mut test_va : Box<VoxelArray<bool>> = VoxelArray::load_new(SIDELENGTH, SIDELENGTH, SIDELENGTH, test_chunk);
    
    test_va.set(8, 8, 4, true);*/
    let mat_idx : MaterialIndex = MaterialIndex::new();

    let air_id : MaterialID = mat_idx.for_name(String::from("test.air"));
    let stone_id : MaterialID = mat_idx.for_name(String::from("test.stone"));
    let dirt_id : MaterialID = mat_idx.for_name(String::from("test.dirt"));
    let grass_id : MaterialID = mat_idx.for_name(String::from("test.grass"));
    
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
                println!("Generating chunk at {}, {}, {}", x, y, z);
            }
        }
    }
    
    //---- Set up window ----
    let screen_width : u32 = 800;
    let screen_height : u32 = 600;
    let display = make_display(screen_width, screen_height);
    let mut keeprunning = true;
    let window = display.get_window().unwrap();
    window.set_cursor_state(glutin::CursorState::Grab);
    //---- Set up screen and some basic graphics stuff ----
    let mut vshaderfile = File::open("vertexshader.glsl").unwrap();
    let mut fshaderfile = File::open("fragmentshader.glsl").unwrap();
    let mut vertex_shader_src = String::new();
    let mut fragment_shader_src = String::new();
    vshaderfile.read_to_string(&mut vertex_shader_src);
    fshaderfile.read_to_string(&mut fragment_shader_src);
    
    println!(line!());
    let program = glium::Program::from_source(&display, vertex_shader_src.as_ref(), fragment_shader_src.as_ref(), None).unwrap();

    let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

    let mut t: f32 = -0.5;
    
    //println!(line!());
    //println!(line!());
    
    let params = glium::DrawParameters {
        depth: glium::Depth {
            test: glium::draw_parameters::DepthTest::IfLess,
            write: true,
            .. Default::default()
        },
        .. Default::default()
    };
    
    //---- Set up our camera ----
    
	let mut camera_pos : Point3<f32> = Point3 {x : 0.0, y : 0.0, z : 10.0}; 
	
	let mouse_sensitivity : f32 = 900.0;
	let move_speed : f32 = 3000.0;
	let mut horz_angle : f32 = 0.0;
	let mut vert_angle : f32 = 0.0;

    //let mut perspective_matrix : cgmath::Matrix4<f32> = cgmath::perspective(cgmath::deg(45.0), 1.333, 0.0001, 100.0);
    //let mut view_matrix : Matrix4<f32> = Matrix4::look_at(view_eye, view_center, view_up);
    let mut model_matrix : Matrix4<f32> = Matrix4::from_scale(1.0);    
    
    let mut mouse_prev_x : i32 = 0;
    let mut mouse_prev_y : i32 = 0;
    
    
    let perspective : cgmath::PerspectiveFov<f32> = cgmath::PerspectiveFov { fovy : cgmath::Rad {s : 1.22173 }, aspect : 4.0 / 3.0, near : 0.1, far : 100.0}; 
    /* --------------------------- Load a chunk, maybe */
    //let chunk_path = Path::new("testchunk.bin");
    
	//let mut options = OpenOptions::new();
    /*let display_path = chunk_path.display();
	match OpenOptions::new()
            .read(true)
            .open(&chunk_path) {
        // The `description` method of `io::Error` returns a string that
        // describes the error
        Err(why) => println!("couldn't open {}: {}", display_path, Error::description(&why)),
        Ok(mut file) => {
            println!("Attempting to load {}", display_path);
            chunk.load(&mut file);
        },
    };*/
    
    //---- Set up our texture(s) and chunk verticies ----
    let stone_art = MatArtSimple { texture_name : String::from("teststone.png") };
    let dirt_art = MatArtSimple { texture_name : String::from("testdirt.png") };
    let grass_art = MatArtSimple { texture_name : String::from("testgrass.png") };
    
    /*let mut texfile = File::open("teststone.png").unwrap();
    let image = image::load(&texfile,
                        image::PNG).unwrap().to_rgba();
    let image_dimensions = image.dimensions();
    let image = glium::texture::RawImage2d::from_raw_rgba_reversed(image.into_raw(), image_dimensions);
    let texture = glium::texture::Texture2d::new(&display, image).unwrap();*/
    let mut mat_art_manager = MatArtMapping::new();
    let mut texture_manager = TextureArrayDyn::new(64, 64, 4096);
    mat_art_manager.insert(grass_id.clone(), grass_art.clone());
    mat_art_manager.insert(stone_id.clone(), stone_art.clone());
    mat_art_manager.insert(dirt_id.clone(), dirt_art.clone());

    //let mut map_verts = Box::new(client::simplerenderer::make_voxel_mesh(&*chunk, &display, &mut texture_manager, &mat_art_manager));
    let mut meshes : Vec<(VoxelRange<i32>, Box<glium::VertexBuffer<PackedVertex>>)> = Vec::new();
    //pub fn make_voxel_mesh(vs : &VoxelStorage<MaterialID, i32>, display : &GlutinFacade, range : VoxelRange<i32>, 
    //                    textures : &mut TextureArrayDyn, art_map : &MatArtMapping)
    for chunk in space.get_regions() { 
        //Push a tuple of the boundaries of the chunk and its mesh data.
        meshes.push(
            (chunk, 
             Box::new( client::simplerenderer::make_voxel_mesh(&space, &display, chunk, &mut texture_manager, &mat_art_manager) ) 
            )
        );
    }
    //---- Some movement stuff ----
    
    let mut w_down : bool = false;
    let mut a_down : bool = false;
    let mut s_down : bool = false;
    let mut d_down : bool = false;
    
    let mut lastupdate = precise_time_s();
    
    let screen_center_x : i32 = screen_width as i32 /2;
    let screen_center_y : i32 = screen_height as i32 /2;
    
    let mut mouse_first_moved : bool = false;
    //---- A mainloop ----
    while keeprunning {
        let mut mesh : &mut Vec<(VoxelRange<i32>, Box<glium::VertexBuffer<PackedVertex>>)> = meshes.as_mut();

		if(vert_angle > 1.57) {
			vert_angle = 1.57;
		}
		else if(vert_angle < -1.57) {
			vert_angle = -1.57;
        }
        
        let forward = Vector3::new(
            vert_angle.cos() * horz_angle.sin(), 
            vert_angle.cos() * horz_angle.cos(),
			vert_angle.sin());
            
		let right = Vector3::new(
			(horz_angle - PI/2.0).sin(), 
			(horz_angle - PI/2.0).cos(),
            0.0
			
		);
        let up = forward.cross( right );

        let click_point = camera_pos + forward.normalize(); //Normalize
        
        let elapsed = (precise_time_s() - lastupdate) as f32;
        for ev in display.poll_events() {
            match ev {
                Event::Closed => {keeprunning = false},   // The window has been closed by the user, external to our game (hitting x in corner, for example)
                Event::MouseMoved(x, y) => {
                    if mouse_first_moved {
                        horz_angle += ((x - screen_center_x) as f32) * (mouse_sensitivity * elapsed);
                        vert_angle -= ((y - screen_center_y) as f32) * (mouse_sensitivity * elapsed);
                    }
                    else {
                        mouse_first_moved = true;
                    }
                    mouse_prev_x = x;
                    mouse_prev_y = y;
                    //window.set_cursor_position(screen_center_x, screen_center_y);
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
                            VirtualKeyCode::Escape => { 
                                keeprunning = false;
                            },
                            _ => ()
                        },
                        None => ()
                    }
                },/*
                Event::MouseInput(state, btn) => {
                    if(state == glutin::ElementState::Released) {
                    println!("X: {}", click_point.x);
                    println!("Y: {}", click_point.y);
                    println!("Z: {}", click_point.z);
                    if((click_point.x >= 0.0) && (click_point.y >= 0.0) && (click_point.z >= 0.0)) { //Change this when it's no longer one chunk.
                        match btn {
                            glutin::MouseButton::Left => {
                                chunk.set(click_point.x as u16, click_point.y as u16, click_point.z as u16, air_id.clone());
                                remesh = true;
                            },
                            glutin::MouseButton::Right => {
                                chunk.set(click_point.x as u16, click_point.y as u16, click_point.z as u16, stone_id.clone());
                                remesh = true;
                            }
                            _ => ()
                        }
                    }
                    }
                }*/
                _ => ()
            }
        }
        if w_down {
            camera_pos += forward * (elapsed * move_speed);
        }
        if a_down {
            camera_pos += right * (elapsed * move_speed);
        }
        if s_down {
            camera_pos += (forward * (elapsed * move_speed)).neg();
        }
        if d_down {
            camera_pos += (right * (elapsed * move_speed)).neg();
        }
        let view_matrix = Matrix4::look_at(camera_pos, camera_pos + forward, up);
        let perspective_matrix = Matrix4::from(perspective);

        /*if(remesh) {
            map_verts = Box::new(client::simplerenderer::make_voxel_mesh(&*chunk, &display, &mut texture_manager, &mat_art_manager));
            remesh = false;
        }*/

        let mut target = display.draw();
        target.clear_color_and_depth((0.43, 0.7, 0.82, 1.0), 1.0);

        if(texture_manager.textures.is_some()) {
            let textures = texture_manager.textures.unwrap(); //Move
            let iter = mesh.into_iter();
            for &mut (bounds, ref mesh) in iter
            {
                //Create a context so uniforms dies and textures is no longer borrowed.
                {
                    /*In C++ I did this next bit with:
                        glm::mat4 Model = glm::translate(glm::mat4(1.0f), 
                        glm::vec3(DrawIter->first->getXPosition()*CHUNK_SIZE, DrawIter->first->getYPosition()*CHUNK_SIZE, DrawIter->first->getZPosition()*CHUNK_SIZE));
                    */
                    let szx = bounds.upper.x - bounds.lower.x;
                    let szy = bounds.upper.y - bounds.lower.y;
                    let szz = bounds.upper.z - bounds.lower.z;
                    let pos = bounds.lower;
                    let chunk_model_matrix = Matrix4::from_translation(Vector3{ x : (pos.x * szx as i32) as f32, y : (pos.y * szy as i32) as f32, z : (pos.z * szz as i32) as f32 });
                    let mvp_matrix = perspective_matrix * view_matrix * chunk_model_matrix;
                    let uniforms = uniform! {
                        mvp: Into::<[[f32; 4]; 4]>::into(mvp_matrix),
                        tex: &textures,
                    };
                    /*target.draw(&vertex_buffer, &indices, &program, &uniforms,
                        &Default::default()).unwrap();*/
                    target.draw(&(**mesh), &indices, &program, &uniforms,
                        &params).unwrap();
                }
            }

            texture_manager.textures = Some(textures); //Move back
        }
        target.finish().unwrap();
        lastupdate = precise_time_s();
    }
    //--------- Save our file on closing --------------
    /*match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&chunk_path) {
        // The `description` method of `io::Error` returns a string that
        // describes the error
        Err(why) => println!("couldn't open {}: {}", display_path, Error::description(&why)),
        Ok(mut file) => {
            chunk.save(&mut file);
        },
    };*/
}
