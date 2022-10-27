use std::sync::Arc;

use futures::executor::block_on;
use glam::{Vec3, Quat};
use log::info;
use wgpu::{Instance, Surface, Adapter, AdapterInfo, DeviceDescriptor, Queue, Device};
use winit::window::Window;
use std::collections::{HashMap, HashSet};
use image::{Rgba, RgbaImage};

use crate::common::voxelmath::SidesArray;
use crate::resource::ResourceId;
use crate::resource::image::ImageProvider;
use crate::world::chunk::CHUNK_SIZE;
use crate::world::tilespace::{world_to_chunk_pos, TileSpaceError, TileSpace};
use crate::world::{ChunkPos, TilePos, TileId};
use crate::world::voxelstorage::{Voxel, VoxelSpace};

use self::tiletextureatlas::{TileAtlasLayout, build_tile_atlas, TileAtlasError};
use self::voxelmesher::{ChunkMesh, MesherState};

use super::clientmain::ClientConfig;

pub mod tiletextureatlas;
pub mod voxelmesher;

type TextureId = ResourceId;

pub struct Drawable { 

}

#[derive(thiserror::Error, Debug)]
pub enum InitRenderError {
    #[error("Unable to instantiate a rendering device: {0:?}")]
    CannotRequestDevice(#[from] wgpu::RequestDeviceError),
    #[error("Failed to request an adapter - no valid rendering device available!")]
    CannotRequestAdapter,
    #[error("Surface incompatible with adapter (As indicated by no preferred format).")]
    NoPreferredFormat,
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
}

impl Renderer { 
    pub async fn init(window: &Window, config: &ClientConfig) -> Result<Self, InitRenderError> { 
        // WGPU instance / drawing-surface.
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };

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
        //If we didn't already pick one, look for any Vulkan device.
        /*
        if adapter_select.is_none() { 
            for adapter_info in adapters.values().map(|a| a.get_info()) { 
                if adapter_info.backend == wgpu::Backend::Vulkan {
                    adapter_select = Some(adapter_info.name.clone());
                    break;
                }
            }
        }*/

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
        let supported_formats = surface.get_supported_formats(&adapter); 
        if supported_formats.is_empty() { 
            return Err(InitRenderError::NoPreferredFormat);
        }
        info!("Render surface supports formats: {:?}", supported_formats);

        let window_size = window.inner_size();
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT, // When we implement portals I am likely to touch this again. 
            format: *supported_formats.first().unwrap(),
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &surface_config);
        
        let aspect_ratio = (window_size.width as f32) / (window_size.height as f32); 

        Ok(Self {
            aspect_ratio,
            window_size,
            surface_config,
            instance,
            surface,
            adapter,
            queue,
            device
        })
    }
    /// Resize the display area
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.window_size = new_size;
            self.surface_config.width = new_size.width;
            self.surface_config.height = new_size.height;
            self.surface.configure(&self.device, &self.surface_config);
            self.aspect_ratio = (new_size.width as f32) / (new_size.height as f32);
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum TerrainRendererError {
    #[error("Error borrowing chunk for terrain renderer: {0:?}")]
    UnrecognizedTexture(#[from] TileSpaceError),
    #[error("Could not prepare meshing for chunk {0:?}, received error: {1:?}")]
    PrepareMeshingError(ChunkPos, String),
    #[error("Could not mesh chunk {0:?}, received error: {1:?}")]
    MeshingError(ChunkPos, String),
    #[error("Could not build tile texture atlas: {0:?}")]
    TileAtlasError(#[from] TileAtlasError)
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
/// Which type of voxel meshing is required to render this voxel cell? 
pub enum VoxelMesherPass {
    /// Any simple bloxel cube which is 1 meter in every direction, 
    /// and for which the tile ID can be mapped naively to a set of 
    /// textures for its six sides (without any extra state required)
    SimpleCubes,
}

/*
// Chunk mesh state which is produced by the mesher and then 
// consumed on uploading it to the GPU
struct IntermediaryChunkMesh { 
    geometry: ChunkMesh, 
    /// This does not correspond to anything on the GPU, it just 
    /// refers to a texture managed internally in the TerrainRenderer
    texture_id: u32,
}
impl IntermediaryChunkMesh {
    fn new(geometry: ChunkMesh, texture_id: u32) -> Self {
        IntermediaryChunkMesh {
            geometry,
            texture_id,
        }
    }
    fn get_texture_id(&self) -> u32 { 
        self.texture_id
    }
}*/

/// Handle to GPU data for this chunk
#[allow(dead_code)]
struct GpuChunkMeshHandle {
    mesh_handle: (), // TODO 
    object_handle: (),  // TODO
}

/// Data we keep around for a tile texture atlas for as long as we don't need to rebuild it.
#[allow(dead_code)]
struct BuiltTexture {
    last_built_revision: u64, 
    gpu_handle: (), // TODO
    material: (), // TODO
}

#[derive(Copy, Clone, Hash, Debug)]
struct ChunkTextureBinding { 
    texture_id: u32,
    tile_atlas_revision: u64,
}

/// A subsystem that is responsible for building, maintaining 
/// (i.e. owning memory / collecting garbage), and drawing 
/// meshes of world voxel terrain.
pub struct TerrainRenderer {
    /// Later this will be used to track tile positions rather than chunk positions, 
    /// so that partial rebuilds of a chunk are possible (Rather than total rebuilds every time)
    pending_remesh: HashSet<ChunkPos>,
    meshed_chunks: HashMap<ChunkPos, ChunkMesh>, 
    built_chunks: HashMap<ChunkPos, GpuChunkMeshHandle>,
    texture_for_chunk: HashMap<ChunkPos, ChunkTextureBinding>,
    texture_layouts: HashMap<u32, TileAtlasLayout>,
    built_textures: HashMap<u32, BuiltTexture>,
    /// One past the highest texture ID in texture_layouts. Incremented each time we add a new texture layout.
    next_texture_id: u32,
    texture_size: u32,
}

impl TerrainRenderer {
    pub fn new(texture_size: u32) -> Self { 
        TerrainRenderer {
            pending_remesh: HashSet::default(),
            meshed_chunks: HashMap::default(),
            built_chunks: HashMap::default(),
            texture_for_chunk: HashMap::default(),
            texture_layouts: HashMap::default(),
            built_textures: HashMap::default(),
            next_texture_id: 0,
            texture_size,
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
    fn make_new_atlas(&mut self) -> ChunkTextureBinding { 
        let new_texture_id = self.next_texture_id;
        self.next_texture_id += 1;
        let new_atlas = TileAtlasLayout::new(self.texture_size, 32, 8, Some(4096));
        self.texture_layouts.insert(new_texture_id, new_atlas);
        ChunkTextureBinding{ 
            texture_id: new_texture_id,
            tile_atlas_revision: 0,
        }
    }
    fn find_available_texture_atlas(&mut self) -> ChunkTextureBinding {
        if self.texture_layouts.is_empty() {
            self.make_new_atlas()
        }
        else {
            for (atlas_id, layout) in self.texture_layouts.iter() { 
                //Is it relatively safe to assume we won't overrun max layout size?
                if layout.get_tile_count() <= (layout.get_max_tiles()/2) { 
                    return ChunkTextureBinding { 
                        texture_id: *atlas_id, 
                        tile_atlas_revision: layout.get_revision(),
                    };
                }
            }
            self.make_new_atlas()
        }
    }
    /// Rebuild any meshes which have been flagged as changed. 
    /// Does not automatically push any mesh data to the GPU. Please use push_to_gpu() to update the meshes for rendering after calling this.
    /// Returns whether or not any remesh is actually required. 
    pub fn process_remesh<A: CubeArtMapper<TileId>>(&mut self, voxel_space: &TileSpace, tiles_to_art: &A) -> Result<bool, TerrainRendererError> {
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
                    self.find_available_texture_atlas()
                };
    
                let chunk = voxel_space.borrow_chunk(chunk_position)?;
    
                //TODO: Handle case where texture atlas goes over max
                let mesher_state = MesherState::prepare_to_mesh(chunk, tiles_to_art, self.texture_layouts.get_mut(&texture_binding.texture_id).unwrap())
                    .map_err(|e| { 
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
    pub fn push_to_gpu<TextureSource: ImageProvider>(&mut self, texture_source: &mut TextureSource) -> Result<(), TerrainRendererError> {
        // First, handle textures.
        let mut textures_to_build: HashSet<u32> = HashSet::new();
        for (_, binding) in self.texture_for_chunk.iter() {
            let revision = self.texture_layouts.get_mut(&binding.texture_id).unwrap().get_revision();
            if (binding.tile_atlas_revision < revision) || ((binding.tile_atlas_revision == 0) && (revision == 0)){ 
                // Newer-than-expected atlas! Mark to rebuild it.
                textures_to_build.insert(binding.texture_id);
            }
        }
        for texture_id in textures_to_build.iter() {
            // Build our tile atlas
            let tile_atlas = self.texture_layouts.get(texture_id).unwrap();
            let image = build_tile_atlas(tile_atlas, texture_source)?;

            // Set up Rend3 texture handle.
            /*let atlas_texture = rend3::types::Texture {
                label: Option::None,
                data: image.to_vec(),
                format: rend3::types::TextureFormat::Rgba8UnormSrgb,
                size: glam::UVec2::new(image.dimensions().0, image.dimensions().1),
                //No mipmaps allowed
                mip_count: rend3::types::MipmapCount::ONE,
                mip_source: rend3::types::MipmapSource::Uploaded,
            };

            let texture_handle = renderer.add_texture_2d(atlas_texture);

            // Add PBR material with all defaults except a single color.
            let material = rend3_routine::pbr::PbrMaterial {
                albedo: rend3_routine::pbr::AlbedoComponent::Texture(texture_handle.clone()), //Texture handle is an ARC internally. 
                unlit: true,
                sample_type: rend3_routine::pbr::SampleType::Nearest,
                ..rend3_routine::pbr::PbrMaterial::default()
            };
            let material_handle = renderer.add_material(material);
            self.built_textures.insert(*texture_id, BuiltTexture{
                last_built_revision: tile_atlas.get_revision(),
                gpu_handle: texture_handle,
                material: material_handle,
            });*/
        }

        // Then, geometry.
        for (position, meshed_chunk) in self.meshed_chunks.drain() { 
            let ChunkMesh { verticies, uv } = meshed_chunk;
            /*
            let mesh = MeshBuilder::new(verticies, Handedness::Left)
                .with_vertex_uv0(uv)
                .build()
                .unwrap();
    
            // Add mesh to renderer's world.
            // All handles are refcounted, so we only need to hang onto the handle until we make an object.
            let mesh_handle = renderer.add_mesh(mesh);

            let atlas = self.built_textures.get(&self.texture_for_chunk.get(&position).unwrap().texture_id).unwrap();

            let chunk_translation = Vec3::new(
                (position.x * CHUNK_SIZE as i32) as f32,
                (position.y * CHUNK_SIZE as i32) as f32,
                (position.z * CHUNK_SIZE as i32) as f32
            );
    
            // Combine the mesh and the material with a location to give an object.
            let object = rend3::types::Object {
                mesh: mesh_handle.clone(),
                material: atlas.material.clone(),
                transform: glam::Mat4::from_scale_rotation_translation(Vec3::ONE, Quat::IDENTITY, chunk_translation),
            };
            let object_handle = renderer.add_object(object);
            self.built_chunks.insert(position, GpuChunkMeshHandle {
                mesh_handle,
                object_handle,
            });*/
        }
        Ok(())
    }
}

pub trait CubeArtMapper<V>
where
    V: Voxel,
{
    fn get_art_for_tile(&self, tile: &V) -> Option<&CubeArt>;
}

impl<V> CubeArtMapper<V> for HashMap<V, CubeArt>
where
    V: Voxel,
{
    fn get_art_for_tile(&self, tile: &V) -> Option<&CubeArt> {
        self.get(tile)
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum CubeTex {
    Invisible,
    Single(TextureId),
    AllSides(Box<SidesArray<TextureId>>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CubeArt {
    pub textures: CubeTex,
    pub cull_self: bool,   //Do we cull the same material?
    pub cull_others: bool, //Do we cull materials other than this one?
}

impl CubeArt {
    pub fn get_render_type(&self) -> VoxelMesherPass {
        VoxelMesherPass::SimpleCubes
    }
    pub fn is_visible(&self) -> bool {
        !(self.textures == CubeTex::Invisible)
    }
    pub fn all_textures(&self) -> Vec<&TextureId> {
        match &self.textures {
            CubeTex::Invisible => Vec::default(),
            CubeTex::Single(v) => vec![v],
            CubeTex::AllSides(sides) => sides.iter().collect(),
        }
    }
    pub fn simple_solid_block(texture: &TextureId) -> Self {
        CubeArt {
            textures: CubeTex::Single(*texture),
            cull_self: true,
            cull_others: true,
        }
    }
    pub fn airlike() -> Self {
        CubeArt {
            textures: CubeTex::Invisible,
            cull_self: false,
            cull_others: false,
        }
    }
}

pub const AIR_ART: CubeArt = CubeArt {
    textures: CubeTex::Invisible,
    cull_self: false,
    cull_others: false,
};

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
