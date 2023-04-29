use std::collections::{HashSet, HashMap};
use std::path::PathBuf;

use glam::{Vec3, Quat, Mat4};
use wgpu::util::DeviceExt;
use wgpu::{PushConstantRange, ShaderStages, TextureView};

use super::array_texture::{ArrayTextureLayout, ArrayTexture, ArrayTextureError};
use super::{load_test_shader, ModelPush};
use super::voxel_art::VoxelArtMapper;
use super::voxel_mesher::{ChunkMesh, MesherState, PackedVertex};
use crate::resource::image::ImageProvider;
use crate::world::tilespace::{TileSpace, TileSpaceError, world_to_chunk_pos, chunk_to_world_pos};
//use crate::world::chunk::CHUNK_SIZE;
//use crate::world::tilespace::{world_to_chunk_pos, TileSpaceError, TileSpace};
use crate::world::{ChunkPos, TilePos, TileId};
use crate::world::voxelstorage::VoxelSpace;

#[derive(thiserror::Error, Debug)]
pub enum TerrainRendererError {
    #[error("Error borrowing chunk for terrain renderer: {0:?}")]
    ChunkError(#[from] TileSpaceError),
    #[error("Could not prepare meshing for chunk {0:?}, received error: {1:?}")]
    PrepareMeshingError(ChunkPos, String),
    #[error("Could not mesh chunk {0:?}, received error: {1:?}")]
    MeshingError(ChunkPos, String),
    #[error("Could not build tile array texture: {0}")]
    ArrayTextureError(#[from] ArrayTextureError),
    #[error("Invalid array texture layout ID encountered.")]
    NoTexLayoutForId,
    #[error("Could not find array texture bound to a chunk mesh.")]
    NoTexForChunk,
    #[error("Could not find array texture bound to an array texture ID.")]
    NoTexForId
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Which type of voxel meshing is required to render this voxel cell? 
pub enum VoxelMesherPass {
    /// Any simple bloxel cube which is 1 meter in every direction, 
    /// and for which the tile ID can be mapped naively to a set of 
    /// textures for its six sides (without any extra state required)
    SimpleCubes,
}

#[derive(Copy, Clone, Hash, Debug)]
struct ChunkTextureBinding { 
    texture_id: u32,
    tile_array_texture_revision: u64,
}

struct BuiltChunk { 
    pub buffer: wgpu::Buffer,
    pub num_verts: u32,
}

/// A subsystem that is responsible for building, maintaining 
/// (i.e. owning memory / collecting garbage), and drawing 
/// meshes of world voxel terrain.
pub struct TerrainRenderer {
    /// Later this will be used to track tile positions rather than chunk positions, 
    /// so that partial rebuilds of a chunk are possible (Rather than total rebuilds every time)
    pending_remesh: HashSet<ChunkPos>,
    meshed_chunks: HashMap<ChunkPos, ChunkMesh>, 
    built_chunks: HashMap<ChunkPos, BuiltChunk>,
    texture_for_chunk: HashMap<ChunkPos, ChunkTextureBinding>,
    texture_layouts: HashMap<u32, ArrayTextureLayout>,
    texture_bind_group_layout: wgpu::BindGroupLayout,
    built_textures: HashMap<u32, ArrayTexture>,
    /// One past the highest texture ID in texture_layouts. Incremented each time we add a new texture layout.
    next_texture_id: u32,
    texture_size: u32,
    
	render_pipeline: wgpu::RenderPipeline,
}

impl TerrainRenderer {
    pub fn new(texture_size: u32,
            camera_layout: &wgpu::BindGroupLayout, 
            device: &wgpu::Device,
            render_format: &wgpu::TextureFormat,
            depth_format: &wgpu::TextureFormat )
                -> Self {
        let texture_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
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

		let voxel_shader_source = load_test_shader(PathBuf::from("voxel_shader_packed.wgsl"));
		let voxel_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
			label: Some("Voxel Shader"),
			source: voxel_shader_source,
		});

		let render_pipeline_layout =
			device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
				label: Some("Render Pipeline Layout"),
				bind_group_layouts: &[
					&texture_bind_group_layout,
					camera_layout,
				],
				push_constant_ranges: &[PushConstantRange{ 
					stages: ShaderStages::VERTEX,
					range: 0..(std::mem::size_of::<ModelPush>() as u32),
				}],
			});

        
		let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
			label: Some("Voxel Render Pipeline"),
			layout: Some(&render_pipeline_layout),
			vertex: wgpu::VertexState {
				module: &voxel_shader,
				entry_point: "vs_main",
				buffers: &[
					PackedVertex::desc(),
				],
			},
            fragment: Some(wgpu::FragmentState {
                module: &voxel_shader,
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
				format: *depth_format,
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

        TerrainRenderer {
            pending_remesh: HashSet::default(),
            meshed_chunks: HashMap::default(),
            built_chunks: HashMap::default(),
            texture_for_chunk: HashMap::default(),
            texture_layouts: HashMap::default(),
            texture_bind_group_layout,
            built_textures: HashMap::default(),
            next_texture_id: 0,
            texture_size,
            render_pipeline,
        }
    }
    /// Inform this terrain renderer that a block at the given position has changed.
    pub fn notify_changed(&mut self, tile_position: &TilePos) { 
        let chunk_position = world_to_chunk_pos(tile_position);
        self.pending_remesh.insert(chunk_position);
    }
    /// Inform this terrain renderer that a specific chunk needs to be remeshed.
    pub fn notify_chunk_remesh_needed(&mut self, chunk_position: &ChunkPos) {
        self.pending_remesh.insert(*chunk_position);
    }
    /// Inform this terrain renderer that the chunk mesh at the given position should
    /// not be kept in memory.
    pub fn notify_unloaded(&mut self, chunk_position: &ChunkPos) {
        if self.pending_remesh.contains(chunk_position) {
            self.pending_remesh.remove(chunk_position);
        }
        if self.meshed_chunks.contains_key(chunk_position) {
            self.meshed_chunks.remove(chunk_position);
        }
        if self.built_chunks.contains_key(chunk_position) {
            self.built_chunks.remove(chunk_position);
        }
        if self.texture_for_chunk.contains_key(chunk_position) {
            self.texture_for_chunk.remove(chunk_position);
        }
    }
    fn make_new_array_texture(&mut self) -> ChunkTextureBinding { 
        let new_texture_id = self.next_texture_id;
        self.next_texture_id += 1;
        let new_array_texture = ArrayTextureLayout::new((self.texture_size, self.texture_size), Some(4096));
        self.texture_layouts.insert(new_texture_id, new_array_texture);
        ChunkTextureBinding{ 
            texture_id: new_texture_id,
            tile_array_texture_revision: 0,
        }
    }
    fn find_available_texture_array_texture(&mut self) -> ChunkTextureBinding {
        if self.texture_layouts.is_empty() {
            self.make_new_array_texture()
        }
        else {
            for (atlas_id, layout) in self.texture_layouts.iter() { 
                //Is it relatively safe to assume we won't overrun max layout size?
                if layout.get_texture_count() <= (layout.get_max_textures()/2) as usize { 
                    return ChunkTextureBinding { 
                        texture_id: *atlas_id, 
                        tile_array_texture_revision: layout.get_revision(),
                    };
                }
            }
            self.make_new_array_texture()
        }
    }

    // Rebuild any meshes which have been flagged as changed.
    // Does not automatically push any mesh data to the GPU. Please use push_to_gpu() to update the meshes for rendering after calling this.
    // Returns whether or not any remesh is actually required.
    pub fn process_remesh<A: VoxelArtMapper<TileId>>(&mut self, voxel_space: &TileSpace, tiles_to_art: &A) -> Result<bool, TerrainRendererError> {
        if self.pending_remesh.is_empty() { 
            Ok(false)
        }
        else { 
            let mut did_mesh = false;
            let remesh_list: HashSet<ChunkPos> = self.pending_remesh.drain().collect();
            for chunk_position in remesh_list.iter() { 
                //let is_new_chunk = !self.gpu_chunks.contains_key(&chunk_position);
                // Do we need to make a new texture atlas for this chunk? 
                let texture_binding = if let Some(previous_texture_id) = self.texture_for_chunk.get(chunk_position) { 
                    *previous_texture_id
                } else {
                    self.find_available_texture_array_texture()
                };
    
                let chunk = voxel_space.borrow_chunk(chunk_position)?;
    
                //TODO: Handle case where texture array goes over max
                let mesher_state = MesherState::prepare_to_mesh(chunk, 
                    tiles_to_art, 
                    self.texture_layouts
                        .get_mut(&texture_binding.texture_id)
                        .ok_or(TerrainRendererError::NoTexLayoutForId)?
                ).map_err(|e| { 
                    TerrainRendererError::PrepareMeshingError(*chunk_position, format!("{:?}",e))
                })?;

                //Make sure not to waste bookkeeping pushing all-air chunks through the pipeline. 
                if mesher_state.needs_draw() { 
                    let mesh = mesher_state.build_mesh()
                        .map_err(|e| {
                            TerrainRendererError::MeshingError(*chunk_position, format!("{:?}",e))
                        })?;
                        
                    if !mesh.verticies.is_empty() {
                        did_mesh = true;
                        self.texture_for_chunk.insert(*chunk_position, texture_binding);
                        self.meshed_chunks.insert(*chunk_position, mesh);
                    }
                }
            }

            Ok(did_mesh)
        }
    }

    /// Takes any of the changed or new chunk meshes made in process_remesh() and makes them available for rendering. 
    pub fn push_to_gpu<TextureSource: ImageProvider>(&mut self,
            device: &mut wgpu::Device,
            queue: &mut wgpu::Queue,
            texture_source: &mut TextureSource) 
                -> Result<(), TerrainRendererError> {
        // First, handle textures.
        let mut textures_to_build: HashSet<u32> = HashSet::new();
        for (_, binding) in self.texture_for_chunk.iter() {
            let revision = self.texture_layouts.get_mut(&binding.texture_id)
                .ok_or(TerrainRendererError::NoTexLayoutForId)?
                .get_revision();
            if binding.tile_array_texture_revision != revision { 
                // Newer-than-expected atlas! Mark to rebuild it.
                textures_to_build.insert(binding.texture_id);
            }
        }
        for texture_id in textures_to_build.drain() {
            // Build our tile atlas
            let tile_array_texture = self.texture_layouts.get(&texture_id)
                .ok_or(TerrainRendererError::NoTexLayoutForId)?;
            let mut array_texture = ArrayTexture::new(tile_array_texture.clone(),
                Some(tile_array_texture.get_max_textures()),
                &self.texture_bind_group_layout,
                device)?;

            array_texture.full_rebuild(&self.texture_bind_group_layout, 
                device, 
                queue, 
                texture_source)?;
            self.built_textures.insert(texture_id, array_texture);
        }

        // Then, geometry.
        for (position, meshed_chunk) in self.meshed_chunks.drain() { 
            let ChunkMesh { verticies } = meshed_chunk;

            let chunk_vertex_buffer = device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&verticies),
                    usage: wgpu::BufferUsages::VERTEX,
                }
            );

            self.built_chunks.insert(position,
                BuiltChunk { 
                    buffer: chunk_vertex_buffer, 
                    num_verts: verticies.len() as u32
                });
        }
        Ok(())
    }
    pub fn draw(&mut self,
            render_surface_view: &TextureView,
            depth_texture_view: &TextureView,
            scale: Vec3,
            translation: Vec3,
            rotation: Quat,
            camera_bind_group: &wgpu::BindGroup,
            encoder: &mut wgpu::CommandEncoder) -> Result<(), TerrainRendererError> {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[
                Some(wgpu::RenderPassColorAttachment {
                    view: render_surface_view,
                    resolve_target: None, //No multisampling
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: true,
                    },
                }),
            ],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_texture_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                }),
                stencil_ops: None,
            }),
        });
        render_pass.set_pipeline(&self.render_pipeline);

        for (chunk_pos, mesh) in self.built_chunks.iter() { 
            let pos_int = chunk_to_world_pos(&chunk_pos);
            let chunk_origin = Vec3::new(pos_int.x as f32, pos_int.y as f32, pos_int.z as f32);
            let translated_origin = chunk_origin + translation;

            // This is very cursed and will be unperformant and should be replaced later.
            // Multiple arrays that are in sync, maybe? Arc<ArrayTexture> instead of weird
            // spread-out IDs in hashamps? 
            let chunk_texture_binding = self.texture_for_chunk.get(chunk_pos)
                .ok_or(TerrainRendererError::NoTexForChunk)?;
            let id = chunk_texture_binding.texture_id; 
            let texture_array = self.built_textures.get(&id)
                .ok_or(TerrainRendererError::NoTexForId)?;
            let texture = texture_array.get_handle();
            
            // Allowing scaling, translation, and rotation of worlds will help us later when/if 
            // vehicles become a thing.
            let model_matrix = Mat4::from_scale_rotation_translation(scale, 
                rotation, 
                translated_origin);

            render_pass.set_push_constants(ShaderStages::VERTEX, 
                0,
                &bytemuck::cast_slice(&[ModelPush::new(model_matrix)]));

            render_pass.set_bind_group(0, &texture.bind_group, &[]);
            render_pass.set_bind_group(1, camera_bind_group, &[]);
            render_pass.set_vertex_buffer(0, mesh.buffer.slice(..));
            render_pass.draw(0..(mesh.num_verts as u32), 0..1);
        }
        Ok(())
    }
}