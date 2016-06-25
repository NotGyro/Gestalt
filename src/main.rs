#![allow(unused_imports)]
//#![feature(collections)]
pub mod util;
pub mod voxel;
pub mod client;

#[macro_use] extern crate glium;

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
use std::f64::consts::*;

use glium::{DisplayBuild, Surface};
use glium::glutin;
#[derive(Copy, Clone)]
struct Vertex {
    position: [f32; 2],
}
implement_vertex!(Vertex, position);

#[allow(dead_code)]
#[allow(unused_parens)]
fn main() {
    //---- Runtime testing stuff ----
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u16);
    }

    let mut test_va : Box<VoxelArray<u16>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    
    assert!(test_va.get(14,14,14).unwrap() == 3822);
    test_va.set(14,14,14,9);
    assert!(test_va.get(14,14,14).unwrap() == 9);
    
   	test_array_fileio();
    //---- Set up window ----
    let display = glutin::WindowBuilder::new()
        .build_glium()
        .unwrap();
    let mut keeprunning = true;
    //---- Set up test graphics ----
    let mut vshaderfile = File::open("vertexshader.glsl").unwrap();
    let mut fshaderfile = File::open("fragmentshader.glsl").unwrap();
    let mut vertex_shader_src = String::new();
    let mut fragment_shader_src = String::new();
    vshaderfile.read_to_string(&mut vertex_shader_src);
    fshaderfile.read_to_string(&mut fragment_shader_src);
    
    let program = glium::Program::from_source(&display, vertex_shader_src.as_ref(), fragment_shader_src.as_ref(), None).unwrap();
    
    let vertex1 = Vertex { position: [-1.0, -1.0] };
    let vertex2 = Vertex { position: [ 0.0,  1.0] };
    let vertex3 = Vertex { position: [ 1.0, -1.0] };
    let shape = vec![vertex1, vertex2, vertex3];
    let vertex_buffer = glium::VertexBuffer::new(&display, &shape).unwrap();
    let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

    let mut t: f32 = -0.5;

    //---- A mainloop ----
    
    while keeprunning {
        t += 0.0002;
        if t > (2.0*(PI as f32)) {
            t = 0.0;
        }
        
        let uniforms = uniform! {
            matrix: [
                [ t.cos(), t.sin(), 0.0, 0.0],
                [-t.sin(), t.cos(), 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0f32],
            ]
        };
        let mut target = display.draw();
        target.clear_color(0.0, 0.0, 1.0, 1.0);
        target.draw(&vertex_buffer, &indices, &program, &uniforms,
            &Default::default()).unwrap();
        // listing the events produced by the window and waiting to be received
        for ev in display.poll_events() {
            match ev {
                glium::glutin::Event::Closed => {keeprunning = false},   // the window has been closed by the user
                _ => ()
            }
        }
        target.finish().unwrap();
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