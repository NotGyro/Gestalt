//! Voxel metaverse "game" you can have some fun in.
#![feature(drain_filter)]
#![feature(string_remove_matches)]
#![feature(generic_const_exprs)]
#![feature(const_fn_trait_bound)]
#![feature(int_roundings)]
#![feature(associated_type_bounds)]

#[macro_use]
pub mod common;
#[macro_use]
pub mod resource;

pub mod client;
pub mod entity;
pub mod net;
pub mod script;
pub mod server;
pub mod world;

use std::{io::Write, path::PathBuf, net::{SocketAddr, IpAddr}, time::Duration};

use log::{LevelFilter, info, error};
use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger, ConfigBuilder};

use common::identity::{do_keys_need_generating, does_private_key_need_passphrase, load_local_identity_keys};
use hashbrown::HashSet;
use mlua::LuaOptions;

use crate::{common::identity::generate_local_keys, net::preprotocol::{launch_preprotocol_listener, preprotocol_connect_to_server}};

// For command-line argument parsing
enum OneOrTwo {
    One(String), 
    Two(String, String)
}
fn split_on_unquoted_equals(input: &str) -> OneOrTwo { 
    if input.contains(" ") { 
        //If it contains spaces, it wasn't split up already by the OS or Rust's std::env,
        //which means it's in quotes. 
        OneOrTwo::One(input.to_string());
    }
    let in_quotes = false;
    let mut previous_was_escape = false; 
    let mut position_to_split = 0;
    for (position, char) in input.chars().enumerate() { 
        if char == '\\' && !previous_was_escape { 
            previous_was_escape = true;
        }
        // OS or Rust's std::env does quote escapes, so if there's a quote here implicitly it has already been escaped. 
        // else if (char == '\"') && !previous_was_escape { 
        //    in_quotes = !in_quotes; 
        //    previous_was_escape = false;
        //}
        else if (char == '=') && !previous_was_escape && !in_quotes { 
            // We found one!
            position_to_split = position;
            break;
        }
        else { 
            previous_was_escape = false;
        }
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

#[allow(unused_must_use)]
fn main() {
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
    program_args.add_arg(vec!["--server", "-s"], false);
    program_args.add_arg(vec!["--verbose", "-v"], false);

    let matches = program_args.get_matches(arg_list);
    
    //Initialize our logger.
    let mut log_config_builder = ConfigBuilder::default();
    let level_filter = if matches.get("--verbose").is_some() { 
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    log_config_builder.set_target_level(level_filter);
    let log_config = log_config_builder.build();

    let log_file_path = PathBuf::from("logs/").join("latest.log");
    CombinedLogger::init(vec![
        TermLogger::new(
            level_filter,
            log_config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            level_filter,
            log_config.clone(),
            std::fs::File::create(log_file_path).unwrap(),
        ),
    ]).unwrap();

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

    // This doesn't do anything yet. 
    let lua_stdlibs = mlua::StdLib::BIT
        | mlua::StdLib::STRING
        | mlua::StdLib::TABLE
        | mlua::StdLib::IO
        | mlua::StdLib::OS
        | mlua::StdLib::JIT
        | mlua::StdLib::PACKAGE;
    let _vm = mlua::Lua::new_with(lua_stdlibs, LuaOptions::default()).unwrap();

    let server_mode: bool = matches.get("--server").is_some();
    
    if server_mode { 
        let (connect_sender, connect_receiver) = crossbeam_channel::unbounded();
        launch_preprotocol_listener(keys.clone(), None, connect_sender );
        loop { 
            match connect_receiver.try_recv() { 
                Ok(entry) => { 
                    info!("User {} connected", entry.peer_identity.to_base64());
                }, 
                Err(crossbeam_channel::TryRecvError::Empty) => {/* wait for more output */},
                Err(e) => { 
                    error!("Error polling for connections: {:?}", e);
                    break;
                },
            }
        }
    }
    else if let Some( ArgumentMatch{ aliases: _, parameter: Some(raw_addr) }) = matches.get("--join") { 
        let address: SocketAddr = if raw_addr.contains(":") { 
            raw_addr.parse().unwrap()
        } else { 
            let ip_addr: IpAddr = raw_addr.parse().unwrap();
            SocketAddr::new(ip_addr, net::preprotocol::PREPROTCOL_PORT)
        };

        let (connect_sender, connect_receiver) = crossbeam_channel::unbounded();
        preprotocol_connect_to_server(keys.clone(), address, Duration::new(5, 0), connect_sender );
        loop { 
            match connect_receiver.try_recv() { 
                Ok(entry) => { 
                    info!("Connected to server {}", entry.peer_identity.to_base64());
                },
                Err(crossbeam_channel::TryRecvError::Empty) => {/* wait for more output */},
                Err(e) => {
                    error!("Error polling for connections: {:?}", e);
                    break;
                },
            }
        }
    }
    //client::clientmain::run_client(keys);

    std::thread::sleep(std::time::Duration::from_millis(100));
}