use std::{io::{BufReader, Read, BufWriter, Write}, time::Instant};

use glam::Vec2;
use log::{warn, error};
use serde::{Serialize, Deserialize};
use winit::{event_loop::EventLoop, window::{Window, Fullscreen}, event::{ElementState, DeviceEvent}, dpi::PhysicalPosition};

use super::camera;

pub const WINDOW_TITLE: &'static str = "Gestalt";
pub const CLIENT_CONFIG_FILENAME: &'static str = "client_config.ron";

// Core / main part of the game client. Windowing and event dispatching lives here. 
// Input events come in through here. 
// Very important that input does not live on the same thread as any heavy compute tasks!
// We need to still be able to read input when the weird stuff is happening. 

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WindowMode { 
    Windowed{
        /// If windowed, can this be resized with the OS' drag-and-drop controls?
        resizable: bool,
        /// Maximized upon creation? 
        maximized: bool,
    },
    BorderlessFullscreen,
    ExclusiveFullscreen,
}
impl Default for WindowMode {
    fn default() -> Self {
        WindowMode::Windowed{resizable: true, maximized: false}
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub struct DisplaySize { 
    pub width: u32,
    pub height: u32,
}
impl Default for DisplaySize {
    fn default() -> Self {
        DisplaySize { 
            width: 1024,
            height: 768,
        }
    }
}
impl From<DisplaySize> for winit::dpi::Size {
    fn from(size: DisplaySize) -> Self {
        winit::dpi::Size::Physical(
            winit::dpi::PhysicalSize { 
                width: size.width, 
                height: size.height,
            }
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DisplayConfig { 
    pub size: DisplaySize,
    pub window_mode: WindowMode,
    ///Corresponds to winit::MonitorHandle.name()
    pub device: Option<String>,
}

impl DisplayConfig { 
    pub fn to_window_builder(&self) -> winit::window::WindowBuilder { 
        //TODO: Select device
        let builder = winit::window::WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(self.size);
        match self.window_mode {
            WindowMode::Windowed { resizable, maximized } => {
                builder.with_resizable(resizable)
                    .with_maximized(maximized)
                    .with_fullscreen(None)
            },
            WindowMode::BorderlessFullscreen => {
                builder.with_fullscreen(
                    Some( Fullscreen::Borderless(None) )
                )
            },
            WindowMode::ExclusiveFullscreen => {
                todo!()
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    pub display_properties: DisplayConfig,
    pub mouse_sensitivity_x: f64,
    pub mouse_sensitivity_y: f64,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self { display_properties: Default::default(), mouse_sensitivity_x: 1.0, mouse_sensitivity_y: 1.0 }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum StartClientError {
    #[error("could not read client config file, i/o error: {0:?}")]
    CouldntOpenConfig(#[from] std::io::Error),
    #[error("Attempted to create channel {0}, which exists already.")]
    CouldntParseConfig(#[from] ron::Error),
    #[error("Could not initialize display: {0:?}")]
    CreateWindowError(#[from] winit::error::OsError)
}

pub fn run_client() {
    let mut event_loop = winit::event_loop::EventLoop::new();
    // Open config
    let mut open_options = std::fs::OpenOptions::new();
    open_options
        .read(true)
        .append(true)
        .create(true);

    let config_maybe: Result<ClientConfig, StartClientError> = open_options.open(CLIENT_CONFIG_FILENAME)
        .map_err( |e| StartClientError::from(e) )
        .and_then(|file| {
            let mut buf_reader = BufReader::new(file);
            let mut contents = String::new();
            buf_reader.read_to_string(&mut contents)
                .map_err(|e| StartClientError::from(e))?; 
            Ok(contents)
        })
        .and_then(|e| { 
            Ok(ron::from_str(e.as_str())
                .map_err(|e| StartClientError::from(e))?)
        });
    //If that didn't load, just use built-in defaults. 
    let config: ClientConfig = match config_maybe {
        Ok(c) => c,
        Err(e) => {
            warn!("Couldn't open client config, using defaults. Error was: {:?}", e); 
            ClientConfig::default()
        },
    };

    // Set up window and event loop. 
    let window_builder = config.display_properties.to_window_builder();
    let window = window_builder
        .build(&event_loop).unwrap();

    let view_location = glam::Vec3::new(3.0, 3.0, -5.0);
    let mut camera = camera::Camera::new(view_location);

    camera.sensitivity = 1.0;
    camera.speed = 0.5;

    let first_frame_time = Instant::now();
    let mut prev_frame_time = Instant::now();

    let mut previous_mouse_position: Option<PhysicalPosition<f64>> = None;

    event_loop.run(move |event, _, control| {
        let elapsed_time = prev_frame_time.elapsed();
        prev_frame_time = Instant::now();
        let elapsed_secs = elapsed_time.as_secs_f64();
        
        match event {
            winit::event::Event::DeviceEvent {
                event: DeviceEvent::MouseMotion { 
                    delta,
                    ..
                },
                ..
            } => {
                let (dx, dy) = delta;
                let adjusted_dx = dx * elapsed_secs * config.mouse_sensitivity_x;
                let adjusted_dy = dy * elapsed_secs * config.mouse_sensitivity_y;
                /*match previous_mouse_position {
                    // First mouse position, init
                    None => {},
                    // Use as a legitimate mouse position change
                    Some(old_position) => { 
                        let dx = (position.x - old_position.x) * elapsed_secs * config.mouse_sensitivity_x;
                        let dy = (position.y - old_position.y) * elapsed_secs * config.mouse_sensitivity_y;

                    }
                }
                // Record our current mouse position as the new one. 
                previous_mouse_position = Some(position)*/

            },
            winit::event::Event::WindowEvent{
                event: winit::event::WindowEvent::KeyboardInput{
                    input,
                    is_synthetic: false,
                    ..
                },
                ..
            } => { 
                if input.state == ElementState::Pressed {

                }
                else if input.state == ElementState::Released {

                }
            },
            // Close button was clicked, we should close.
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::CloseRequested,
                ..
            } => {
                *control = winit::event_loop::ControlFlow::Exit;
            },
            // Window was resized, need to resize renderer.
            winit::event::Event::WindowEvent {
                event: winit::event::WindowEvent::Resized(size),
                ..
            } => {

            },
            // Render!
            winit::event::Event::MainEventsCleared => {
                // Present the frame on screen
            },
            winit::event::Event::LoopDestroyed => {
                // Cleanup on quit. 
                let mut cfg_string = ron::ser::to_string_pretty(&config, ron::ser::PrettyConfig::default() ).unwrap();
                let mut open_options = std::fs::OpenOptions::new();
                open_options
                    .write(true)
                    .truncate(true)
                    .create(true);
                
                match open_options.open(CLIENT_CONFIG_FILENAME) { 
                    Ok(mut file) => { 
                        file.write_all(cfg_string.as_bytes()).unwrap();
                        file.flush().unwrap();
                    },
                    Err(err) => {
                        error!("Could not write config file at exit! Reason is {:?}. Your configs were {}", err, cfg_string)
                    }
                }
            },
            // Other events we don't care about
            _ => {}
        }
    });
}