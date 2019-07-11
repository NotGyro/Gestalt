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

extern crate mint;
extern crate three;

use util::logger;
use util::logger::*;

use std::vec::Vec;

use std::{io, cmp};
use num::Zero;
use std::thread;

//use gluon::vm::api::IO;

use voxel::voxelstorage::*;
use voxel::voxelarray::*;

use voxel::voxelmath::*;

use voxel::voxelevent::*;
use util::event::EventBus;

use voxel::block::BlockID;

use cgmath::prelude::*;
use three::Object;

const COLOR_BACKGROUND: three::Color = 0xf0e0b6;

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
    /*
    let receiver_move = receiver.clone();
    
    let log_thread = thread::spawn(move || {
        loop {
            let msg = receiver_move.recv();
        }
    });
    */
    let mut win = three::Window::new("Gestalt");
    win.scene.background = three::Background::Color(COLOR_BACKGROUND);
    let cam = win.factory.perspective_camera(75.0, 1.0 .. 50.0);
    cam.set_position([0.0, 0.0, 10.0]);

    let mbox = {
        let geometry = three::Geometry::cuboid(3.0, 2.0, 1.0);
        let material = three::material::Wireframe { color: 0x00FF00 };
        win.factory.mesh(geometry, material)
    };

        mbox.set_position([-3.0, -3.0, 0.0]);
    win.scene.add(&mbox);

    let mcyl = {
        let geometry = three::Geometry::cylinder(1.0, 2.0, 2.0, 5);
        let material = three::material::Wireframe { color: 0xFF0000 };
        win.factory.mesh(geometry, material)
    };
    mcyl.set_position([3.0, -3.0, 0.0]);
    win.scene.add(&mcyl);

    let msphere = {
        let geometry = three::Geometry::uv_sphere(2.0, 5, 5);
        let material = three::material::Wireframe { color: 0xFF0000 };
        win.factory.mesh(geometry, material)
    };
    msphere.set_position([-3.0, 3.0, 0.0]);
    win.scene.add(&msphere);

    // test removal from scene
    win.scene.remove(&mcyl);
    win.scene.remove(&mbox);
    win.scene.add(&mcyl);
    win.scene.add(&mbox);

    let mline = {
        let geometry = three::Geometry::with_vertices(vec![
            [-2.0, -1.0, 0.0].into(),
            [0.0, 1.0, 0.0].into(),
            [2.0, -1.0, 0.0].into(),
        ]);
        let material = three::material::Line { color: 0x0000FF };
        win.factory.mesh(geometry, material)
    };
    mline.set_position([3.0, 3.0, 0.0]);
    win.scene.add(&mline);

    let mut angle = cgmath::Rad::zero();
    while win.update() && !win.input.hit(three::KEY_ESCAPE) {
        if let Some(diff) = win.input.timed(three::AXIS_LEFT_RIGHT) {
            angle += cgmath::Rad(1.5 * diff);
            let q = cgmath::Quaternion::from_angle_y(angle);
            mbox.set_orientation(q);
            mcyl.set_orientation(q);
            msphere.set_orientation(q);
            mline.set_orientation(q);
        }
        win.render(&cam);
    }

    let v : Vec<String> = receiver.try_iter().collect();
    trace!("So far we have logged {} messages.", v.len()); 
    info!("Quitting application.");
}
