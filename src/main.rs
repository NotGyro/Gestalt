//! Voxel metaverse "game" you can have some fun in.

#![allow(incomplete_features)]

#![feature(drain_filter)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]
#![feature(associated_type_bounds)]

use std::{fs::File};
use log::{LevelFilter};

use simplelog::{ColorChoice, CombinedLogger, TermLogger, TerminalMode, WriteLogger};
/*
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
    window::Window,
};
*/
#[macro_use] pub mod common;
pub mod world;

#[allow(unused_must_use)]
fn main() {
    //let event_loop = EventLoop::new();
    //let window = WindowBuilder::new().build(&event_loop).unwrap();

    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Warn, simplelog::Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, simplelog::Config::default(), File::create("latest.log").unwrap()),
        ]
    ).unwrap();


    // Silly wasmtime engine test stuff. 
    let engine = wasmtime::Engine::default();
    let wat = r#"
        (module
            (import "host" "hello" (func $host_hello (param i32)))

            (func (export "hello")
                i32.const 3
                call $host_hello)
        )
    "#;
    let module = wasmtime::Module::new(&engine, wat).unwrap();

    // Build our "store", which holds data that comes in from the host for the instance.
    let mut store = wasmtime::Store::new(&engine, 4);
    // Make a function we will expose to the client 
    let host_hello = wasmtime::Func::wrap(&mut store, |caller: wasmtime::Caller<'_, u32>, param: i32| {
        println!("Got {} from WebAssembly", param);
        println!("my host state is: {}", caller.data());
    });

    let instance = wasmtime::Instance::new(&mut store, &module, &[host_hello.into()]).unwrap();
    let hello = instance.get_typed_func::<(), (), _>(&mut store, "hello").unwrap();

    hello.call(&mut store, ()).unwrap();

    std::thread::sleep(std::time::Duration::from_millis(100));
}