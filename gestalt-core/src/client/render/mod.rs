use std::fs::OpenOptions;
use std::io::Read;
use std::iter;
use std::num::NonZeroU32;
use std::ops::Neg;
use std::path::{Path, PathBuf};
use glam::{Quat, Vec3, Mat4, EulerRot};
use image::{Rgba, RgbaImage};
use log::info;
use wgpu::util::DeviceExt;
use std::collections::HashMap;
use wgpu::{
	CreateSurfaceError, DeviceDescriptor,
	InstanceDescriptor, PushConstantRange, ShaderStages,
};
use winit::window::Window;

use crate::client::client_config::{ClientConfig, DisplaySize};
use crate::common::{Color, FastHashMap, new_fast_hash_map};
use crate::entity::{EcsWorld, EntityPos, EntityScale, EntityVelocity};
use crate::resource::image::{ID_PENDING_TEXTURE, ID_MISSING_TEXTURE, ImageProvider, InternalImage};
use crate::resource::{ResourceId, ResourcePoll};

use self::drawable::BillboardDrawable;
use self::terrain_renderer::{TerrainRendererError, TerrainRenderer};

use super::camera::Camera;

pub mod drawable;
pub mod array_texture;
pub mod tiletextureatlas;
pub mod voxel_mesher;
pub mod voxel_art;
pub mod terrain_renderer;

pub(in self) fn load_test_shader<P: AsRef<Path>>(path: P) -> wgpu::ShaderSource<'static> {
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

#[derive(thiserror::Error, Debug)]
pub enum DrawFrameError {
	#[error("Unable to draw a frame, could not acquire render surface: {0}")]
	CannotRequestDevice(#[from] wgpu::SurfaceError),
	#[error("Unable to draw a frame due to voxel rendering issue: {0}")]
	VoxelError(#[from] TerrainRendererError)
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

// Not indexed because there are only 2 overlaps so it's not really worth it 
// (reconsider if seams appear)
const UNIT_BILLBOARD: &[Vertex] = &[
	Vertex { position: [-0.5, -0.5, 0.0], tex_coords: [0.0, 1.0], },
	Vertex { position: [0.5, 0.5, 0.0], tex_coords: [1.0, 0.0], },
	Vertex { position: [-0.5, 0.5, 0.0], tex_coords: [0.0, 0.0], },
	
	Vertex { position: [-0.5, -0.5, 0.0], tex_coords: [0.0, 1.0], },
	Vertex { position: [0.5, -0.5, 0.0], tex_coords: [1.0, 1.0], },
	Vertex { position: [0.5, 0.5, 0.0], tex_coords: [1.0, 0.0], },
];

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Mat4 = Mat4::from_cols_array(&[
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.0,
    0.0, 0.0, 0.5, 1.0,
]);

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(in self) struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}
impl CameraUniform { 
	pub fn new() -> Self { 
		Self {
			view_proj: Mat4::IDENTITY.to_cols_array_2d()
		}
	}
	pub fn update(&mut self, matrix: Mat4) {
		self.view_proj = matrix.to_cols_array_2d();
	}
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(in self) struct ModelPush {
    matrix: [[f32; 4]; 4],
}
impl ModelPush {
	pub fn new(matrix: Mat4) -> Self { 
		Self {
			matrix: matrix.to_cols_array_2d()
		}
	}
}

struct TextureManager {
    id_to_texture: FastHashMap<ResourceId, ImageTextureBinding>, 
    loaded_textures: HashMap<u32, LoadedTexture, nohash::BuildNoHashHasher<u32>>,
	
    next_texture_handle: TextureHandle,
	
    missing_image: InternalImage,
    pending_image: InternalImage,
    error_image: InternalImage,
}

impl TextureManager {
	pub fn new() -> Self {
        // Set up engine textures.
        let missing_image = generate_missing_texture_image(32, 32);
        let pending_image = generate_pending_texture_image(32, 32);
        let error_image = generate_error_texture_image(32, 32);

		Self {
			next_texture_handle: unsafe {
				//Actually very safe, because one is not zero, but we'll humor the compiler here.
				TextureHandle::new_unchecked(1)
			},
			missing_image,
			pending_image,
			error_image,
            id_to_texture: new_fast_hash_map(), 
            loaded_textures: HashMap::with_hasher(nohash::BuildNoHashHasher::default()),
		}
		
	}
	pub fn load_image(image: &InternalImage,
		sampler_config: &wgpu::SamplerDescriptor,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		bind_group_layout: &wgpu::BindGroupLayout
	) -> LoadedTexture {
        let texture_size = wgpu::Extent3d {
            width: image.dimensions().0,
            height:  image.dimensions().1,
            depth_or_array_layers: 1
        };

        // Create the buffer on the GPU.
        let texture_buffer = device.create_texture(
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
        queue.write_texture(
            //Dest
            wgpu::ImageCopyTexture {
                texture: &texture_buffer,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            //Source
            &image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: std::num::NonZeroU32::new(4 * texture_size.width),
                rows_per_image: std::num::NonZeroU32::new(texture_size.height),
            },
            texture_size,
        );
        
        let texture_view = texture_buffer.create_view(&wgpu::TextureViewDescriptor::default());
        
        let sampler = device.create_sampler(sampler_config);

        let bind_group = device.create_bind_group(
            &wgpu::BindGroupDescriptor {
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    }
                ],
                label: Some("diffuse_bind_group"),
            }
        );
		LoadedTexture {
            buffer_handle: Box::new(texture_buffer),
            texture_view,
            bind_group,
        }
	}
    // This will likely change when the engine as a whole is more structured.
    // Probably it'll be some kind of message-passing situation. 
    pub fn ingest_image_resource<P>(&mut self,
		resource_id: &ResourceId,
		sampler_config: &wgpu::SamplerDescriptor,
		device: &wgpu::Device,
		queue: &wgpu::Queue,
		bind_group_layout: &wgpu::BindGroupLayout,
		loader: &mut P
	) -> TextureHandle
            where P: ImageProvider {

        let image = if resource_id == &ID_PENDING_TEXTURE {
            &self.pending_image
        } else if resource_id == &ID_MISSING_TEXTURE {
            &self.missing_image
        }
        else {
            match loader.load_image(resource_id) {
                ResourcePoll::Pending => &self.pending_image,
                ResourcePoll::Errored(e) => match e {
                    crate::resource::image::RetrieveImageError::DoesNotExist(_) => &self.missing_image,
                    _ => &self.error_image,
                },
                ResourcePoll::Ready(image) => image,
            }
        };
		
		let loaded_texture = Self::load_image(image, sampler_config, device, queue, bind_group_layout);
        let handle = self.next_texture_handle;
        self.next_texture_handle = self.next_texture_handle.checked_add(1)
            .expect("Ran out of texture handle IDs!");

        let previous_texture = self.loaded_textures.insert(handle.get(), loaded_texture);
        assert!(previous_texture.is_none());
        self.id_to_texture.insert(resource_id.clone(), handle);
        
        handle
    }
	pub fn get(&self, handle: TextureHandle) -> Option<&LoadedTexture> { 
		self.loaded_textures.get(&handle.get())
	}
	pub fn get_by_resource(&self, resource: &ResourceId) -> Option<&LoadedTexture> { 
		let id = self.id_to_texture.get(resource)?;
		self.get(*id)
	}
	pub fn get_id_by_resource(&self, resource: &ResourceId) -> Option<&TextureHandle> { 
		self.id_to_texture.get(resource)
	}
}

pub(self) struct LoadedTexture {
    pub buffer_handle: Box<wgpu::Texture>,
    pub texture_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
}

/// Describes where an Image ResourceID lives in the renderer. 
pub type ImageTextureBinding = TextureHandle;

/*
#[repr(C)]
#[derive(Debug, Clone)]
/// Describes where an Image ResourceID lives in the renderer. 
pub enum ImageTextureBinding {
	/// This image got its own buffer and bindgroup.
	OneToOne(TextureHandle),
	/// This image has been batched into one layer of an ArrayTexture.
	InBatch{
		array_texture: TextureHandle,
		cell: u32,
	},
	/// This image needed to be in both its own buffer and inside an ArrayTexture at the same time,
	/// and so it was uploaded to one first and then that texture data was copied to the second 
	/// buffer via the command queue (this should never be done on the CPU and then uploaded a
	/// second time!).
	Both{
		unary_texture: TextureHandle,
		array_texture: TextureHandle,
		cell: u32,
	},
}*/

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

    texture_bind_group_layout: wgpu::BindGroupLayout,
    
    camera_uniform: CameraUniform,
    camera_matrix_buffer: wgpu::Buffer,
    camera_matrix_bind_group: wgpu::BindGroup,

	depth_texture: (wgpu::Texture, wgpu::TextureView, wgpu::Sampler),

	texture_manager: TextureManager, 
	
    missing_texture: LoadedTexture,
    pending_texture: LoadedTexture,
    error_texture: LoadedTexture,

	pub terrain_renderer: TerrainRenderer,
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
			// This path is only possible to reach if the adapter was in the set,
			// it is okay to use unwrap here.
			Some(adapt_name) => adapters.remove(&adapt_name).unwrap(),
			None => instance
				.request_adapter(&wgpu::RequestAdapterOptions {
					power_preference: wgpu::PowerPreference::HighPerformance,
					compatible_surface: Some(&surface),
					force_fallback_adapter: false,
				})
				.await
				.ok_or(InitRenderError::CannotRequestAdapter)?,
		};

		let features = wgpu::Features::default()
			.union(wgpu::Features::PUSH_CONSTANTS);
		// wgpu::Features::TEXTURE_BINDING_ARRAY
		// wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
		let mut limits = wgpu::Limits::default(); 
		limits.max_push_constant_size = std::mem::size_of::<ModelPush>() as u32;
		let (mut device, mut queue) = adapter
			.request_device(
				&DeviceDescriptor {
					label: None,
					features,
					limits,
				},
				None,
			)
			.await?;
		info!("Max array layers: {} \n Max 3D texture size: {}", 
			device.limits().max_texture_array_layers,
			device.limits().max_texture_dimension_3d);
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

		// ^
		// | Alright, we're done initializing the screen, time for pipileines. |
		//                                                                     v

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
			label: Some("Billboard Shader"),
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

		// For testing
		let vertex_buffer = device.create_buffer_init(
			&wgpu::util::BufferInitDescriptor {
				label: Some("Vertex Buffer"),
				contents: bytemuck::cast_slice(UNIT_BILLBOARD),
				usage: wgpu::BufferUsages::VERTEX,
			}
		);

		let render_pipeline_layout =
			device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("Render Pipeline Layout"),
				bind_group_layouts: &[
					&texture_bind_group_layout,
					&camera_bind_group_layout,
				],
				push_constant_ranges: &[PushConstantRange{ 
					stages: ShaderStages::VERTEX,
					range: 0..(std::mem::size_of::<ModelPush>() as u32),
				}],
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
				cull_mode: Some(wgpu::Face::Back),
				polygon_mode: wgpu::PolygonMode::Fill,
				unclipped_depth: false,
				conservative: false,
			},
			depth_stencil: Some(wgpu::DepthStencilState {
				format: Self::DEPTH_FORMAT,
				depth_write_enabled: true,
				depth_compare: wgpu::CompareFunction::Less,
				stencil: wgpu::StencilState::default(),
				bias: wgpu::DepthBiasState::default(),
			}),
			multisample: wgpu::MultisampleState {
				count: 1,
				mask: !0,
				alpha_to_coverage_enabled: false,
			},
			multiview: None,
		});

		let depth_texture = Self::create_depth_texture(&device, &surface_config, "depth_texture");

		let texture_manager = TextureManager::new();

		let desc = wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::Repeat,
			address_mode_v: wgpu::AddressMode::Repeat,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		};

		// Generate our various types of error textures.
		let error_image = generate_error_texture_image(64, 64); 
		let error_texture = TextureManager::load_image(&error_image,
			&desc,
			&mut device,
			&mut queue,
			&texture_bind_group_layout);
		let missing_image = generate_missing_texture_image(64, 64); 
		let missing_texture = TextureManager::load_image(&missing_image,
			&desc,
			&mut device,
			&mut queue,
			&texture_bind_group_layout);
		let pending_image = generate_missing_texture_image(64, 64); 
		let pending_texture = TextureManager::load_image(&pending_image,
			&desc,
			&mut device,
			&mut queue,
			&texture_bind_group_layout);

		let terrain_renderer = TerrainRenderer::new(64,
			&camera_bind_group_layout, 
			&device,
			render_format, 
			&Self::DEPTH_FORMAT);
		
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
            texture_bind_group_layout,
			vertex_buffer, 

			camera_uniform,
			camera_matrix_buffer: camera_buffer,
			camera_matrix_bind_group: camera_bind_group,

			depth_texture,
			texture_manager,
			terrain_renderer,
			error_texture,
			missing_texture,
			pending_texture,
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
			self.depth_texture = Self::create_depth_texture(&self.device, &self.surface_config, "depth_texture");
		}
	}
	pub fn render_frame(&mut self, 
			camera: &Camera, 
			ecs_world: &EcsWorld, 
			clear_color: &Color,
			secs_since_last_tick: f32) -> Result<(), DrawFrameError> {
		let view_projection_matrix = camera.build_view_projection_matrix();
		let output = self.surface.get_current_texture()?;

		let camera_matrix = OPENGL_TO_WGPU_MATRIX * view_projection_matrix;
		self.camera_uniform.update(camera_matrix);
		
		self.queue.write_buffer(
			&self.camera_matrix_buffer,
			0,
			bytemuck::cast_slice(&[self.camera_uniform]),
		);

		let surface_texture_view = output
			.texture
			.create_view(&wgpu::TextureViewDescriptor::default());

		let mut encoder = self
			.device
			.create_command_encoder(&wgpu::CommandEncoderDescriptor {
				label: Some("Render Encoder"),
			});
		{
			// In the future everything inside this block will instead exist in 
			// separate render pass structs. 
			let (clear_r, clear_g, clear_b) = clear_color.to_normalized_float();
			let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
				label: Some("Render Pass"),
				color_attachments: &[
					Some(wgpu::RenderPassColorAttachment {
						view: &surface_texture_view,
						resolve_target: None,
						ops: wgpu::Operations {
							load: wgpu::LoadOp::Clear(wgpu::Color {
								r: clear_r as f64,
								g: clear_g as f64,
								b: clear_b as f64,
								a: 1.0,
							}),
							store: true,
						},
					}),
				],
				depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
					view: &self.depth_texture.1,
					depth_ops: Some(wgpu::Operations {
						load: wgpu::LoadOp::Clear(1.0),
						store: true,
					}),
					stencil_ops: None,
				}),
			});

			for (_entity, (
					position, 
					drawable,
					scale_maybe,
					velocity_maybe
				)
			) in ecs_world.query::<
					(&EntityPos, 
					&BillboardDrawable,
					Option<&EntityScale>,
					Option<&EntityVelocity>)
				>().iter() {
				let texture_maybe = match &drawable.texture_handle {
					Some(handle) => self.texture_manager.get(*handle),
					None => self.texture_manager.get_by_resource(&drawable.texture),
				};
				let texture = match texture_maybe { 
					Some(texture) => texture, 
					None => &self.missing_texture,
				};
				render_pass.set_pipeline(&self.render_pipeline);

				/*
				let model_matrix = match (rot_maybe, scale_maybe) {
					(Some(rot), Some(scale)) => {
						Mat4::from_scale_rotation_translation(
							scale.get().into(), 
							rot.get(), 
							position.get().into())
					}, 
					(Some(rot), None) => { 
						Mat4::from_rotation_translation(rot.get(), position.get().into())
					}
					(None, Some(scale)) => {
						Mat4::from_scale_rotation_translation(
							scale.get().into(), 
							Quat::IDENTITY, 
							position.get().into())
					},
					(None, None) => { 
						Mat4::from_translation(position.get().into())
					}
				};*/
				// Translate camera into this-object-space
				/*
						Quat::from_euler(EulerRot::YXZ, 
							(camera.get_yaw().get_radians() - std::f32::consts::PI)
								% std::f32::consts::PI,
							(camera.get_pitch().get_radians() - std::f32::consts::PI)
								% std::f32::consts::PI, 
							(camera.get_roll().get_radians() - std::f32::consts::PI)
								% std::f32::consts::PI) */
				// Guess where the entity *should* be independent of tick rate. 
				let interpolated_pos = match velocity_maybe {
					Some(vel) => {
						let motion_per_second = vel.get_motion_per_second();
						let movement_guess = motion_per_second * secs_since_last_tick; 
						position.get() + movement_guess
					},
					None => position.get(),
				};

				let negated_camera_forward = camera.get_front().neg().normalize();
				let initial_look_back = Quat::from_rotation_arc(Vec3::new(0.0,0.0,1.0), negated_camera_forward);
				let billboard_look_back = match drawable.style {
					drawable::BillboardStyle::Spherical => {
						let euler = initial_look_back.to_euler(EulerRot::YXZ);
						Quat::from_euler(EulerRot::YXZ, euler.0, euler.1, 0.0)
					},
					drawable::BillboardStyle::Cylindrical => {
						let yaw = initial_look_back.to_euler(EulerRot::YXZ).0;
						Quat::from_euler(EulerRot::YXZ, yaw, 0.0, 0.0)
					},
				}.normalize();
				let model_matrix = match scale_maybe {
					Some(scale) => {
						Mat4::from_scale_rotation_translation(
							scale.get().into(), 
							billboard_look_back, 
							interpolated_pos)
					}, 
					None => {
						Mat4::from_rotation_translation(billboard_look_back, interpolated_pos)
					}
				};

				render_pass.set_push_constants(ShaderStages::VERTEX, 
					0,
					&bytemuck::cast_slice(&[ModelPush::new(model_matrix)]));

				render_pass.set_bind_group(0, &texture.bind_group, &[]);
				render_pass.set_bind_group(1, &self.camera_matrix_bind_group, &[]);
				render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
				render_pass.draw(0..(UNIT_BILLBOARD.len() as u32), 0..1);
			}
		}
		self.terrain_renderer.draw(&surface_texture_view, 
			&self.depth_texture.1, 
			Vec3::ONE,
			Vec3::ZERO,
			Quat::IDENTITY, 
			&self.camera_matrix_bind_group, 
			&mut encoder)?;

		self.queue.submit(iter::once(encoder.finish()));
		output.present();

		Ok(())
	}

	pub fn process_terrain_mesh_uploads<P>(&mut self, image_loader: &mut P) 
			-> Result<(), TerrainRendererError> 
			where P: ImageProvider { 
		self.terrain_renderer.push_to_gpu(&mut self.device, &mut self.queue, image_loader)
	}

	pub fn get_aspect_ratio(&self) -> f32 { 
		self.aspect_ratio
	}
	
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    
    fn create_depth_texture(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, label: &str) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        let size = wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[Self::DEPTH_FORMAT],
        };
        let texture = device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::LessEqual),
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                ..Default::default()
            }
        );

        (texture, view, sampler)
    }
	pub fn ingest_image<P>(&mut self,
		resource_id: &ResourceId,
		texture_loader: &mut P)
			where P: ImageProvider {
				
		let diffuse_sampler = wgpu::SamplerDescriptor {
			address_mode_u: wgpu::AddressMode::Repeat,
			address_mode_v: wgpu::AddressMode::Repeat,
			address_mode_w: wgpu::AddressMode::ClampToEdge,
			mag_filter: wgpu::FilterMode::Nearest,
			min_filter: wgpu::FilterMode::Nearest,
			mipmap_filter: wgpu::FilterMode::Nearest,
			..Default::default()
		};
		self.texture_manager.ingest_image_resource(resource_id, &diffuse_sampler, &self.device, &self.queue, &self.texture_bind_group_layout, texture_loader);
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
