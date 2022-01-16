use std::io::{BufReader, Read};

use log::warn;
use serde::{Serialize, Deserialize};
use winit::{event_loop::EventLoop, window::{Window, Fullscreen}};

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

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ClientConfig {
    pub display_properties: DisplayConfig,
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

pub struct GameClient {
    pub window: Window,
    pub has_focus: bool,
    pub config: ClientConfig,
}

impl GameClient {
    pub fn init(event_loop: &EventLoop<()>) -> Result<Self, StartClientError> { 
        // Open config
        let mut open_options = std::fs::OpenOptions::new();
        open_options
            .read(true)
            .append(true)
            .create(true);

        let config_maybe: Result<ClientConfig, StartClientError> = open_options.open("config.ron")
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
            .build(event_loop)?;

        Ok(GameClient {
            window,
            has_focus: false,
            config,
        })
    }
}

/// Takes ownership of the thread its in and does not return until the program
/// as a whole has stopped running.
pub fn run_client(mut client: GameClient, event_loop: &EventLoop<()>) {
    //We will now track focus. Enter a known-good state.
    client.window.focus_window();
    client.has_focus = true;
}