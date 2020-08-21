extern crate anyhow;
#[macro_use] extern crate crossbeam_channel;
extern crate hashbrown;
extern crate kiss3d;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate nalgebra as na;
extern crate num;
extern crate parking_lot;
extern crate rand;
extern crate rusty_v8;
extern crate serde;
extern crate serde_json;
//extern crate ustr;
extern crate uuid;

use kiss3d::light::Light;
use kiss3d::window::Window;
use na::{UnitQuaternion, Vector3};
use anyhow::Result;

use std::fs::File;
use std::io::prelude::*;

use rusty_v8 as v8;

pub mod world;
#[macro_use] pub mod util;

//use wasmtime::*;

fn main() -> Result<()> {
    match util::logger::init_logger() {
        Ok(_) => info!("Logger initialized."),
        Err(e) => panic!("Could not initialize logger! Reason: {}", e),
    };
    
    let mut window = Window::new("Kiss3d: cube");
    let mut c = window.add_cube(1.0, 1.0, 1.0);

    let platform = v8::new_default_platform().unwrap();
    v8::V8::initialize_platform(platform);
    v8::V8::initialize();

    let isolate = &mut v8::Isolate::new(Default::default());

    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope);
    let scope = &mut v8::ContextScope::new(scope, context);

    let code = v8::String::new(scope, "'Hello' + ' World!'").unwrap();
    println!("javascript code: {}", code.to_rust_string_lossy(scope));

    let mut script = v8::Script::compile(scope, code, None).unwrap();
    let result = script.run(scope).unwrap();
    let result = result.to_string(scope).unwrap();
    println!("result: {}", result.to_rust_string_lossy(scope));

    c.set_color(1.0, 0.0, 0.0);

    window.set_light(Light::StickToCamera);

    let rot = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.014);

    while window.render() {
        c.prepend_to_local_rotation(&rot);
    }
    Ok(())
}