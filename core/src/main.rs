//! Voxel metaverse "game" you can have some fun in.
#![feature(drain_filter)]
#![feature(adt_const_params)]
#![feature(string_remove_matches)]

use log::{LevelFilter, info, error};
use mlua::{MultiValue, LuaOptions};
use std::{fs::File};

use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};

#[macro_use]
pub mod common;
pub mod entity;
pub mod script;
pub mod world;

#[allow(unused_must_use)]
fn main() {
    //let event_loop = EventLoop::new();
    //let window = WindowBuilder::new().build(&event_loop).unwrap();

    CombinedLogger::init(vec![
        TermLogger::new(
            LevelFilter::Warn,
            simplelog::Config::default(),
            TerminalMode::Mixed,
            ColorChoice::Auto,
        ),
        WriteLogger::new(
            LevelFilter::Info,
            simplelog::Config::default(),
            File::create("latest.log").unwrap(),
        ),
    ]).unwrap();

    let lua_stdlibs = mlua::StdLib::BIT | mlua::StdLib::STRING | mlua::StdLib::TABLE | mlua::StdLib::IO | mlua::StdLib::OS | mlua::StdLib::JIT | mlua::StdLib::PACKAGE;
    let vm = mlua::Lua::new_with(lua_stdlibs, LuaOptions::default()).unwrap();
    
    info!("Starting Gestalt");

    std::thread::sleep(std::time::Duration::from_millis(100));
}