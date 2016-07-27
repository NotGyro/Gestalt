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
extern crate time;
extern crate image;

use time::*;

use std::vec::Vec;
use voxel::voxelstorage::VoxelStorage;
use voxel::voxelarray::VoxelArray;
use voxel::vspalette::VoxelPalette;
use voxel::material::MaterialID;
use client::dwarfmode::*;
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
use cgmath::Point3;
use cgmath::InnerSpace;

use glium::{DisplayBuild, Surface};
use glium::glutin;
use glium::glutin::Event;
use glium::glutin::VirtualKeyCode;
use glium::texture::Texture2dArray;

fn main() {
    println!(line!());
    /*let mut test_va : Box<VoxelArray<bool>> = VoxelArray::load_new(SIDELENGTH, SIDELENGTH, SIDELENGTH, test_chunk);
    
    test_va.set(8, 8, 4, true);*/
    let air_id : MaterialID = String::from("Air");
    let stone_id : MaterialID = String::from("Stone");
    let dirt_id : MaterialID = String::from("Dirt");
    let grass_id : MaterialID = String::from("Grass");
    
    const SIDELENGTH : u32 = 16;
    const OURSIZE : usize  = (SIDELENGTH * SIDELENGTH * SIDELENGTH) as usize;
    let mut test_chunk : Vec<u8> = vec![0; OURSIZE];

    let mut backing_va : Box<VoxelArray<u8>> = VoxelArray::load_new(SIDELENGTH, SIDELENGTH, SIDELENGTH, test_chunk);
    let mut test_va : VoxelPalette<String, u8, u32> = VoxelPalette {base : backing_va, index : Vec::new(), rev_index : HashMap::new() };
    test_va.init_default_value(air_id.clone(), 0);

    let surface = 10;
    let dirt_height = (surface-2);
    for x in 0 .. SIDELENGTH {
        for y in 0 .. SIDELENGTH {
            for z in 0 .. dirt_height {
                test_va.set(x, y, z, stone_id.clone()); //TODO: less stupid material IDs that pass-by-copy by default
            }
            for z in dirt_height .. surface {
                test_va.set(x, y, z, dirt_id.clone()); //TODO: less stupid material IDs that pass-by-copy by default
            }
            test_va.set(x, y, surface, grass_id.clone());
        }
    }
    test_va.set(8, 8, (surface+2), stone_id.clone());
    
    //---- Set up window ----
    let screen_width : u32 = 800;
    let screen_height : u32 = 600;
    let display = glutin::WindowBuilder::new()
        .with_dimensions(screen_width, screen_height)
        .with_depth_buffer(24)
        .build_glium()
        .unwrap();
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

    //---- Some movement stuff ----
    
    let mut w_down : bool = false;
    let mut a_down : bool = false;
    let mut s_down : bool = false;
    let mut d_down : bool = false;
    
    let mut lastupdate = precise_time_s();
    
    let screen_center_x : i32 = screen_width as i32 /2;
    let screen_center_y : i32 = screen_height as i32 /2;
    
    let mut mouse_first_moved : bool = false;
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
    mat_art_manager.insert(stone_id.clone(), stone_art.clone());
    mat_art_manager.insert(dirt_id.clone(), dirt_art.clone());
    mat_art_manager.insert(grass_id.clone(), grass_art.clone());
    let mut map_verts = Box::new(client::simplerenderer::make_voxel_mesh(&test_va, &display, &mut texture_manager, &mat_art_manager));
    
    let mut remesh : bool = false;
    //---- A mainloop ----
    while keeprunning {

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
                    window.set_cursor_position(screen_center_x, screen_center_y);
                },
                Event::KeyboardInput(state, sc, keyopt) => {
                    let key = keyopt.unwrap();
                    match key {
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
                    }
                },
                Event::MouseInput(state, btn) => {
                    if(state == glutin::ElementState::Released) {
                    println!("X: {}", click_point.x);
                    println!("Y: {}", click_point.y);
                    println!("Z: {}", click_point.z);
                    if((click_point.x >= 0.0) && (click_point.y >= 0.0) && (click_point.z >= 0.0)) { //Change this when it's no longer one chunk.
                        match btn {
                            glutin::MouseButton::Left => {
                                test_va.set(click_point.x as u32, click_point.y as u32, click_point.z as u32, air_id.clone());
                                remesh = true;
                            },
                            glutin::MouseButton::Right => {
                                test_va.set(click_point.x as u32, click_point.y as u32, click_point.z as u32, stone_id.clone());
                                remesh = true;
                            }
                            _ => ()
                        }
                    }
                    }
                }
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
            camera_pos += forward.neg() * (elapsed * move_speed);
        }
        if d_down {
            camera_pos += right.neg() * (elapsed * move_speed);
        }
        let view_matrix = Matrix4::look_at(camera_pos, camera_pos + forward, up);
        let perspective_matrix = Matrix4::from(perspective);
        let mvp_matrix = perspective_matrix * view_matrix * model_matrix;

        if(remesh) {
            map_verts = Box::new(client::simplerenderer::make_voxel_mesh(&test_va, &display, &mut texture_manager, &mat_art_manager));
            remesh = false;
        }

        let mut target = display.draw();
        target.clear_color_and_depth((0.43, 0.7, 0.82, 1.0), 1.0);

        if(texture_manager.textures.is_some()) {
            let textures = texture_manager.textures.unwrap(); //Move
            //Create a context so uniforms dies and textures is no longer borrowed.
            {
                let uniforms = uniform! {
                    mvp: Into::<[[f32; 4]; 4]>::into(mvp_matrix),
                    tex: &textures,
                };
                /*target.draw(&vertex_buffer, &indices, &program, &uniforms,
                    &Default::default()).unwrap();*/
                target.draw(&(*map_verts), &indices, &program, &uniforms,
                    &params).unwrap();
            }

            texture_manager.textures = Some(textures); //Move back
        }
        target.finish().unwrap();
        lastupdate = precise_time_s();
    }
}

fn test_array_fileio() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u8);
    }

    let mut test_va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);

    assert!(test_va.get(14,14,14).unwrap() == 238);
    test_va.set(14,14,14,9);
    assert!(test_va.get(14,14,14).unwrap() == 9);
    
    let path = Path::new("hello.bin");

    // Open the path in read-only mode, returns `io::Result<File>`
    //let file : &mut File = try!(File::open(path));
    
    let display = path.display();
	let mut file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path) {
        // The `description` method of `io::Error` returns a string that
        // describes the error
        Err(why) => panic!("couldn't open {}: {}", display,
                                                   Error::description(&why)),
        Ok(file) => file,
    };
	// We create a buffered writer from the file we get
	//let mut writer = BufWriter::new(&file);
	    
   	test_va.save(&mut file);
    _load_test(path);
}

fn _load_test(path : &Path) -> bool {
	//let mut options = OpenOptions::new();
	// We want to write to our file as well as append new data to it.
	//options.write(true).append(true);
    let display = path.display();
	let mut file = match File::open(&path) {
        // The `description` method of `io::Error` returns a string that
        // describes the error
        Err(why) => panic!("couldn't open {}: {}", display,
                                                   Error::description(&why)),
        Ok(file) => file,
    };
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = Vec::with_capacity(OURSIZE);
    for _i in 0 .. OURSIZE {
    	test_chunk.push(0);
    }
	
    let mut va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    
    va.load(&mut file);
    
    assert!(va.get(14,14,14).unwrap() == 9);

    return true;
}