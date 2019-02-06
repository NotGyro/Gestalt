//#![feature(collections)]
pub mod util;
pub mod voxel;

#[macro_use] extern crate serde_derive;
#[macro_use] extern crate string_cache;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate crossbeam;
//#[macro_use] extern crate gluon;
extern crate num;
extern crate serde;

#[macro_use] extern crate cgmath;

extern crate time;
extern crate image;

use time::*;
use std::thread::sleep;
use std::time::Duration;

use std::vec::Vec;

use std::path::Path;
use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::io::{BufWriter, Cursor};
use std::{io, cmp};
use std::f32::consts::*;
use std::f32;
use std::f32::*;
use std::ops::Neg;
use std::collections::{HashMap, HashSet};
use num::Zero;

//use gluon::vm::api::IO;

use voxel::voxelstorage::*;
use voxel::voxelarray::*;

use voxel::voxelmath::*;

use voxel::voxelevent::*;
use util::event::EventBus;

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
/*
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
*/
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
/*
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
  */  
    //---- Let's try Gluon ----
    /*let glu_path = "gamedata/main.glu";
    let vm = gluon::new_vm();
    let mut glu_source = String::new();
    let mut file = File::open(glu_path).expect(&format!("File {} not found.", glu_path));
    match file.read_to_string(&mut glu_source) {
        Ok(_) => println!("Successfully loaded Gluon file!"),
        Err(err) => eprint!("Could not load Gluon script file: {:?}", err),  
    }

    let result = gluon::Compiler::new()
        .run_io(true)
        .run_expr::<IO<()>>(&vm, "test", &glu_source)
        .unwrap();
    */
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

    sleep(Duration::new(1, 0));

    //--------- Save our file on closing --------------
    //space.unload_all();
}
