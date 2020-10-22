//! Voxel metaverse "game" you can have some fun in.

#![feature(drain_filter)]
#![feature(seek_convenience)]
#![feature(const_int_pow)]

#[macro_use] extern crate hemlock;

#[macro_use] extern crate arr_macro;
extern crate bincode;
extern crate blake3;
extern crate clap;
extern crate crossbeam_channel;
#[macro_use] extern crate custom_error;
#[macro_use] extern crate enum_dispatch;
#[macro_use] extern crate glium;
extern crate glutin;
extern crate hashbrown;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate legion;
extern crate libp2p;
extern crate num;
extern crate parking_lot;
extern crate rand;
extern crate rusty_v8;
extern crate semver;
extern crate serde;
extern crate ustr;
extern crate uuid;

use logger::hemlock_scopes;
use clap::{Arg, App};
use std::net::SocketAddr;

#[macro_use] pub mod common;
pub mod client;
pub mod entity;
pub mod world;
/// The main purpose of the Logger module is to define our Hemlock scopes. 
/// It also contains a https://crates.io/crates/log proxy into Hemlock, so anything 
/// logged using that crate's macros will show up as coming from the "Library" scope.
pub mod logger;

#[allow(unused_must_use)]
fn main() {
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
    if !is_server {
        info!(Core, "Starting as client - join IP is {:?} (singleplayer if none).", ip);
        crate::client::run_client(ip);
    }
    else {
        info!(Core, "Starting as server - our IP (hosting from) is {:?}.", ip);
    }
}