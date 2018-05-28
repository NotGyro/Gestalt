#![allow(dead_code)]
#![allow(unused_parens)]
#![allow(unused_assignments)]
#![allow(unused_mut)]
#![allow(unused_variables)]
#![allow(unused_must_use)]

//#![feature(collections)]
pub mod util;
pub mod voxel;

#[macro_use] extern crate serde_derive;
#[macro_use] extern crate string_cache;
#[macro_use] extern crate lazy_static;
extern crate num;

#[macro_use] extern crate cgmath;

extern crate time;
extern crate image;

use time::*;
use std::thread::sleep;
use std::time::Duration;

use std::vec::Vec;
use voxel::voxelstorage::*;
use voxel::voxelarray::*;
use voxel::vspalette::*;
use voxel::material::*;
use voxel::voxelspace::*;

use util::voxelutil::*;

use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufWriter, Cursor};
use std::fs::OpenOptions;
use std::{io, cmp};
use std::f32::consts::*;
use std::f32;
use std::f32::*;
use std::ops::Neg;
use std::collections::{HashMap, HashSet};
use num::Zero;

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

//Just a test here. Not for use in production.
fn voxel_raycast_first(space : &VoxelSpace, air_id : MaterialID, raycast : &mut VoxelRaycast) -> Option<VoxelPos<i32>> {
    let mut count = 0;
    const MAX_COUNT : usize = 4096; //TODO: Don't use a magic number.
    loop {
        let result = space.getv(raycast.pos);
        match result {
            Some(val) => { 
                if val != air_id {
                    return Some(raycast.pos)
                }
            },
            None => return None,
        }
        count = count + 1;
        if(count > MAX_COUNT) {
            return None;
        }
        raycast.step();
    }
}

use std::time::Instant;

fn fps_limit(fps : u64, frame_start : Instant) {
    use std::time::Duration;

    let now = Instant::now();

    /* we allocate N ms for each frame and then we subtract the difference between the start and
     * the end of the frame from it, we use the result as the duration to sleep
     * #: allocated unit
     * +: allocated unit, used
     * assuming unit = 10ms and fps = 10, [##########]
     * [+++++++###], thus we sleep ### -> 30 ms in order to fill it
     */
    let diff = now.duration_since(frame_start);
    let allocated = Duration::from_millis(1000 / fps);
    let sleepdur = allocated.checked_sub(diff);
    match sleepdur {
        Some(x) => sleep(x),
        None => ()
    }
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

    let mut keeprunning = true;

    //window.set_cursor_state(glutin::CursorState::Grab);

    //---- Set up screen and some basic graphics stuff ----
    /*let mut vshaderfile = File::open("vertexshader.glsl").unwrap();
    let mut fshaderfile = File::open("fragmentshader.glsl").unwrap();
    let mut vertex_shader_src = String::new();
    let mut fragment_shader_src = String::new();
    vshaderfile.read_to_string(&mut vertex_shader_src);
    fshaderfile.read_to_string(&mut fragment_shader_src);*/

    //---- Set up our camera ----

	//let mut camera_pos : Point3<f32> = Point3 {x : 0.0, y : 0.0, z : 10.0};

	//let mut horz_angle : Rad<f32> = Rad::zero();
	//let mut vert_angle : Rad<f32> = Rad::zero();

    //let mut perspective_matrix : cgmath::Matrix4<f32> = cgmath::perspective(cgmath::deg(45.0), 1.333, 0.0001, 100.0);
    //let mut view_matrix : Matrix4<f32> = Matrix4::look_at(view_eye, view_center, view_up);
    //let mut model_matrix : Matrix4<f32> = Matrix4::from_scale(1.0);

    //let perspective : cgmath::PerspectiveFov<f32> = cgmath::PerspectiveFov { fovy : cgmath::Rad {s : 1.22173 }, aspect : 4.0 / 3.0, near : 0.1, far : 100.0};

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
	let mouse_sensitivity : f32 = 0.0005;
	let move_speed : f32 = 16.0;

    let mut mouse_prev_x : i32 = 0;
    let mut mouse_prev_y : i32 = 0;
    
    //window.set_cursor_state(glutin::CursorState::Grab);

    /*while keeprunning {
        let start = Instant::now();
        lastupdate = precise_time_s();
        // Do game stuff here.
        let now = precise_time_s();
        elapsed = (now - lastupdate) as f32;
        lastupdate = now;
        fps_limit(60, start);
        if(precise_time_s() > 20.0f64) { keeprunning = false; }
    }*/ 

    sleep(Duration::new(10, 0));

    //--------- Save our file on closing --------------
    space.unload_all();
}
