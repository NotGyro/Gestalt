#![allow(dead_code)]

extern crate cgmath;
extern crate fine_grained;
extern crate fnv;
extern crate noise;
extern crate rand;
extern crate smallvec;
extern crate winit;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate log;
extern crate string_cache;
extern crate linear_map;
extern crate crossbeam;
extern crate serde;
extern crate serde_json;
extern crate toml;
extern crate hashbrown;
extern crate parking_lot;

extern crate image;
extern crate rgb;
extern crate swsurface;
extern crate euc;
extern crate vek;

//#[macro_use] extern crate vulkano;
//#[macro_use] extern crate vulkano_shader_derive;
//extern crate image;

#[macro_use] mod voxel;

mod util;
mod world;
mod network;
mod entity;
mod client;

extern crate clap;
use clap::{Arg, App};

use voxel::voxelmath::*;
use voxel::subdivmath::*;
use voxel::subdivstorage::*;
use world::tile::*;

use swsurface::{Format, SwWindow};
use winit::{
    event::{Event, WindowEvent, ElementState, KeyboardInput},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit::dpi::LogicalSize;
use winit::platform::desktop::EventLoopExtDesktop;

use cgmath::{Angle, Matrix4, Vector3, Vector4, Point3, InnerSpace, Rotation, Rotation3, Quaternion, Deg, Rad, BaseFloat, BaseNum};
use std::ops::Neg;
use self::string_cache::DefaultAtom as Atom;
use rgb::*;
use std::time::{Duration, Instant};

use std::fs::File;

fn main() {
    let matches = App::new("Gestalt Engine").arg(Arg::with_name("server")
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

    let server_mode : bool = matches.is_present("server");

    let server_ip = matches.value_of("server");

    let join_ip = matches.value_of("join");
    if join_ip.is_some() && server_mode {
        println!("Cannot host a server that also joins a server.");
        return;
    }

    //let mut mode = game::GameMode::Singleplayer;
    if let Some(ip) = server_ip {
        //mode = game::GameMode::Server(ip.parse().unwrap());
    } else if join_ip.is_some() {
        println!("Launching to join a server at {}", join_ip.unwrap());
        //mode = game::GameMode::JoinServer(join_ip.unwrap().parse().unwrap());
    }

    match util::logger::init_logger() {
        Ok(_) => {},
        Err(error) => { println!("Unable to initialize logger. Reason: {}. Closing application.", error); return; }
    }

    if !server_mode { 
        
        // Set up our display properties.
        let window_width : u32 = 800;
        let window_height : u32 = 600;
        let fov : cgmath::Rad<f64> = cgmath::Rad::from(cgmath::Deg(100.0 as f64));

        //Open a Winit window.
        let mut event_loop = EventLoop::new();
        let window = WindowBuilder::new()
            .with_title("gestalt")
            .with_inner_size(LogicalSize::from((window_width, window_height)))
            .with_resizable(false)
            .build(&event_loop)
            .unwrap();
        let event_loop_proxy = event_loop.create_proxy();
        let sw_context = swsurface::ContextBuilder::new(&event_loop)
            //.with_ready_cb(move |_| {
            //    let _ = event_loop_proxy.send_event(());
            //})
            .build();
        let sw_window = SwWindow::new(window, &sw_context, &Default::default());
        let format = [Format::Argb8888]
            .iter()
            .cloned()
            .find(|&fmt1| sw_window.supported_formats().any(|fmt2| fmt1 == fmt2))
            .unwrap();
            
        sw_window.update_surface_to_fit(format);

        // Set up our player and where we're looking
        let player_pos : Point3<f64> = Point3::new(10.0, -10.0, 24.0);
        
        let yaw = Rad::from(Deg(60.0 as f64));
        let pitch = Rad::from(Deg(-80.0 as f64));
        let yaw_quat : Quaternion<f64> = Quaternion::from_angle_y( yaw );
        let pitch_quat : Quaternion<f64> = Quaternion::from_angle_x( pitch );
        let rotation = (yaw_quat * pitch_quat).normalize();
        let mut forward : Vector3<f64> = Vector3::new(0.0, 0.0, -1.0);
        let mut right : Vector3<f64> = Vector3::new(0.0, 1.0, 0.0);
        
        forward = rotation.rotate_vector(forward);
        right = rotation.rotate_vector(right);

        let up = forward.cross( right ).neg();

        // Create a test / example world. 

        // Describe some tiles. 
        let air_id = TILE_REGISTRY.lock().register_tile(&Atom::from("air"));
        //TILE_TO_ART.write().insert(air_id, TileArt{color: RGB{r:255,g:255,b:255}, air:true });
        let stone_id = TILE_REGISTRY.lock().register_tile(&Atom::from("stone"));
        //TILE_TO_ART.write().insert(stone_id, TileArt{color: RGB{r:134,g:139,b:142}, air:false });
        let lava_id = TILE_REGISTRY.lock().register_tile(&Atom::from("lava"));
        //TILE_TO_ART.write().insert(lava_id, TileArt{color: RGB{r:255,g:140,b:44}, air:false });

        // Scale 6: a 64 meter x 64 meter x 64 meter chunk
        let mut world : NaiveVoxelOctree<TileID, ()> = 
            NaiveVoxelOctree{scale : 4 , root: NaiveOctreeNode::new_leaf(stone_id)};
        world.set(opos!((1,0,1) @ 3), air_id).unwrap();
        world.set(opos!((0,0,1) @ 3), air_id).unwrap();
        world.set(opos!((1,0,0) @ 3), air_id).unwrap();
        let lava_pos = opos!((2,1,2) @ 2);
        world.set(lava_pos, lava_id).unwrap();
        let mut chop_pos = lava_pos.scale_to(1); 
        chop_pos.pos.x += 1;
        //chop_pos.pos.y += 1;
        chop_pos.pos.z += 1;
        world.set(chop_pos, air_id).unwrap();
        //world.set(opos!((0,1,0) @ 3), lava_id).unwrap();
        world.root.rebuild_lod();
        
        let game_start = Instant::now();
        // Here goes a mainloop.
        event_loop.run_return(move |event, window, control_flow| {
            let mut quit_game : bool = false;
            //*control_flow = ControlFlow::Poll; 
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput {
                            input:
                                KeyboardInput {
                                    state: ElementState::Released,
                                    virtual_keycode: Some(key),
                                    modifiers,
                                    ..
                                },
                            ..
                    } => {
                        use winit::event::VirtualKeyCode::*;
                        match key {
                            Escape => quit_game = true,
                            _ => *control_flow = ControlFlow::Poll,
                        }
                    },
                    _ => *control_flow = ControlFlow::Poll,
                },
                _ => *control_flow = ControlFlow::Poll,
            }
            if game_start.elapsed().as_secs_f32() > 6.0 {
                quit_game = true;
            }
            if *control_flow != ControlFlow::Exit {
                /*
                if let Some(image_index) = sw_window.poll_next_image() {
                    let frame_begin = Instant::now();
                    info!("Starting a frame raycast draw...");
                    match renderer.draw_frame(player_pos, yaw, pitch, &world, &mut sw_window.lock_image(image_index), sw_window.image_info()) {
                        Err(err) => {
                            error!("Error encountered while attempting to draw a frame in software rendering: {}", err);
                            *control_flow = ControlFlow::Exit; 
                            },
                        _ => info!("Drew a frame in {} milliseconds.", frame_begin.elapsed().as_millis()),
                    }
                    sw_window.present_image(image_index);
                };*/
            }
            if quit_game {
                *control_flow = ControlFlow::Exit;
            }
            if *control_flow == ControlFlow::Exit { 
                panic!();
            }
            /*
            else {
                //If we don't do THIS then the loop is fully immortal.
                panic!();
            }*/
        });
    }
}