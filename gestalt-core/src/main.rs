//! Voxel metaverse "game" you can have some fun in.
#![feature(drain_filter)]
#![feature(adt_const_params)]
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
pub mod script;
pub mod world;

use std::io::Write;

use common::identity::{do_keys_need_generating, does_private_key_need_passphrase, load_local_identity_keys};
use mlua::LuaOptions;
use rand_core::OsRng;

use crate::common::identity::generate_local_keys;

#[allow(unused_must_use)]
fn main() {
    /*
    let mut log_config_builder = ConfigBuilder::default();
    log_config_builder.set_target_level(LevelFilter::Error);
    let log_config = log_config_builder.build();

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Warn,
            log_config.clone(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Warn,
            log_config.clone(),
            File::create("latest.log").unwrap(),
        ),
    ]).unwrap();*/

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

    println!("Identity keys loaded! Initializing engine...");

    let lua_stdlibs = mlua::StdLib::BIT
        | mlua::StdLib::STRING
        | mlua::StdLib::TABLE
        | mlua::StdLib::IO
        | mlua::StdLib::OS
        | mlua::StdLib::JIT
        | mlua::StdLib::PACKAGE;
    let _vm = mlua::Lua::new_with(lua_stdlibs, LuaOptions::default()).unwrap();

    client::clientmain::run_client(keys);

    std::thread::sleep(std::time::Duration::from_millis(100));
}