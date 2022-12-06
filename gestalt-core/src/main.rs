//! Voxel metaverse "game" you can have some fun in.
#![feature(drain_filter)]
#![feature(string_remove_matches)]
#![feature(generic_const_exprs)]
#![feature(int_roundings)]
#![feature(associated_type_bounds)]
#![feature(inherent_associated_types)]
#![feature(return_position_impl_trait_in_trait)]

#![allow(clippy::large_enum_variant)]

#[macro_use]
extern crate gestalt_proc_macros;

#[macro_use]
pub mod common;
pub use common::message as message;

#[macro_use]
pub mod net;

#[macro_use]
pub mod resource;

pub mod client;
pub mod entity;
pub mod message_types;
pub mod script;
pub mod server;
pub mod world;

use std::{io::Write, path::PathBuf, net::{SocketAddr, IpAddr, Ipv6Addr}, time::Duration};

use log::{LevelFilter, info, error, warn};
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger, ConfigBuilder};

use common::{identity::{do_keys_need_generating, does_private_key_need_passphrase, load_local_identity_keys}, Version, message::*, identity::NodeIdentity};
use std::collections::HashSet;
use tokio::sync::mpsc;

use crate::{
    net::{
        preprotocol::{
            launch_preprotocol_listener, 
            preprotocol_connect_to_server
        }, 
        net_channels::{
            NetSendChannel, 
            net_send_channel, 
            net_recv_channel
        }, 
        SelfNetworkRole, 
        reliable_udp::LaminarConfig, 
        NetworkSystem, default_protocol_store_dir
    }, 
    common::{
        identity::generate_local_keys
    },
    message_types::{
        voxel::{
            VoxelChangeAnnounce, 
            VoxelChangeRequest
        }, 
        JoinDefaultEntry, 
        JoinAnnounce
    }, 
    message::QuitReceiver
};

pub const ENGINE_VERSION: Version = version!(0,0,1);

// For command-line argument parsing
enum OneOrTwo {
    One(String), 
    Two(String, String)
}
fn split_on_unquoted_equals(input: &str) -> OneOrTwo { 
    if input.contains(' ') { 
        //If it contains spaces, it wasn't split up already by the OS or Rust's std::env,
        //which means it's in quotes. 
        return OneOrTwo::One(input.to_string());
    }
    let in_quotes = false;
    let mut previous_was_escape = false; 
    let mut position_to_split = 0;
    for (position, char) in input.chars().enumerate() { 
        if char == '\\' && !previous_was_escape { 
            previous_was_escape = true;
        }
        else if (char == '=') && !previous_was_escape && !in_quotes { 
            // We found one!
            position_to_split = position;
            break;
        }
        else { 
            previous_was_escape = false;
        }
        // OS or Rust's std::env does quote escapes, so if there's a quote here implicitly it has already been escaped. 
        // else if (char == '\"') && !previous_was_escape { 
        //    in_quotes = !in_quotes; 
        //    previous_was_escape = false;
        //}
    }
    if position_to_split != 0 { 
        let (left, right) = input.split_at(position_to_split);
        OneOrTwo::Two(left.to_string(), right.to_string())
    }
    else {
        OneOrTwo::One(input.to_string())
    }
} 

#[derive(Clone, Debug)]
pub struct Argument { 
    pub aliases: HashSet<String>,
    pub takes_parameter: bool,
}
#[derive(Clone, Debug)]
pub struct ArgumentMatch {
    pub aliases: HashSet<String>,
    pub parameter: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ArgumentMatches {
    pub matches: Vec<ArgumentMatch>,
}
impl ArgumentMatches { 
    pub fn get(&self, alias: &str) -> Option<ArgumentMatch> { 
        let alias = alias.to_ascii_lowercase();
        for matching_arg in self.matches.iter() { 
            if matching_arg.aliases.contains(&alias) { 
                return Some(matching_arg.clone());
            }
        }
        None
    }
}

pub struct ProgramArgs { 
    arguments: Vec<Argument>,
}

impl ProgramArgs { 
    pub fn new() -> Self { 
        ProgramArgs { 
            arguments: Vec::default(),
        }
    }
    pub fn add_arg(&mut self, aliases: Vec<&str>, takes_parameter: bool) { 
        let mut converted_aliases: Vec<String> = aliases.iter().map(|alias| alias.to_ascii_lowercase()).collect();
        let mut alias_set = HashSet::default(); 
        for alias in converted_aliases.drain(0..) {
            alias_set.insert(alias);
        }
        self.arguments.push(Argument {
            aliases: alias_set,
            takes_parameter,
        })
    }
    pub fn get_matches(&self, args: Vec<String>) -> ArgumentMatches {
        let mut match_list = Vec::new();
        for (index, arg_in) in args.iter().enumerate() { 
            let arg_in = arg_in.to_ascii_lowercase();
            for arg_def in self.arguments.iter() { 
                for alias in arg_def.aliases.iter() { 
                    if arg_in.starts_with(alias) { 
                        //We have a match! Let's see what to do with it. 
                        if arg_def.takes_parameter { 
                            match split_on_unquoted_equals(&arg_in) { 
                                OneOrTwo::One(_just_the_arg) => {
                                    //Look ahead
                                    if index+1 < args.len() {
                                        if let Some(param) = args.get(index+1) {
                                            match_list.push( ArgumentMatch {
                                                aliases: arg_def.aliases.clone(),
                                                parameter: Some(param.to_string()),
                                            })
                                        }
                                    }
                                }, 
                                OneOrTwo::Two(_arg, param) => { 
                                    match_list.push( ArgumentMatch {
                                        aliases: arg_def.aliases.clone(),
                                        parameter: Some(param),
                                    })
                                }
                            }
                        }
                        else {
                            match_list.push( ArgumentMatch {
                                aliases: arg_def.aliases.clone(),
                                parameter: None,
                            })
                        }
                    }
                }
            }
        }
        ArgumentMatches { 
            matches: match_list,
        }
    }
}

impl Default for ProgramArgs {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn protocol_key_change_approver(mut receiver: BroadcastReceiver<NodeIdentity>, sender: BroadcastSender<(NodeIdentity, bool)>) {
    loop {     
        match receiver.recv_wait().await { 
            Ok(idents) => for ident in idents {
                warn!("Protocol key has changed for peer {:?} - most likely this is the same user \n\
                    connecting with a new device, but it's possible it's an attempt to impersonate them.", 
                    ident.to_base64());
                //Approve implicitly.
                //When GUI is a thing, we want this to generate a popup for clients. 
                sender.send_one((ident.clone(), true)).unwrap();
            },
            Err(e) => panic!("Protocol key change approver channel died: {:?}", e), 
        }
    }
}

global_channel!(BroadcastChannel, PROTOCOL_KEY_REPORTER, NodeIdentity, 1024);
global_channel!(BroadcastChannel, PROTOCOL_KEY_APPROVER, (NodeIdentity, bool), 1024);

#[allow(unused_must_use)]
fn main() {
    // Announce the engine launching, for our command-line friends. 
    println!("Launching Gestalt Engine v{}", ENGINE_VERSION);
    // Parse command-line arguments
    let mut arg_list: Vec<String> = Vec::new();
    for argument in std::env::args() {
        // Skip initial "here is your directory" argument
        if !( argument.contains("gestalt_core.exe") || argument.contains("gestalt.exe") ) {
            arg_list.push(argument);
        }
    }
    let mut program_args = ProgramArgs::new(); 
    program_args.add_arg(vec!["--join", "-j"], true);
    program_args.add_arg(vec!["--server", "-s"], true);
    program_args.add_arg(vec!["--verbose", "-v"], false);
    program_args.add_arg(vec!["--nosave", "-n"], false);

    let matches = program_args.get_matches(arg_list);
    
    //Initialize our logger.
    let mut log_config_builder = ConfigBuilder::default();
    
    let level_filter = { 
        if let Some(_) = matches.get("--verbose") {
            // Verbosely log Gestalt messages but try not to verbosely log renderer messages because they can get ridiculous.
            log_config_builder.add_filter_ignore_str("wgpu");
            log_config_builder.add_filter_ignore_str("rend3");
            log_config_builder.add_filter_ignore_str("naga");
            log_config_builder.set_location_level(LevelFilter::Error);
            LevelFilter::Trace
        }
        else { 
            log_config_builder.add_filter_ignore_str("wgpu_core::device");
            LevelFilter::Info
        }
    };

    log_config_builder.set_target_level(level_filter);

    let log_config = log_config_builder.build();

    let log_dir = PathBuf::from("logs/"); 
    let log_file_path = log_dir.join("latest.log");

    if !log_dir.exists() { 
        std::fs::create_dir(log_dir);
    }

    CombinedLogger::init(vec![
        TermLogger::new(
            level_filter,
            log_config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            level_filter,
            log_config,
            std::fs::File::create(log_file_path).unwrap(),
        ),
    ]).unwrap();

    if matches!(level_filter, LevelFilter::Trace) { 
        warn!("Verbose logging CAN, OCCASIONALLY, LEAK PRIVATE INFORMATION. \n It is only recommended for debugging purposes. \n Please do not use it for general play.");
    }

    // Load our identity key pair. Right now this will be the same on both client and server - that will change later. 
    let keys = if do_keys_need_generating() {
        println!("No identity keys found, generating identity keys.");
        println!("Optionally enter a passphrase.");
        println!("Minimum length is 4 characters.");
        println!("WARNING: If you forget your passphrase, this will be impossible to recover!");
        println!("Leave this blank if you do not want to use a passphrase.");
        print!("Enter your passphrase: ");
        let _ = std::io::stdout().flush();
        
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).expect("Error reading from STDIN");

        let passphrase = if input.chars().count() > 4 {
            Some(input)
        } else {
            None
        };

        generate_local_keys(passphrase).unwrap()
    } else { 
        let passphrase = if does_private_key_need_passphrase().unwrap() { 
            println!("Your identity key is encrypted. Please enter your passphrase.");
            print!("Passphrase: ");
            let _ = std::io::stdout().flush();
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).expect("Error reading from STDIN");
            Some(input)
        } else {
            None
        };
        load_local_identity_keys(passphrase).unwrap()
    };

    info!("Identity keys loaded! Initializing engine...");

    info!("Setting up async runtime.");
    let mut runtime_builder = tokio::runtime::Builder::new_multi_thread();
    runtime_builder.enable_all();

    let async_runtime = match runtime_builder.build() { 
        Ok(rt) => rt, 
        Err(e) => { 
            error!("Unable to start async runtime: {:?}", e); 
            panic!("Unable to start async runtime: {:?}", e);
        }
    };

    info!("Setting up channels.");
    
    async_runtime.spawn( 
        protocol_key_change_approver(
            PROTOCOL_KEY_REPORTER.receiver_subscribe(), 
            PROTOCOL_KEY_APPROVER.sender_subscribe(), 
        ) 
    );

    let mut laminar_config = LaminarConfig::default();
    laminar_config.heartbeat_interval = Some(Duration::from_secs(1));

    let protocol_store_dir = default_protocol_store_dir();

    if let Some( ArgumentMatch{ aliases: _, parameter: addr } ) = matches.get("--server") { 
        info!("Launching as server - parsing address.");
        let (connect_sender, connect_receiver) = mpsc::unbounded_channel();

        let udp_address = if let Some(raw_addr) = addr { 
            if raw_addr.contains(':') { 
                raw_addr.parse().unwrap()
            } else { 
                let ip_addr: IpAddr = raw_addr.parse().unwrap();
                SocketAddr::new(ip_addr, 3223)
            }
        }
        else { 
            SocketAddr::from((Ipv6Addr::LOCALHOST, 3223))
        };

        info!("Spawning preprotocol listener task.");
        async_runtime.spawn(
            launch_preprotocol_listener(keys, 
                None, 
                connect_sender, 
                3223, 
                protocol_store_dir, 
                PROTOCOL_KEY_REPORTER.clone(),
                PROTOCOL_KEY_APPROVER.clone()
            )
        );

        info!("Spawning network system task.");
        let keys_for_net = keys.clone();
        let net_system_join_handle = async_runtime.spawn(
            async move {
                let mut sys = NetworkSystem::new(SelfNetworkRole::Server,
                udp_address, 
                connect_receiver,
                keys_for_net,
                laminar_config,
                Duration::from_millis(25)).await.unwrap();
                sys.run().await
            }
        );

        //let test_world_range: VoxelRange<i32> = VoxelRange{upper: vpos!(3,3,3), lower: vpos!(-2,-2,-2) };
        //let mut world_space = TileSpace::new();
        //for chunk_position in test_world_range {
        //    let chunk = gen_test_chunk(chunk_position);
        //    world_space.ingest_loaded_chunk(chunk_position, chunk).unwrap();
        //}
        
        // Set up our test world a bit 
        //let mut world_space = TileSpace::new();
        //let test_world_range: VoxelRange<i32> = VoxelRange{upper: vpos!(3,3,3), lower: vpos!(-2,-2,-2) };

        //let world_id = get_lobby_world_id(&keys.public);
        //load_or_generate_dev_world(&mut world_space, &world_id, test_world_range, None).unwrap();
        
        info!("Launching server mainloop.");
        let mut total_changes: Vec<VoxelChangeAnnounce> = Vec::new();
        async_runtime.block_on(async move { 
            let mut quit_receiver = QuitReceiver::new();
            let mut server_voxel_receiver_from_client = net_recv_channel::subscribe::<VoxelChangeRequest>().unwrap();
            let mut server_join_receiver_from_client = net_recv_channel::subscribe::<JoinDefaultEntry>().unwrap();
            loop {
                tokio::select! { 
                    voxel_events_maybe = server_voxel_receiver_from_client.recv_wait() => { 
                        if let Ok(voxel_events) = voxel_events_maybe { 
                            for (ident, event) in voxel_events { 
                                //world_space.set(event.pos, event.new_tile).unwrap();
                                info!("Received {:?} from {}", &event, ident.to_base64());
                                let announce: VoxelChangeAnnounce = event.into();
                                net_send_channel::send_one_to_all_except(announce.clone(), &ident).unwrap();
                                total_changes.push(announce);
                            }
                        }
                    }
                    join_event_maybe = server_join_receiver_from_client.recv_wait() => { 
                        if let Ok(events) = join_event_maybe { 
                            for (ident, event) in events {
                                info!("User {} has joined with display name {}", ident.to_base64(), &event.display_name);
                                let announce = JoinAnnounce {
                                    display_name: event.display_name, 
                                    identity: ident,
                                };
                                net_send_channel::send_one_to_all_except(announce.clone(), &ident).unwrap();
                                info!("Sending all previous changes to the newly-joined user.");
                                net_send_channel::send_multi_to(total_changes.clone(), &ident).unwrap();
                            }
                        }
                    }
                    quit_ready_indicator = quit_receiver.wait_for_quit() => { 
                        quit_ready_indicator.notify_ready();
                        break;
                    }
                }
            }
        });
        message::quit_game(Duration::from_secs(10));
        async_runtime.block_on(net_system_join_handle);
    }
    else if let Some( ArgumentMatch{ aliases: _, parameter: Some(raw_addr) }) = matches.get("--join") {
        let address: SocketAddr = if raw_addr.contains(':') { 
            raw_addr.parse().unwrap()
        } else {
            let ip_addr: IpAddr = raw_addr.parse().unwrap();
            SocketAddr::new(ip_addr, 3223)
        };

        let (connect_sender, connect_receiver) = mpsc::unbounded_channel();
        let keys_for_net = keys.clone();
        let net_system_join_handle = async_runtime.spawn(
            async move { 
                let mut sys = NetworkSystem::new( SelfNetworkRole::Client,  address, 
                    connect_receiver,
                    keys_for_net, 
                    laminar_config,
                    Duration::from_millis(25) ).await.unwrap();
                sys.run().await
            }
        );
        let completed = async_runtime.block_on(
            preprotocol_connect_to_server(
                keys, 
                address, 
                Duration::new(5, 0),
                protocol_store_dir,
                PROTOCOL_KEY_REPORTER.sender_subscribe(), 
                PROTOCOL_KEY_APPROVER.receiver_subscribe(), 
            )
        ).unwrap();
        let server_identity = completed.peer_identity.clone();
        connect_sender.send(completed).unwrap();

        std::thread::sleep(Duration::from_millis(50));
                
        let _voxel_event_sender: NetSendChannel<VoxelChangeRequest> = net_send_channel::subscribe_sender(&server_identity).unwrap();
        
        let mut client_join_receiver_from_server = net_recv_channel::subscribe::<JoinAnnounce>().unwrap();
        let _client_voxel_receiver_from_server = net_recv_channel::subscribe::<VoxelChangeAnnounce>().unwrap();

        async_runtime.spawn( async move { 
            loop { 
                match client_join_receiver_from_server.recv_wait().await { 
                    Ok(join_msgs) => {
                        for (_server_ident, JoinAnnounce{identity, display_name }) in join_msgs { 
                            info!("Peer {} joined with display name {}", identity.to_base64(), &display_name);
                        }
                    }
                    Err(_e) => { 
                        info!("New join handler closed.");
                        break;
                    }
                }
            }
        });

        async_runtime.spawn( async move {
            let mut quit_receiver = QuitReceiver::new();
            let quit_ready = quit_receiver.wait_for_quit().await;
            net_system_join_handle.await; //This is why quit_ready_sender exists. Make sure that's all done. 
            quit_ready.notify_ready();
        });
        /*
        client::clientmain::run_client(keys, 
                voxel_event_sender, 
                client_voxel_receiver_from_server, 
                Some(server_identity),
                async_runtime,
            );
        */
    }
    else {
        /*
        let (voxel_event_sender, mut voxel_event_receiver) = tokio::sync::broadcast::channel(4096); 
        let voxel_event_sender = NetSendChannel::new(voxel_event_sender); 

        let client_voxel_receiver_from_server = net_recv_channel::subscribe::<VoxelChangeAnnounce>().unwrap();

        async_runtime.spawn( async move {
            loop { 
                //redirect to /dev/null
                let _ = voxel_event_receiver.recv().await;
            }
        });
        client::clientmain::run_client(keys, 
            voxel_event_sender, 
            client_voxel_receiver_from_server, 
            None,
            async_runtime,
            );
        */
    }
}