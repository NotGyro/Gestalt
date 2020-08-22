#[macro_use] extern crate hemlock;

extern crate anyhow;
extern crate bincode;
#[macro_use] extern crate crossbeam_channel;
#[macro_use] extern crate custom_error;
extern crate hashbrown;
extern crate kiss3d;
#[macro_use] extern crate lazy_static;
extern crate log;
extern crate nalgebra as na;
extern crate num;
extern crate parking_lot;
extern crate rand;
extern crate rusty_v8;
extern crate serde;
extern crate ustr;
extern crate uuid;

use logger::hemlock_scopes;

use kiss3d::light::Light;
use kiss3d::window::Window;
use kiss3d::event::{Action, WindowEvent};

use na::{UnitQuaternion, Vector3};

use anyhow::Result;

use serde::{Serialize, Deserialize};
use std::fs::OpenOptions;
use std::fs::File;
use std::io::prelude::*;

use ron::ser::{to_string_pretty, PrettyConfig};
use ron::de::from_reader;
use rusty_v8 as v8;

pub mod world;
#[macro_use] pub mod core;

/// The main purpose of the Logger module is to define our Hemlock scopes. 
/// It also contains a https://crates.io/crates/log proxy into Hemlock, so anything 
/// logged using that crate's macros will show up as coming from the "Library" scope.
pub mod logger;

#[derive(Debug, Serialize, Deserialize)]
struct ClientConfig {
    pub resolution: (u32, u32),
}

impl Default for ClientConfig {
    fn default() -> Self { ClientConfig {resolution: (800,600)} }
}

fn main() -> Result<()> {
    match logger::init_logger() {
        Ok(_) => info!(Core, "Logger initialized."),
        Err(e) => panic!("Could not initialize logger! Reason: {}", e),
    };

    let client_config_filename = "client.ron";

    let client_config_result = OpenOptions::new().read(true)
                                                .write(true)
                                                .truncate(false)
                                                .open(client_config_filename);
    let mut create_conf_flag = false;
    let client_config: ClientConfig = match client_config_result {
        Ok(file) => {
            match from_reader(file) {
                Ok(x) => x,
                Err(e) => {
                    error!(Core, "Failed to load client config: {}", e);
                    error!(Core, "Using default client config values.");
                    ClientConfig::default()
                }
            }
        }, 
        Err(e) => {
            warn!(Core, "Failed to open {} (client config file): {}", client_config_filename, e);
            warn!(Core, "Using default client config values.");
            create_conf_flag = true;
            ClientConfig::default()
        }
    };

    // Client.ron wasn't there, create it. 
    if create_conf_flag { 
        info!(Core, "Creating {}, since it wasn't there before.", client_config_filename);
        let mut f = File::create(client_config_filename)?;
        let pretty = PrettyConfig::new().with_depth_limit(16)
                                        .with_enumerate_arrays(true);
        let s = to_string_pretty(&client_config, pretty).expect("Serialization failed");
        f.write_all(s.as_bytes())?;
        f.flush()?;
        drop(f);
    }
    
    let mut window = Window::new_with_size("Gestalt early demo", client_config.resolution.0, client_config.resolution.1);
    let mut c = window.add_cube(0.1, 0.1, 0.1);

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
        
        for mut event in window.events().iter() {
            match event.value {
                WindowEvent::Key(button, Action::Press, _) => {
                    info!(Test, "You pressed the button: {:?}", button);
                },
                _ => {},
            }
        }
    }
    Ok(())
}