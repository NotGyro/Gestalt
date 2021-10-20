//! Voxel metaverse "game" you can have some fun in.

#![allow(incomplete_features)]

#![feature(drain_filter)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]
#![feature(associated_type_bounds)]

#[macro_use] extern crate hemlock;

//#[macro_use] extern crate arr_macro;
extern crate base16;
extern crate bincode;
extern crate blake3;
extern crate bytemuck;
extern crate clap;
extern crate crossbeam_channel;
#[macro_use] extern crate custom_error;
#[macro_use] extern crate enum_dispatch;
extern crate hashbrown;
#[macro_use] extern crate lazy_static;
extern crate num;
extern crate parking_lot;
extern crate rand;
extern crate semver;
extern crate serde;
extern crate sodiumoxide;
//#[macro_use] extern crate tokio;
extern crate ustr;
extern crate uuid;
extern crate winit;


use std::error::Error;
use logger::hemlock_scopes;
use clap::{Arg, App};
use std::net::SocketAddr;

use futures::executor::block_on;

use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    window::Window,
};

#[macro_use] pub mod common;
pub mod entity;
pub mod world;
/// The main purpose of the Logger module is to define our Hemlock scopes. 
/// It also contains a https://crates.io/crates/log proxy into Hemlock, so anything 
/// logged using that crate's macros will show up as coming from the "Library" scope.
pub mod logger;

#[allow(unused_must_use)]
fn main() {
    /*
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    let mut screen_state = match block_on(ScreenState::new(&window)) {
        Ok(st) => {info!(Renderer, "Screen state initialized."); st},
        Err(e) => panic!("Could not initialize screen state! Reason: {}", e),
    };
    match logger::logger::init_logger() {
        Ok(_) => info!(Core, "Logger initialized."),
        Err(e) => panic!("Could not initialize logger! Reason: {}", e),
    };
    
    let matches = App::new("Gestalt Engine")
        .arg(Arg::with_name("server")
            .short("s")
            .long("server")
            .help("Starts a server version of this engine, headless."))
        .arg(Arg::with_name("ip")
            .short("i")
            .long("ip")
            .value_name("IP")
            .help("Joins a server at the selected IP address and socket if client, hosts from IP and socket if server.")
            .takes_value(true))
        .get_matches();

    let ip: Option<SocketAddr> = matches.value_of("ip").map(|i| i.parse()
                                        .map_err(|e| panic!("Unable to parse provided IP address: {:?}", e)).unwrap());
    
    let is_server: bool = matches.is_present("server");
    let mut last_updated = std::time::Instant::now();
    if !is_server {
        info!(Core, "Starting as client - join IP is {:?} (singleplayer if none).", ip);
        trace!(Core, "Client code goes here later.");
        event_loop.run(move |event, _, control_flow| match event {
            Event::RedrawRequested(_) => {

                let elapsed = std::time::Instant::now() - last_updated;
                screen_state.update(elapsed);
                last_updated = std::time::Instant::now();

                match screen_state.render() {
                    Ok(_) => { /* Sucessfully drew a frame! */}
                    // Recreate the swap_chain if lost
                    Err(wgpu::SwapChainError::Lost) => screen_state.resize(screen_state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SwapChainError::OutOfMemory) => {
                        hemlock::error!(Core, "System out of memory! Cannot render next frame. Exiting.");
                        *control_flow = ControlFlow::Exit
                    },
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            },
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            },
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => if !screen_state.input(event) {
                match event {
                WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                WindowEvent::KeyboardInput { input, .. } => match input {
                    KeyboardInput {
                        state: ElementState::Pressed,
                        virtual_keycode: Some(VirtualKeyCode::Escape),
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    _ => {}
                },
                WindowEvent::Resized(physical_size) => {
                    screen_state.resize(*physical_size);
                },
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    // new_inner_size is &&mut so we have to dereference it twice
                    screen_state.resize(**new_inner_size);
                },
                _ => {}
            }
        }
        _ => {}
        });
        
    }
    else {
        info!(Core, "Starting as server - our IP (hosting from) is {:?}.", ip);
    }
    info!(Core, "Ustr cache used {} bytes of memory.", ustr::total_allocated());
    std::thread::sleep(std::time::Duration::from_millis(100));
    */
}