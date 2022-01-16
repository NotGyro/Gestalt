//! Voxel metaverse "game" you can have some fun in.
#![feature(drain_filter)]
#![feature(adt_const_params)]
#![feature(string_remove_matches)]
#![feature(generic_const_exprs)]
#![feature(const_fn_trait_bound)]

#[macro_use]
pub mod common;
#[macro_use]
pub mod resource;

pub mod client;
pub mod entity;
pub mod script;
pub mod world;

use hashbrown::HashSet;
use log::{LevelFilter, info, error};
use mlua::{MultiValue, LuaOptions};
use winit::event::{VirtualKeyCode, ElementState};
use std::{fs::File, sync::Arc, time::Instant};

use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger, ConfigBuilder};

use glam::{Vec3, Quat};

use client::camera as camera;

#[allow(unused_must_use)]
fn main() {
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
            LevelFilter::Debug,
            log_config.clone(),
            File::create("latest.log").unwrap(),
        ),
    ]).unwrap();
    
    let lua_stdlibs = mlua::StdLib::BIT | mlua::StdLib::STRING | mlua::StdLib::TABLE | mlua::StdLib::IO | mlua::StdLib::OS | mlua::StdLib::JIT | mlua::StdLib::PACKAGE;
    let vm = mlua::Lua::new_with(lua_stdlibs, LuaOptions::default()).unwrap();

    client::clientmain::run_client();

    std::thread::sleep(std::time::Duration::from_millis(100));
}