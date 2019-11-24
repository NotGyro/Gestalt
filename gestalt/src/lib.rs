// External crates

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate lumberjack;
extern crate make_names;

extern crate bincode;
extern crate cgmath;
extern crate clap;
extern crate crossbeam_channel;
extern crate euc;
extern crate fine_grained;
extern crate futures;
extern crate hashbrown;
extern crate image;
extern crate linear_map;
extern crate noise;
extern crate num;
extern crate parking_lot;
extern crate rand;
extern crate rgb;
extern crate serde;
extern crate serde_json;
extern crate smallvec;
extern crate string_cache;
extern crate swsurface;
extern crate toml;
extern crate vek;
extern crate phosphor;
extern crate clipboard;
extern crate sodiumoxide;
//extern crate tokio;

// modules

#[macro_use] pub mod voxel;
#[macro_use] mod names;

pub mod client;
pub mod clipboard_backend;
pub mod chunk_mesher;
pub mod entity;
pub mod game;
pub mod input;
pub mod metrics;
pub mod player;
#[macro_use] pub mod network;
pub mod util;
pub mod world;

// imports

use std::net::SocketAddr;
use clap::{Arg, App};
use game::Game;

#[allow(non_upper_case_globals)]
pub mod lumberjack_scopes {
    use lumberjack::prelude::*;
    use lumberjack::Verbosity::*;

    lazy_static! {
        pub static ref Test:     u32 = lumberjack::register_scope(Scope::new("Test").log(Verbose).print(Warning));
        pub static ref Game:     u32 = lumberjack::register_scope(Scope::new("Game"));
        pub static ref Network:  u32 = lumberjack::register_scope(Scope::new("Network").print(Info));
        pub static ref Renderer: u32 = lumberjack::register_scope(Scope::new("Renderer"));
        pub static ref Mesher:   u32 = lumberjack::register_scope(Scope::new("Mesher"));
    }
}


use sodiumoxide::crypto::sign;
use network::{NetworkRole, ClientNet, ServerNet, NetMsg, PacketGuarantees, StreamSelector, ServerToClient, ClientToServer};
use std::time::{Duration, Instant};
use serde::{Serialize,Deserialize};

#[derive(Clone, Serialize, Deserialize)]
struct HelloMessage {
    pub hello: String,
}
impl HelloMessage {
    pub fn new() -> Self {
        HelloMessage {
            hello: "Hello, friendo!".to_owned(),
        }
    }
}

impl_netmsg!(HelloMessage, ClientToServer, 1, ReliableUnordered, 1);
impl_netmsg!(HelloMessage, ServerToClient, 4, ReliableUnordered, 1);



pub fn main() {
    match sodiumoxide::init() {
        Ok(()) => {},
        Err(()) => {
            error!(Network, "Unable to initialize cryptography library!");
            panic!();
        },
    };

    let our_identity = match network::SelfIdentity::init() {
        Ok(ident) => ident,
        Err(e) => {
            error!(Network, "Could not initialize our cryptographic identity! Reason: {}", e);
            panic!()
        }
    };

    let example_data: u64 = rand::random();
    let sig = our_identity.sign(&example_data.to_le_bytes());
    assert!(sign::verify_detached(&sig, &example_data.to_le_bytes(), &our_identity.public_key));
    info!(Network, "Confirmed our cryptographic identity is valid - data signed by our secret key can be verified by our public key.");

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
    } else if matches.is_present("join") {
        NetworkRole::Client
    } else {
        NetworkRole::Offline
    };

    let server_ip = matches.value_of("server");

    let join_ip = matches.value_of("join");
    if join_ip.is_some() && net_role == NetworkRole::Server {
        println!("Cannot host a server that also joins a server.");
        return;
    }

//    //let mut mode = game::GameMode::Singleplayer;
//    if let Some(ip) = server_ip {
//        //mode = game::GameMode::Server(ip.parse().unwrap());
//    } else if join_ip.is_some() {
//        println!("Launching to join a server at {}", join_ip.unwrap());
//        //mode = game::GameMode::JoinServer(join_ip.unwrap().parse().unwrap());
//    }

//    match util::logger::init_logger() {
//        Ok(_) => {},
//        Err(error) => {
//            println!("Unable to initialize logger. Reason: {}. Closing application.", error);
//            return;
//        }
//    }


    else if net_role == NetworkRole::Offline {
    //let mut rt = tokio::runtime::Runtime::new().unwrap();
        let start_time = Instant::now();
        if net_role == NetworkRole::Client {
            let join_ip_inner : SocketAddr = join_ip.unwrap().parse().unwrap();
            let mut client_net = ClientNet::new(&our_identity);
            client_net.connect(join_ip_inner).unwrap();
            let listener = client_net.listen_from_servers::<HelloMessage>().unwrap();

            let mut sent_test_message = false;
            //Early development - just to test.
            while Instant::now() - start_time < Duration::from_secs(45) {
                client_net.process().unwrap();
                if !sent_test_message && Instant::now() - start_time >= Duration::from_secs(5) {
                    let hello = HelloMessage::new();
                    client_net.send_to_server(&hello).unwrap();
                    sent_test_message = true
                }

                match listener.poll() {
                    Ok(tuple) => {
                        println!("Server said: {}", tuple.0.hello);
                    },
                    Err(_) => {},
                }
            }
            Game::new().run();
        }
        else if net_role == NetworkRole::Server {
            let server_ip_inner : SocketAddr = server_ip.unwrap().parse().unwrap();
            let mut server_net = ServerNet::new(&our_identity, server_ip_inner).unwrap();
            let listener = server_net.listen_from_clients::<HelloMessage>().unwrap();
            let new_client_listener = server_net.listen_new_clients();
            //Early development - just to test.
            while Instant::now() - start_time < Duration::from_secs(45) {
                server_net.process().unwrap();
                match new_client_listener.try_recv() {
                    Ok(event) => {
                        server_net.send_to_client(&HelloMessage::new(), &event.identity).unwrap();
                    },
                    Err(_) => {},
                }
                match listener.poll() {
                    Ok(tuple) => {
                        println!("Client said: {}", tuple.0.hello);
                    },
                    Err(_) => {},
                }
            }
        }
        else if net_role == NetworkRole::Offline {
            Game::new().run();
        }
    }
}
