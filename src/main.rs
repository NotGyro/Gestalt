//! Voxel metaverse "game" you can have some fun in.

#![allow(incomplete_features)]

#![feature(drain_filter)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]
#![feature(associated_type_bounds)]

use std::{error::Error, fs::File};
use log::{LevelFilter, info, trace};
use clap::{Arg, App};
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};
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

#[allow(unused_must_use)]
fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    

    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Warn, simplelog::Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, simplelog::Config::default(), File::create("latest.log").unwrap()),
        ]
    ).unwrap();

    
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

    std::thread::sleep(std::time::Duration::from_millis(100));
}