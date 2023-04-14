use std::fs::OpenOptions;
use std::io::Read;
use std::iter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::executor::block_on;
use glam::{Vec3, Quat};
use log::info;
use wgpu::{Instance, Surface, Adapter, AdapterInfo, DeviceDescriptor, Queue, Device, InstanceDescriptor, CreateSurfaceError};
use winit::window::Window;
use std::collections::{HashMap, HashSet};
use image::{Rgba, RgbaImage};

use crate::client::client_config::{ClientConfig, DisplaySize};
use crate::entity::EcsWorld;
use crate::resource::ResourceId;

pub mod tiletextureatlas;
pub mod drawable;
//pub mod voxelmesher;
//pub mod terrain_renderer;

fn load_test_shader<P: AsRef<Path>>(path: P) -> wgpu::ShaderSource<'static> { 
    let path = path.as_ref();
    let mut file = OpenOptions::new().read(true)
        .create(false)
        .open(path).expect("Could not open shader file.");
    let mut source = String::default(); 
    let _len_read = file.read_to_string(&mut source).expect("Could not read shader file to string");
    wgpu::ShaderSource::Wgsl(source.into())
}

#[derive(thiserror::Error, Debug)]
pub enum InitRenderError {
    #[error("Unable to instantiate a rendering device: {0:?}")]
    CannotRequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("Failed to request an adapter - no valid rendering device available!")]
    CannotRequestAdapter,
    #[error("Surface incompatible with adapter (As indicated by no preferred format).")]
    NoPreferredFormat,
    #[error("Failed to create render surface: {0:?}")]
    CannotCreateSurface(#[from] CreateSurfaceError),
}

#[derive(thiserror::Error, Debug)]
pub enum DrawFrameError {
    #[error("Unable to draw a frame, could not acquire render surface: {0:?}")]
    CannotRequestDevice(#[from] wgpu::SurfaceError),
}

struct StaticBillboardPass { 

}

pub struct Renderer {
    window_size: winit::dpi::PhysicalSize<u32>,
    instance: wgpu::Instance,
    surface: wgpu::Surface,
    surface_config: wgpu::SurfaceConfiguration,
    adapter: wgpu::Adapter,
    queue: wgpu::Queue,
    device: wgpu::Device,
    aspect_ratio: f32,
    render_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    pub async fn new(window: &Window, config: &ClientConfig) -> Result<Self, InitRenderError> { 
        // WGPU instance / drawing-surface.
        let instance = wgpu::Instance::new(InstanceDescriptor::default());
        let surface = unsafe { instance.create_surface(window)? };

        let mut adapters: HashMap<String, wgpu::Adapter> = instance
            .enumerate_adapters(wgpu::Backends::all())
            .map(|a| (a.get_info().name.clone(), a) )
            .collect();

        let mut adapter_select: Option<String> = None; 
        
        let mut info_string = "Available rendering adapters are:\n".to_string();
        // Iterate through our list of devices to print a list for debugging purposes. 
        for adapter_info in adapters.values().map(|a| a.get_info()) { 
            // Handy device listing.
            let adapter_string = format!(" * {:?}\n", adapter_info);
            info_string.push_str(&adapter_string);
            // See if this one matches the one we requested.
            if let Some(preferred_adapter) = config.display_properties.device.as_ref() { 
                if &adapter_info.name == preferred_adapter { 
                    adapter_select = Some(adapter_info.name.clone());
                    break;
                }
            }
        }
        // Print debug list.
        info!("{}", info_string);

        // Final decision on which device gets used. 
        let adapter = match adapter_select { 
            Some(adapt_name) => { 
                adapters.remove(&adapt_name).unwrap()
            }, 
            None => instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::default(),
                        compatible_surface: Some(&surface),
                        force_fallback_adapter: false,}).await.ok_or(InitRenderError::CannotRequestAdapter)?,
        };


        let (device, queue) = adapter.request_device(
            &DeviceDescriptor{ 
                label: None,
                features: wgpu::Features::default(),
                limits: wgpu::Limits::default(),
            }, None).await?;
        
        //Ensure WGPU knows how to use our surface.
        let surface_capabilities = surface.get_capabilities(&adapter);
        if surface_capabilities.formats.is_empty() { 
            return Err(InitRenderError::NoPreferredFormat);
        }
        info!("Render surface supports formats: {:?}", &surface_capabilities.formats);

        let render_format = surface_capabilities.formats.first().unwrap();

        let window_size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, // When we implement portals I am likely to touch this again. 
            format: render_format.clone(),
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![render_format.clone()],
        };
        surface.configure(&device, &surface_config);
        
        let aspect_ratio = (window_size.width as f32) / (window_size.height as f32); 

        // Load some simple shaders to figure out what I'm doing here with. 
        let shader_source = load_test_shader(PathBuf::from("test_shader.wgsl"));
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: shader_source,
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });
        
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main", // 1.
                buffers: &[], // 2.
            },
            fragment: Some(wgpu::FragmentState { // 3.
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState { // 4.
                    format: render_format.clone(),
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw, // 2.
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None, // 1.
            multisample: wgpu::MultisampleState {
                count: 1, // 2.
                mask: !0, // 3.
                alpha_to_coverage_enabled: false, // 4.
            },
            multiview: None, // 5.
        });

        Ok(Self {
            aspect_ratio,
            window_size,
            surface_config,
            instance,
            surface,
            adapter,
            queue,
            device, 
            render_pipeline
        })
    }
    /// Resize the display area
    pub fn resize(&mut self, new_size: DisplaySize) {
        let new_size: winit::dpi::PhysicalSize<u32> = new_size.into();
        if new_size.width > 0 && new_size.height > 0 {
            self.window_size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
            self.aspect_ratio = (new_size.width as f32) / (new_size.height as f32);
        }
    }
    pub fn render_frame(&mut self, ecs_world: &EcsWorld) -> Result<(), DrawFrameError> { 
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[
                    // This is what @location(0) in the fragment shader targets
                    Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(
                                wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }
                            ),
                            store: true,
                        }
                    })
                ],
                depth_stencil_attachment: None,
            });
        
            render_pass.set_pipeline(&self.render_pipeline); // 2.
            render_pass.draw(0..3, 0..1); // 3.
        }
        
        self.queue.submit(iter::once(encoder.finish()));
        output.present();
        
        Ok(())
    }
}

pub fn generate_engine_texture_image(
    width: u32,
    height: u32,
    color_foreground: &Rgba<u8>,
    color_background: &Rgba<u8>,
) -> RgbaImage {
    let mut img_base = RgbaImage::new(width, height);

    for x in 0..width {
        for y in 0..height {
            // The rare logical/boolean XOR.
            if (x >= width / 2) ^ (y >= height / 2) {
                img_base.put_pixel(x, y, *color_foreground);
            } else {
                img_base.put_pixel(x, y, *color_background);
            }
        }
    }
    img_base
}

pub fn generate_missing_texture_image(width: u32, height: u32) -> RgbaImage {
    let foreground = Rgba([255, 25, 225, 255]);
    let background = Rgba([0, 0, 0, 255]);

    generate_engine_texture_image(width, height, &foreground, &background)
}

pub fn generate_pending_texture_image(width: u32, height: u32) -> RgbaImage {
    let foreground = Rgba([40, 120, 255, 255]);
    let background = Rgba([30, 40, 80, 255]);

    generate_engine_texture_image(width, height, &foreground, &background)
}
