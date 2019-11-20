// External crates

#[macro_use] extern crate vulkano;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;

extern crate cgmath;
extern crate fine_grained;
extern crate fnv;
extern crate futures;
extern crate noise;
extern crate rand;
extern crate smallvec;
extern crate winit;
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
extern crate rustls;
extern crate rusttype;
extern crate image;
extern crate rcgen;
extern crate rgb;
extern crate sodiumoxide;
extern crate tokio;
extern crate vulkano_shaders;
extern crate webpki;

// modules

#[macro_use] pub mod voxel;

pub mod buffer;
pub mod client;
pub mod entity;
pub mod game;
pub mod geometry;
pub mod input;
pub mod memory;
pub mod chunk_mesher;
pub mod network;
pub mod player;
pub mod pipeline;
pub mod registry;
pub mod renderer;
pub mod renderpass;
pub mod shader;
pub mod util;
pub mod vulkano_win;
pub mod world;

// imports

use clap::{Arg, App};

use game::Game;

use sodiumoxide::crypto::sign;

use network::NetworkRole;

pub fn main() {
    match util::logger::init_logger() {
        Ok(_) => {},
        Err(error) => {
            println!("Unable to initialize logger. Reason: {}. Closing application.", error);
            return;
        }
    }

    match sodiumoxide::init() {
        Ok(()) => {},
        Err(()) => {
            error!("Unable to initialize cryptography library!");
            panic!();
        },
    };

    let our_identity = match network::IdentitySelf::init() {
        Ok(ident) => ident,
        Err(e) => {
            error!("Could not initialize our cryptographic identity! Reason: {}", e);
            panic!()
        }
    };

    let example_data : u64 = rand::random();
    let sig = our_identity.sign(&example_data.to_le_bytes());
    assert!( sign::verify_detached(&sig, &example_data.to_le_bytes(), &our_identity.public_key) );
    info!("Confirmed our cryptographic identity is valid - data signed by our secret key can be verified by our public key.");

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

    if net_role == NetworkRole::Client {
        unimplemented!();
    }
    else if net_role == NetworkRole::Offline {
        Game::new().run();
    }
}