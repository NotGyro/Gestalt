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
extern crate parking_lot;

#[macro_use] extern crate cgmath;

extern crate time;
extern crate image;

#[macro_use] extern crate log;
extern crate chrono;

use util::logger;
use util::logger::*;

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

use voxel::block::BlockID;

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
    //This MUST be the first thing we call. 
    init_logger();
    println!("{:?}", std::env::current_exe());
    //Messing around with logging a bit.
    trace!("Hello, world!");
    info!("I have a logger now!");
    error!("Oh no! This is an error");
    let gls = GAME_LOGGER_STATE.lock();
    let receiver = gls.console_receiver.clone();
    drop(gls);
    let v : Vec<String> = receiver.try_iter().collect();
    trace!("So far we have logged {} messages.", v.len()); 
    info!("Quitting application.");
}
