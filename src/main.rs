#![allow(unused_imports)]
//#![feature(collections)]
pub mod util;
pub mod voxel;
pub mod client;

#[macro_use] extern crate glium;
#[macro_use] extern crate cgmath;
extern crate time;

use time::*;

use std::vec::Vec;
use voxel::voxelstorage::VoxelStorage;
use voxel::voxelarray::VoxelArray;
use client::dwarfmode::*;
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

use cgmath::Matrix4;
use cgmath::Vector3;
use cgmath::Point3;

use glium::{DisplayBuild, Surface};
use glium::glutin;
use glium::glutin::Event;
use glium::glutin::VirtualKeyCode;

#[allow(dead_code)]
#[allow(unused_parens)]
fn main() {
    //---- Runtime testing stuff ----
    const SIDELENGTH : u32 = 16;
    const OURSIZE : usize  = (SIDELENGTH * SIDELENGTH * SIDELENGTH) as usize;
    let mut test_chunk : Vec<bool> = vec![false; OURSIZE];

    let mut test_va : Box<VoxelArray<bool>> = VoxelArray::load_new(SIDELENGTH, SIDELENGTH, SIDELENGTH, test_chunk);
    for x in 0 .. SIDELENGTH {
        for y in 0 .. SIDELENGTH {
            for z in 0 .. 3 {
                test_va.set(x, y, z, true);
            }
        }
    }
    test_va.set(8, 8, 4, true);
    
    //---- Set up window ----
    let screen_width : u32 = 800;
    let screen_height : u32 = 600;
    let display = glutin::WindowBuilder::new()
        .with_dimensions(screen_width, screen_height)
        .build_glium()
        .unwrap();
    let mut keeprunning = true;
    let window = display.get_window().unwrap();
    window.set_cursor_state(glutin::CursorState::Grab);
    //---- Set up test graphics ----
    let mut vshaderfile = File::open("vertexshader.glsl").unwrap();
    let mut fshaderfile = File::open("fragmentshader.glsl").unwrap();
    let mut vertex_shader_src = String::new();
    let mut fragment_shader_src = String::new();
    vshaderfile.read_to_string(&mut vertex_shader_src);
    fshaderfile.read_to_string(&mut fragment_shader_src);
    
    let program = glium::Program::from_source(&display, vertex_shader_src.as_ref(), fragment_shader_src.as_ref(), None).unwrap();

    let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

    let mut t: f32 = -0.5;
    
    let map_verts = client::simplerenderer::mesh_voxels(test_va.as_ref(), &display);

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
    
    let mut mouse_first_moved : bool = false;
    
    let perspective : cgmath::PerspectiveFov<f32> = cgmath::PerspectiveFov { fovy : cgmath::Rad {s : 1.22173 }, aspect : 4.0 / 3.0, near : 0.1, far : 100.0}; 

    //---- Some movement stuff ----
    
    let mut w_down : bool = false;
    let mut a_down : bool = false;
    let mut s_down : bool = false;
    let mut d_down : bool = false;
    
    let mut lastupdate = precise_time_s();
    
    let screen_center_x : i32 = screen_width as i32 /2;
    let screen_center_y : i32 = screen_height as i32 /2;
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
        
        let elapsed = (precise_time_s() - lastupdate) as f32;
        for ev in display.poll_events() {
            match ev {
                Event::Closed => {keeprunning = false},   // the window has been closed by the user
                Event::MouseMoved(x, y) => {
                    //if mouse_first_moved {
                        horz_angle += ((x - screen_center_x) as f32) * (mouse_sensitivity * elapsed);
                        vert_angle += ((y - screen_center_y) as f32) * (mouse_sensitivity * elapsed);
                        window.set_cursor_position(screen_center_x, screen_center_y);
                    //}
                    //else {
                    //    mouse_first_moved = true;
                    //}
                    mouse_prev_x = x;
                    mouse_prev_y = y;
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
        let uniforms = uniform! {
            mvp: Into::<[[f32; 4]; 4]>::into(mvp_matrix)
        };
        let mut target = display.draw();
        target.clear_color(0.0, 0.0, 1.0, 1.0);
        /*target.draw(&vertex_buffer, &indices, &program, &uniforms,
            &Default::default()).unwrap();*/
        target.draw(&map_verts, &indices, &program, &uniforms,
            &Default::default()).unwrap();
        // listing the events produced by the window and waiting to be received
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