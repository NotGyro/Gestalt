// External crates

#[macro_use] extern crate vulkano;
#[macro_use] extern crate vulkano_shader_derive;

extern crate cgmath;
extern crate fine_grained;
extern crate fnv;
extern crate noise;
extern crate rand;
extern crate smallvec;
extern crate winit;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate string_cache;
extern crate linear_map;
extern crate crossbeam;
extern crate serde;
extern crate serde_json;
extern crate toml;
extern crate hashbrown;
extern crate parking_lot;
extern crate clap;
extern crate num;

extern crate image;
extern crate rgb;
extern crate swsurface;
extern crate euc;
extern crate vek;

//extern crate flame;


// modules

#[macro_use] mod voxel;

mod buffer;
mod client;
mod entity;
mod game;
mod geometry;
mod input;
mod memory;
mod mesh_simplifier;
mod octree_mesher;
mod player;
mod pipeline;
mod registry;
mod renderer;
mod renderpass;
mod shader;
mod network;
mod util;
mod vulkano_win;
mod world;

// imports

use clap::{Arg, App};

use game::Game;


#[derive(PartialEq)]
pub enum NetworkRole {
    Server = 0,
    Client = 1,
    Offline = 2
}


fn main() {
    // command line parsing (currently unused)
    let matches = App::new("Gestalt Engine")
        .arg(Arg::with_name("server")
           .short("s")
           .long("server")
           .value_name("IP")
           .help("Starts a server version of this engine. No graphics. Hosts from selected IP address and socket."))
        .arg(Arg::with_name("join")
           .short("j")
           .long("join")
           .value_name("IP")
           .help("Joins a server at the selected IP address and socket.")
           .takes_value(true))
        .get_matches();

    // network roles (currently unused)
    let net_role: NetworkRole = if matches.is_present("server") {
        NetworkRole::Server
    } else {
        NetworkRole::Offline
    };

//    let server_ip = matches.value_of("server");
//
//    let join_ip = matches.value_of("join");
//    if join_ip.is_some() && net_role == NetworkRole::Server {
//        println!("Cannot host a server that also joins a server.");
//        return;
//    }
//
//    //let mut mode = game::GameMode::Singleplayer;
//    if let Some(ip) = server_ip {
//        //mode = game::GameMode::Server(ip.parse().unwrap());
//    } else if join_ip.is_some() {
//        println!("Launching to join a server at {}", join_ip.unwrap());
//        //mode = game::GameMode::JoinServer(join_ip.unwrap().parse().unwrap());
//    }

    match util::logger::init_logger() {
        Ok(_) => {},
        Err(error) => {
            println!("Unable to initialize logger. Reason: {}. Closing application.", error);
            return;
        }
    }

    if net_role == NetworkRole::Client {
        unimplemented!();
    }
    else if net_role == NetworkRole::Offline {
        Game::new().run();
    }
}