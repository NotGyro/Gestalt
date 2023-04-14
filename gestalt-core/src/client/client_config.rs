use serde::{Serialize, Deserialize};
use winit::window::Fullscreen;

pub const WINDOW_TITLE: &str = "Gestalt";
pub const CLIENT_CONFIG_FILENAME: &str = "client_config.ron";

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WindowMode {
    Windowed {
        /// If windowed, can this be resized with the OS' drag-and-drop controls?
        resizable: bool,
        /// Maximized upon creation?
        maximized: bool,
    },
    BorderlessFullscreenWindow,
    ExclusiveFullscreen,
}
impl Default for WindowMode {
    fn default() -> Self {
        WindowMode::Windowed {
            resizable: true,
            maximized: false,
        }
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
impl From<DisplaySize> for winit::dpi::PhysicalSize<u32> { 
    fn from(size: DisplaySize) -> Self { 
        winit::dpi::PhysicalSize {
            width: size.width,
            height: size.height,
        }
    }
}
impl From<winit::dpi::PhysicalSize<u32>> for DisplaySize { 
    fn from(size: winit::dpi::PhysicalSize<u32>) -> Self { 
        Self {
            width: size.width,
            height: size.height,
        }
    }
}
impl From<DisplaySize> for winit::dpi::Size {
    fn from(size: DisplaySize) -> Self {
        winit::dpi::Size::Physical(size.into())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DisplayConfig {
    pub size: DisplaySize,
    pub window_mode: WindowMode,
    ///Corresponds to winit::MonitorHandle.name()
    pub monitor: Option<String>,
    /// Which graphics card?
    pub device: Option<String>,
}

impl DisplayConfig {
    pub fn to_window_builder(&self) -> winit::window::WindowBuilder {
        //TODO: Select device
        let builder = winit::window::WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(self.size);
        match self.window_mode {
            WindowMode::Windowed {
                resizable,
                maximized,
            } => builder
                .with_resizable(resizable)
                .with_maximized(maximized)
                .with_fullscreen(None),
            WindowMode::BorderlessFullscreenWindow => {
                builder.with_fullscreen(Some(Fullscreen::Borderless(None)))
            }
            WindowMode::ExclusiveFullscreen => {
                todo!()
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClientConfig {
    pub your_display_name: String, 
    pub display_properties: DisplayConfig,
    pub mouse_sensitivity_x: f32,
    pub mouse_sensitivity_y: f32,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            your_display_name: String::from("player"),
            display_properties: Default::default(),
            mouse_sensitivity_x: 64.0,
            mouse_sensitivity_y: 64.0,
        }
    }
}