pub mod renderer;
pub mod tileart;
pub mod input;

use cgmath::{Angle, Matrix4, Vector3, Point3, InnerSpace, Rotation, Rotation3, Quaternion, Rad};

use glium::backend::glutin::Display;
use glutin::window::*;
use glutin::event::*;
use glutin::event_loop::*;
use glutin::ContextBuilder;
use glutin::dpi::PhysicalPosition;
use glium::Surface;

use serde::{Serialize, Deserialize};
use std::fs::OpenOptions;
use std::fs::File;
use std::io::prelude::*;
use std::error::Error;
//use std::time::Duration;
use core::ops::Neg;
use num::Zero;

use ron::ser::{to_string_pretty, PrettyConfig};
use ron::de::from_reader;
//use rusty_v8 as v8;
use ustr::*;
use std::time::*;
use std::net::SocketAddr;


use crate::common::voxelmath::*;
use crate::world::*;
use crate::client::tileart::*;
use crate::world::TileId;


#[derive(Clone, Debug, Serialize, Deserialize)]
struct ClientConfig {
    pub resolution: (u32, u32),
    pub fov: f32,
}

fn make_display(conf : &ClientConfig) -> std::result::Result<(Display, EventLoop<()>), Box<dyn Error>> {
    let el = EventLoop::new();

    let wb = WindowBuilder::new()
        .with_title("Gestalt")
        .with_inner_size(glutin::dpi::LogicalSize{ width: conf.resolution.0, height: conf.resolution.1});

    let cb = ContextBuilder::new().with_depth_buffer(24);
    
    Ok( (Display::new(wb, cb, &el)?, el))
}

impl Default for ClientConfig {
    fn default() -> Self { ClientConfig {resolution: (800,600), fov: 90.0} }
}


#[allow(unused_variables)]
#[allow(unused_must_use)]
pub fn run_client(join: Option<SocketAddr>) -> Result<(), Box<dyn Error>> {
    let air = ustr("air");
    let stone = ustr("stone");
    let dirt = ustr("dirt");
    let grass = ustr("grass");

    let mut space = Space::new();

    for x in -2 .. 2 {
        for y in -1 .. 4 {
            for z in -2 .. 2 {
                space.load_or_gen_chunk(vpos!(x,y,z)).unwrap();
            }
        }
    }

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

    //---- Set up window ----

    let (display, event_loop) = make_display(&client_config).unwrap();

    let mut vshaderfile = File::open("vertexshader.glsl").unwrap();
    let mut fshaderfile = File::open("fragmentshader.glsl").unwrap();
    let mut vertex_shader_src = String::new();
    let mut fragment_shader_src = String::new();
    vshaderfile.read_to_string(&mut vertex_shader_src).unwrap();
    fshaderfile.read_to_string(&mut fragment_shader_src).unwrap();

    let program = glium::Program::from_source(&display, vertex_shader_src.as_ref(), fragment_shader_src.as_ref(), None).unwrap();

    let params = glium::DrawParameters {
        depth: glium::Depth {
            test: glium::draw_parameters::DepthTest::IfLess,
            write: true,
            .. Default::default()
        }, 
        backface_culling : glium::draw_parameters::BackfaceCullingMode::CullClockwise,
        .. Default::default()
    };
    
    //---- Set up our camera ----
    
	let mut camera_pos : Point3<f32> = Point3 {x : 0.0, y : 66.0, z : 10.0}; 
	
	let mouse_sensitivity : f32 = 0.0015;
	let move_speed : f32 = 160.0;
	let mut horz_angle : Rad<f32> = Rad::zero();
    let mut vert_angle : Rad<f32> = Rad::zero();
    
    let fovy = Rad::from( cgmath::Deg(client_config.fov) );
    
    let perspective : cgmath::PerspectiveFov<f32> = cgmath::PerspectiveFov { fovy : fovy, 
                        aspect : client_config.resolution.0 as f32 / client_config.resolution.1 as f32,
                        near : 0.1, far : 1024.0};
    
    //---- Set up our texture(s) and chunk verticies ----

    let mut arry : [Ustr; 6] = arr![ustr("testgrass.png"); 6];

    arry[posi_x_index!()] = ustr("test.png");

    let mut renderer : crate::client::renderer::Renderer = crate::client::renderer::Renderer::new();
    let air_art = TileArtSimple { textures : BlockTex::Invisible };
    let stone_art = TileArtSimple { textures : BlockTex::Single(ustr("teststone.png")) };
    let dirt_art = TileArtSimple { textures : BlockTex::Single(ustr("testdirt.png")) };
    let grass_art = TileArtSimple { textures : BlockTex::AllSides(arry) };

    renderer.texture_manager.associate_tile(&display, air, air_art);
    renderer.texture_manager.associate_tile(&display, stone, stone_art);
    renderer.texture_manager.associate_tile(&display, dirt, dirt_art);
    renderer.texture_manager.associate_tile(&display, grass, grass_art);

    renderer.texture_manager.rebuild(&display);

    for chunk in space.get_loaded_chunks() {
        info!(Mesher, "Forcing mesh of {}...", chunk);
        if chunk.y > 0 { 
            info!(Mesher, "- (This should be all air!)");
        }
        let start = Instant::now();
        renderer.force_mesh(&space, chunk, &display);
        let elapsed = start.elapsed();
        info!(Mesher, "- Meshing {} took {} microseconds", chunk, elapsed.as_micros());
    }

    //---- Some movement stuff ----

    let mut w_down : bool = false;
    let mut a_down : bool = false;
    let mut s_down : bool = false;
    let mut d_down : bool = false;

    let mut set_action : bool = false;
    let mut delete_action : bool = false;

    let screen_center_x : i32 = client_config.resolution.0 as i32 /2;
    let screen_center_y : i32 = client_config.resolution.1 as i32 /2;
    
    let mut mouse_first_moved : bool = false;
    let mut grabs_mouse : bool = true;
    //---- A mainloop ----
    let mut lastupdate = Instant::now();

    {
        display.gl_window().window().set_cursor_grab(grabs_mouse);
        display.gl_window().window().set_cursor_visible(!grabs_mouse);
    }

    event_loop.run(move |event, _, control_flow| {
        
        *control_flow = ControlFlow::Poll;

        match event {
            Event::LoopDestroyed => return,
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit
                },
                
                WindowEvent::CursorMoved{position, .. } => {
                    //println!("Mouse moved ({}, {})", position.x, position.y);
                    let x = position.x as f32;
                    let y = position.y as f32;
                    if grabs_mouse {
                        if !mouse_first_moved {
                            let elapsed = lastupdate.elapsed().as_secs_f32();
                            let amt_x = (x - screen_center_x as f32) as f32 / elapsed; //Pixels moved by cursor per second.
                            let amt_y = (y - screen_center_y as f32) as f32 / elapsed; //Pixels moved by cursor per second.
                            
                            horz_angle.0 -= amt_x * mouse_sensitivity * elapsed;
                            vert_angle.0 -= amt_y * mouse_sensitivity * elapsed;

                        }
                        else {
                            mouse_first_moved = true;
                        }
                        let gl_window = display.gl_window();
                        let window = gl_window.window();
                        //let window_position = window.inner_position().unwrap(); 
                        window.set_cursor_position(PhysicalPosition{x: screen_center_x, y: screen_center_y}).unwrap();
                    }
                },
                WindowEvent::MouseInput{ state, button, .. } => {
                    if state == ElementState::Released {
                        match button {
                            glutin::event::MouseButton::Left => delete_action = true,
                            glutin::event::MouseButton::Right => set_action = true,
                            _ => {},
                        }
                    }
                },
                WindowEvent::KeyboardInput{input, is_synthetic, ..} => {
                    if !is_synthetic {
                        match input.virtual_keycode {
                            Some(VirtualKeyCode::Escape) => {
                                *control_flow = ControlFlow::Exit;
                            },
                            Some(VirtualKeyCode::Tab) => { 
                                match input.state {
                                    glutin::event::ElementState::Released => {
                                        grabs_mouse = !grabs_mouse;
                                        display.gl_window().window().set_cursor_grab(grabs_mouse);
                                        display.gl_window().window().set_cursor_visible(!grabs_mouse);
                                    },
                                    _ => {},
                                }
                            },
                            Some(VirtualKeyCode::W) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => w_down = true,
                                    glutin::event::ElementState::Released => w_down = false,
                                }
                            },
                            Some(VirtualKeyCode::A) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => a_down = true,
                                    glutin::event::ElementState::Released => a_down = false,
                                }
                            },
                            Some(VirtualKeyCode::S) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => s_down = true,
                                    glutin::event::ElementState::Released => s_down = false,
                                }
                            },
                            Some(VirtualKeyCode::D) => {
                                match input.state {
                                    glutin::event::ElementState::Pressed => d_down = true,
                                    glutin::event::ElementState::Released => d_down = false,
                                }
                            },
                            _ => {}
                        }
                    }
                }
                _ => (),
            },
            _ => (),
        }
        horz_angle = horz_angle.normalize();
        vert_angle = vert_angle.normalize();
        
        let yaw : Quaternion<f32> = Quaternion::from_angle_y(horz_angle);
        let pitch : Quaternion<f32> = Quaternion::from_angle_x(vert_angle);
        let rotation = (yaw * pitch).normalize();

        let mut forward : Vector3<f32> = Vector3::new(0.0, 0.0, -1.0);
        let mut right : Vector3<f32> = Vector3::new(1.0, 0.0, 0.0);
        forward = rotation.rotate_vector(forward);
        right = rotation.rotate_vector(right);
        let up = forward.cross( right ).neg();

        //let click_point = camera_pos + forward.normalize();
        //let click_point_vx : VoxelPos<i32> = VoxelPos{x: click_point.x.round() as i32, y: click_point.y.round() as i32, z: click_point.z.round() as i32};
        
        if delete_action || set_action {
            let cam_pos : Point3<f64> = (camera_pos.x as f64, camera_pos.y as f64, camera_pos.z as f64).into();
            let forw_f64 : Vector3<f64> = (forward.x as f64, forward.y as f64, forward.z as f64).into();
            let mut raycast = VoxelRaycast::new(cam_pos, forw_f64);
            let mut tile: Option<TileId>  = None;
            let mut struck_pos: VoxelPos<i32> = raycast.pos;
            for _i in 0..1024{ 
                raycast.step();
                if !space.is_loaded(raycast.pos) {
                    break;
                }
                let found_tile = space.get(raycast.pos).map_err(|e| error!(Game, "{}", e)).unwrap();
                if found_tile != air {
                    tile = Some(found_tile);
                    struck_pos = raycast.pos;
                    break;
                }
            }
            if let Some(tile) = tile {
                if delete_action {
                    if tile != air {
                        space.set(struck_pos, air.clone());
                        renderer.notify_remesh(struck_pos);
                    }
                    delete_action = false;
                }
                else if set_action {
                    //Get the side our raycast hit.
                    let direction = raycast.get_last_direction();
                    let block_pos = struck_pos.get_neighbor(direction.opposite());
                    if space.get(block_pos).unwrap() != ustr("stone") {
                        space.set(block_pos, ustr("stone"));
                        renderer.notify_remesh(block_pos);
                    }
                    set_action = false;
                }
            }
        }
        
        //Movement
        if w_down {
            camera_pos += forward * (lastupdate.elapsed().as_secs_f32() * move_speed);
        }
        if d_down {
            camera_pos += right * (lastupdate.elapsed().as_secs_f32() * move_speed);
        }
        if s_down {
            camera_pos += (forward * (lastupdate.elapsed().as_secs_f32() * move_speed)).neg();
        }
        if a_down {
            camera_pos += (right * (lastupdate.elapsed().as_secs_f32() * move_speed)).neg();
        }

        //Drawing
        let view_matrix: Matrix4<f32> = Matrix4::look_at(camera_pos, camera_pos + forward, up);
        let perspective_matrix = Matrix4::from(perspective);
        
        let before_remesh = Instant::now();
        renderer.process_remesh(&space, &display).map_err(|e| error!(Mesher, "Error attempting to re-mesh: {}", e));
        let remesh_time = before_remesh.elapsed().as_micros();
        if remesh_time > 60 {
            info!(Mesher, "Took {} microseconds to remesh chunks.", remesh_time);
        }

        let mut target = display.draw();
        target.clear_color_and_depth((0.43, 0.7, 0.82, 1.0), 1.0);
        renderer.draw(perspective_matrix, view_matrix, &mut target, &program, &params).map_err(|e| error!(Renderer, "Error trying to draw: {}", e));
        target.finish().unwrap();
        lastupdate = Instant::now();
    });
}