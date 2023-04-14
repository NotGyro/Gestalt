use std::fs::OpenOptions;
use std::io::Read;
use std::iter;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use futures::executor::block_on;
use glam::{Quat, Vec3, Mat4, Vec4};
use image::{Rgba, RgbaImage};
use log::info;
use wgpu::util::DeviceExt;
use std::collections::{HashMap, HashSet};
use wgpu::{
	Adapter, AdapterInfo, CreateSurfaceError, Device, DeviceDescriptor,
	InstanceDescriptor, Queue, Surface,
};
use winit::window::Window;

use crate::client::client_config::{ClientConfig, DisplaySize};
use crate::entity::{EcsWorld, EntityPos};
use crate::resource::image::{ID_PENDING_TEXTURE, ID_MISSING_TEXTURE, ImageProvider, InternalImage};
use crate::resource::{ResourceId, ResourceStatus};

use self::drawable::BillboardDrawable;

use super::camera::Camera;

pub mod drawable;
pub mod tiletextureatlas;
//pub mod voxelmesher;
//pub mod terrain_renderer;

fn load_test_shader<P: AsRef<Path>>(path: P) -> wgpu::ShaderSource<'static> {
	let path = path.as_ref();
	let mut file = OpenOptions::new()
		.read(true)
		.create(false)
		.open(path)
		.expect("Could not open shader file.");
	let mut source = String::default();
	let _len_read = file
		.read_to_string(&mut source)
		.expect("Could not read shader file to string");
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

/// Renderer-internal handle to a currently-loaded texture.
pub(in crate::client::render) type TextureHandle = NonZeroU32;

// Renderer-internal handle to a currently-loaded texture.
/*
pub(in crate::client::render) struct LoadedTextureRef {
    pub base_texture: TextureHandle, 
    /// Which cell in an array-texture or a texture atlas is this referring to?
    pub cell: u16,
}*/

struct LoadedTexture { 
    pub buffer_handle: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

#[derive(thiserror::Error, Debug)]
pub enum DrawFrameError {
	#[error("Unable to draw a frame, could not acquire render surface: {0:?}")]
	CannotRequestDevice(#[from] wgpu::SurfaceError),
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    tex_coords: [f32; 2],
}
impl Vertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
            ],
        }
    }
}

const VERTICES: &[Vertex] = &[
	Vertex { position: [-0.0868241, 0.49240386, 0.0], tex_coords: [0.4131759, 0.00759614], }, // A
    Vertex { position: [-0.49513406, 0.06958647, 0.0], tex_coords: [0.0048659444, 0.43041354], }, // B
    Vertex { position: [0.44147372, 0.2347359, 0.0], tex_coords: [0.9414737, 0.2652641], }, // E
	
    Vertex { position: [-0.49513406, 0.06958647, 0.0], tex_coords: [0.0048659444, 0.43041354], }, // B
    Vertex { position: [-0.21918549, -0.44939706, 0.0], tex_coords: [0.28081453, 0.949397], }, // C
    Vertex { position: [0.44147372, 0.2347359, 0.0], tex_coords: [0.9414737, 0.2652641], }, // E
	
    Vertex { position: [-0.21918549, -0.44939706, 0.0], tex_coords: [0.28081453, 0.949397], }, // C
    Vertex { position: [0.35966998, -0.3473291, 0.0], tex_coords: [0.85967, 0.84732914], }, // D
    Vertex { position: [0.44147372, 0.2347359, 0.0], tex_coords: [0.9414737, 0.2652641], }, // E
];
		
#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Mat4 = glam::mat4(
    Vec4::new(1.0, 0.0, 0.0, 0.0),
    Vec4::new(0.0, 1.0, 0.0, 0.0),
    Vec4::new(0.0, 0.0, 0.5, 0.0),
    Vec4::new(0.0, 0.0, 0.5, 1.0),
); 

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    mvp: [[f32; 4]; 4],
}
impl CameraUniform { 
	pub fn new() -> Self { 
		Self {
			mvp: Mat4::IDENTITY.to_cols_array_2d()
		}
	}
	pub fn update(&mut self, matrix: Mat4) {
		self.mvp = matrix.to_cols_array_2d();
	}
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
    vertex_buffer: wgpu::Buffer,

    next_texture_handle: TextureHandle,
    id_to_texture: HashMap<ResourceId, TextureHandle>, 
    loaded_textures: HashMap<TextureHandle, LoadedTexture>,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    
    missing_texture: InternalImage,
    pending_texture: InternalImage,
    error_texture: InternalImage,

    mvp_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
}

impl Renderer {
	pub async fn new(window: &Window, camera: &Camera, config: &ClientConfig) -> Result<Self, InitRenderError> {
		// WGPU instance / drawing-surface.
		let instance = wgpu::Instance::new(InstanceDescriptor::default());
		let surface = unsafe { instance.create_surface(window)? };

		let mut adapters: HashMap<String, wgpu::Adapter> = instance
			.enumerate_adapters(wgpu::Backends::all())
			.map(|a| (a.get_info().name.clone(), a))
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
			Some(adapt_name) => adapters.remove(&adapt_name).unwrap(),
			None => instance
				.request_adapter(&wgpu::RequestAdapterOptions {
					power_preference: wgpu::PowerPreference::default(),
					compatible_surface: Some(&surface),
					force_fallback_adapter: false,
				})
				.await
				.ok_or(InitRenderError::CannotRequestAdapter)?,
		};

		let (device, queue) = adapter
			.request_device(
				&DeviceDescriptor {
					label: None,
					features: wgpu::Features::default(),
					limits: wgpu::Limits::default(),
				},
				None,
			)
			.await?;

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

        let texture_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            }
        );

		// Load some simple shaders to figure out what I'm doing here with.
		let shader_source = load_test_shader(PathBuf::from("test_shader.wgsl"));
		let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("Shader"),
			source: shader_source,
		});

		// Set up the uniform for our camera. 
		let mut camera_uniform = CameraUniform::new();
		camera_uniform.update(OPENGL_TO_WGPU_MATRIX * camera.build_view_projection_matrix());

		let camera_buffer = device.create_buffer_init(
			&wgpu::util::BufferInitDescriptor {
				label: Some("Camera Buffer"),
				contents: bytemuck::cast_slice(&[camera_uniform]),
				usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
			}
		);
		let camera_bind_group_layout = device.create_bind_group_layout(
			&wgpu::BindGroupLayoutDescriptor {
				entries: &[
					wgpu::BindGroupLayoutEntry {
						binding: 0, 
						visibility: wgpu::ShaderStages::VERTEX,
						ty: wgpu::BindingType::Buffer { 
							ty: wgpu::BufferBindingType::Uniform, 
							has_dynamic_offset: false, 
							min_binding_size: None 
						},
						count: None,
					}
				],
				label: Some("camera_bind_layout"),
			}
		);

		let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor { 
			layout: &camera_bind_group_layout,
			entries: &[
				wgpu::BindGroupEntry { 
					binding: 0,
					resource: camera_buffer.as_entire_binding(),
				}
			],
			label: Some("camera_bind_group")
		});

		let render_pipeline_layout =
			device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("Render Pipeline Layout"),
				bind_group_layouts: &[
					&texture_bind_group_layout,
					&camera_bind_group_layout,
				],
				push_constant_ranges: &[],
			});

		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("Render Pipeline"),
			layout: Some(&render_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &shader,
				entry_point: "vs_main",
				buffers: &[
					Vertex::desc(),
				],
			},
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: render_format.clone(),
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
			primitive: wgpu::PrimitiveState {
				topology: wgpu::PrimitiveTopology::TriangleList,
				strip_index_format: None,
				front_face: wgpu::FrontFace::Ccw,
				cull_mode: None, //Some(wgpu::Face::Back)
				polygon_mode: wgpu::PolygonMode::Fill,
				unclipped_depth: false,
				conservative: false,
			},
			depth_stencil: None,
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			multiview: None, // 5.
		});

        // Set up engine textures.
        let missing_texture = generate_missing_texture_image(32, 32);
        let pending_texture = generate_pending_texture_image(32, 32);
        let error_texture = generate_error_texture_image(32, 32);

		// For testing 
		
		let vertex_buffer = device.create_buffer_init(
			&wgpu::util::BufferInitDescriptor {
				label: Some("Vertex Buffer"),
				contents: bytemuck::cast_slice(VERTICES),
				usage: wgpu::BufferUsages::VERTEX,
			}
		);

		Ok(Self {
			aspect_ratio,
			window_size,
			surface_config,
			instance,
			surface,
			adapter,
			queue,
			device,
			render_pipeline,
            next_texture_handle: unsafe {
                //Actually very safe, because one is not zero, but we'll humor the compiler here.
                TextureHandle::new_unchecked(1)
            },
            id_to_texture: HashMap::default(), 
            loaded_textures: HashMap::default(),
            texture_bind_group_layout,
			vertex_buffer, 

			mvp_uniform: camera_uniform,
			camera_buffer,
			camera_bind_group,

            pending_texture,
            missing_texture,
            error_texture,
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
	pub fn render_frame(&mut self, camera: &Camera, ecs_world: &EcsWorld) -> Result<(), DrawFrameError> {
		let view_projection_matrix = camera.build_view_projection_matrix();
		let output = self.surface.get_current_texture()?;

		let view = output
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());

		for (_entity, (position, drawable)) in ecs_world.query::<(&EntityPos, &BillboardDrawable)>().iter() {
			let mut encoder = self
				.device
				.create_command_encoder(&wgpu::CommandEncoderDescriptor {
					label: Some("Render Encoder"),
				});
			{
				let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
					label: Some("Render Pass"),
					color_attachments: &[
						Some(wgpu::RenderPassColorAttachment {
							view: &view,
							resolve_target: None,
							ops: wgpu::Operations {
								load: wgpu::LoadOp::Clear(wgpu::Color {
									r: 0.35,
									g: 0.4,
									b: 0.8,
									a: 1.0,
								}),
								store: true,
							},
						}),
					],
					depth_stencil_attachment: None,
				});

				let model_matrix = Mat4::from_translation(position.get().into());
				let mvp_matrix = OPENGL_TO_WGPU_MATRIX * view_projection_matrix * model_matrix;
				self.mvp_uniform.update(mvp_matrix);
				self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.mvp_uniform]));

				let texture_maybe = match &drawable.texture_handle {
					Some(handle) => self.loaded_textures.get(handle),
					None => {
						match self.id_to_texture.get(&drawable.texture) {
							Some(handle) => {
								self.loaded_textures.get(handle)
							}
							None => { 
								None
							}
						}
					},
				};
				let texture = texture_maybe.unwrap();
				render_pass.set_pipeline(&self.render_pipeline);
				render_pass.set_bind_group(0, &texture.bind_group, &[]);
				render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
				render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
				render_pass.draw(0..(VERTICES.len() as u32), 0..1);
			}

			self.queue.submit(iter::once(encoder.finish()));
		}
		output.present();

		Ok(())
	}
    
    // This will likely change when the engine as a whole is more structured.
    // Probably it'll be some kind of message-passing situation. 
    pub fn ingest_image<P>(&mut self, 
		resource_id: &ResourceId, 
		loader: &mut P
	) -> TextureHandle
            where P: ImageProvider {

        let diffuse_image = if resource_id == &ID_PENDING_TEXTURE {
            &self.pending_texture
        } else if resource_id == &ID_MISSING_TEXTURE {
            &self.missing_texture
        }
        else { 
            match loader.load_image(resource_id) {
                ResourceStatus::Pending => &self.pending_texture,
                ResourceStatus::Errored(e) => match e {
                    crate::resource::image::RetrieveImageError::DoesNotExist(_) => &self.missing_texture,
                    _ => &self.error_texture,
                },
                ResourceStatus::Ready(image) => image,
            }
        };
        
        let texture_size = wgpu::Extent3d { 
            width: diffuse_image.dimensions().0,
            height:  diffuse_image.dimensions().1, 
            depth_or_array_layers: 1
        };
        // Create the buffer on the GPU.
        let diffuse_texture_buffer = self.device.create_texture(
            &wgpu::TextureDescriptor {
                size: texture_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                label: Some("diffuse_texture"),
                view_formats: &[],
            }
        );
        // Upload the image to the buffer
        self.queue.write_texture(
            //Dest
            wgpu::ImageCopyTexture {
                texture: &diffuse_texture_buffer,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            //Source
            &diffuse_image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * texture_size.width),
                rows_per_image: std::num::NonZeroU32::new(texture_size.height),
            },
            texture_size,
        );
        
        let diffuse_texture_view = diffuse_texture_buffer.create_view(&wgpu::TextureViewDescriptor::default());
        
        // Set up for texture sampling
        let diffuse_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let diffuse_bind_group = self.device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &self.texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                    }
                ],
                label: Some("diffuse_bind_group"),
            }
        );

        let handle = self.next_texture_handle; 
        self.next_texture_handle = self.next_texture_handle.checked_add(1)
            .expect("Ran out of texture handle IDs!");
        let finished_texture = LoadedTexture {
            buffer_handle: diffuse_texture_buffer,
            texture_view: diffuse_texture_view,
            bind_group: diffuse_bind_group,
        };

        let previous_texture = self.loaded_textures.insert(handle, finished_texture);
        assert!(previous_texture.is_none());
        self.id_to_texture.insert(resource_id.clone(), handle);
        
        handle
    }

	pub fn get_aspect_ratio(&self) -> f32 { 
		self.aspect_ratio
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

pub fn generate_error_texture_image(width: u32, height: u32) -> RgbaImage {
	let foreground = Rgba([255, 0, 0, 255]);
	let background = Rgba([0, 0, 0, 255]);

	generate_engine_texture_image(width, height, &foreground, &background)
}
