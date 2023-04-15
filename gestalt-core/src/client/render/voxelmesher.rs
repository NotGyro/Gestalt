use std::{collections::HashMap, error::Error};

use glam::{vec3, Vec2, Vec3};
use std::collections::HashSet;
use log::{error, warn};

use crate::common::voxelmath::VoxelPos;
use crate::{
    common::voxelmath::*,
    resource::{
        image::{ID_MISSING_TEXTURE, ID_PENDING_TEXTURE},
        ResourceId,
    },
    world::{
        chunk::{Chunk, ChunkInner, CHUNK_SIZE},
        voxelstorage::Voxel,
        TileId,
    },
};

use super::voxel_art::VoxelArt;
use super::{
    tiletextureatlas::{TileAtlasError, TileAtlasLayout},
};

use crate::world::chunk::{self as chunk, CHUNK_SIZE_CUBED};

/// A side index and voxel cell represented as [side_idx, x, y, z]
pub(super) struct SidePos([u8; 4]);

impl SidePos { 
    pub fn get_side_idx(&self) -> u8 { 
        self.0[0]
    }
    pub fn get_x(&self) -> u8 { 
        self.0[1]
    }
    pub fn get_y(&self) -> u8 { 
        self.0[2]
    }
    pub fn get_z(&self) -> u8 { 
        self.0[3]
    }
    pub fn set_side(&mut self, value: VoxelSide) { 
        self.0[0] = value.to_id()
    }
    pub fn set_side_idx(&mut self, value: u8) { 
        self.0[0] = value
    }
    pub fn set_x(&mut self, value: u8) { 
        self.0[1] = value
    }
    pub fn set_y(&mut self, value: u8) { 
        self.0[2] = value
    }
    pub fn set_z(&mut self, value: u8) { 
        self.0[3] = value
    }
}
pub(super) trait VoxelVertex: Sized + Send + Sync + bytemuck::Pod + bytemuck::Zeroable {
    /// Texture index as passed in when generating a face.
    type TexRepr : Sized + Send + Sync;
    const VERTICIES_PER_FACE: usize;
    fn buffer_layout() -> wgpu::VertexBufferLayout<'static>;
    fn generate_face(side_pos: SidePos, 
            texture_index: &Self::TexRepr) 
        -> [Self; Self::VERTICIES_PER_FACE];
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [u32; 3],
}
impl Vertex {
    pub fn get_x(&self) -> u32 {
        self.position[0]
    }
    pub fn get_y(&self) -> u32 {
        self.position[1]
    }
    pub fn get_z(&self) -> u32 {
        self.position[2]
    }
    pub fn to_rend3_vertex(&self) -> glam::f32::Vec3 {
        Vec3::new(
            self.get_x() as f32,
            self.get_y() as f32,
            self.get_z() as f32,
        )
    }
}

impl From<Vertex> for Vec3 {
    fn from(val: Vertex) -> Self {
        vec3(
            val.position[0] as f32,
            val.position[1] as f32,
            val.position[2] as f32,
        )
    }
}

const POSX_POSY_POSZ_VERT: Vertex = Vertex {
    position: [1, 1, 1],
};
const POSX_POSY_NEGZ_VERT: Vertex = Vertex {
    position: [1, 1, 0],
};
const POSX_NEGY_NEGZ_VERT: Vertex = Vertex {
    position: [1, 0, 0],
};
const POSX_NEGY_POSZ_VERT: Vertex = Vertex {
    position: [1, 0, 1],
};
const NEGX_POSY_NEGZ_VERT: Vertex = Vertex {
    position: [0, 1, 0],
};
const NEGX_POSY_POSZ_VERT: Vertex = Vertex {
    position: [0, 1, 1],
};
const NEGX_NEGY_POSZ_VERT: Vertex = Vertex {
    position: [0, 0, 1],
};
const NEGX_NEGY_NEGZ_VERT: Vertex = Vertex {
    position: [0, 0, 0],
};

const POSITIVE_X_FACE: [Vertex; 6] = [
    POSX_POSY_NEGZ_VERT,
    POSX_POSY_POSZ_VERT,
    POSX_NEGY_POSZ_VERT,
    //-Second triangle:
    POSX_NEGY_POSZ_VERT,
    POSX_NEGY_NEGZ_VERT,
    POSX_POSY_NEGZ_VERT,
];

const NEGATIVE_X_FACE: [Vertex; 6] = [
    //-First triangle:
    NEGX_POSY_POSZ_VERT,
    NEGX_POSY_NEGZ_VERT,
    NEGX_NEGY_NEGZ_VERT,
    //-Second triangle
    NEGX_NEGY_NEGZ_VERT,
    NEGX_NEGY_POSZ_VERT,
    NEGX_POSY_POSZ_VERT,
];

const POSITIVE_Y_FACE: [Vertex; 6] = [
    //-First triangle:
    NEGX_POSY_NEGZ_VERT,
    NEGX_POSY_POSZ_VERT,
    POSX_POSY_POSZ_VERT,
    //-Second triangle
    POSX_POSY_POSZ_VERT,
    POSX_POSY_NEGZ_VERT,
    NEGX_POSY_NEGZ_VERT,
];

const NEGATIVE_Y_FACE: [Vertex; 6] = [
    //-First triangle:
    POSX_NEGY_NEGZ_VERT,
    POSX_NEGY_POSZ_VERT,
    NEGX_NEGY_POSZ_VERT,
    //-Second triangle
    NEGX_NEGY_POSZ_VERT,
    NEGX_NEGY_NEGZ_VERT,
    POSX_NEGY_NEGZ_VERT,
];

const POSITIVE_Z_FACE: [Vertex; 6] = [
    //-First triangle:
    POSX_POSY_POSZ_VERT,
    NEGX_POSY_POSZ_VERT,
    NEGX_NEGY_POSZ_VERT,
    //-Second triangle
    NEGX_NEGY_POSZ_VERT,
    POSX_NEGY_POSZ_VERT,
    POSX_POSY_POSZ_VERT,
];

const NEGATIVE_Z_FACE: [Vertex; 6] = [
    //-First triangle:
    NEGX_POSY_NEGZ_VERT,
    POSX_POSY_NEGZ_VERT,
    POSX_NEGY_NEGZ_VERT,
    //-Second triangle
    POSX_NEGY_NEGZ_VERT,
    NEGX_NEGY_NEGZ_VERT,
    NEGX_POSY_NEGZ_VERT,
];

fn get_face_verts(side: VoxelSide) -> [Vertex; 6] {
    match side {
        VoxelSide::PosiX => POSITIVE_X_FACE,
        VoxelSide::NegaX => NEGATIVE_X_FACE,
        VoxelSide::PosiY => POSITIVE_Y_FACE,
        VoxelSide::NegaY => NEGATIVE_Y_FACE,
        VoxelSide::PosiZ => POSITIVE_Z_FACE,
        VoxelSide::NegaZ => NEGATIVE_Z_FACE,
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum TextureLookupError {
    #[error("Texture ID has not been loaded or is not valid: {0}")]
    UnrecognizedTexture(String),
    #[error("Tried to look up a texture but no texture mapping found for tile ID {0}")]
    UnrecognizedTile(String),
    #[error("Tried to associate tile {0} with texture {1}, but that texture (which should have been loaded into the renderer already) has not been loaded in to the renderer.")]
    FileNotLoaded(String, String),
}

/* Commented out because this was from when tile textures were an atlas and not an arraytexture
#[derive(Copy, Clone, Default, Debug, PartialEq)]
// We record the associated U,V values in this implementation (for the Texture Atlas)
pub struct UvCache {
    pub(crate) lower_u: f32,
    pub(crate) lower_v: f32,
    pub(crate) higher_u: f32,
    pub(crate) higher_v: f32,
} */

type TextureArrayIndex = u16;

type SidesCache = SidesArray<ResourceId>;

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
struct CubeArtNotes {
    /// Should *this voxel mesher code* draw this tile?
    pub visible_this_pass: bool,
    /// Do we cull the same material? i.e do other tiles with the same ID get culled by this one?
    pub cull_self: bool,
    /// Do we cull other materials? i.e do other tiles with different IDs get culled by this one?
    pub cull_others: bool,
}

impl From<&VoxelArt> for CubeArtNotes {
    fn from(art: &VoxelArt) -> Self {
        match art {
            VoxelArt::SimpleCube(cube) => {
                CubeArtNotes { 
                    visible_this_pass: true,
                    cull_self: cube.cull_self,
                    cull_others: cube.cull_others,
                }
            },
            _ => CubeArtNotes {
                visible_this_pass: false, 
                cull_self: false,
                cull_others: false,
            },
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub struct ArtCacheEntry {
    pub(crate) textures: SidesCache,
    pub(crate) tile_info: CubeArtNotes,
}

impl ArtCacheEntry {
    fn new(art: &CubeArt, atlas: &mut TileAtlasLayout) -> Self {
        let notes = CubeArtNotes::from(art);
        if !notes.visible {
            return Self {
                textures: SidesCache::default(),
                tile_info: notes,
            };
        }
        let sides_uv = match sides_cache_from_art(art, atlas) {
            Ok(Some(sides)) => sides,
            _ => sides_cache_missing_texture(atlas),
        };
        Self {
            textures: sides_uv,
            tile_info: notes,
        }
    }
}

const CUBE_ART_MISSING_TEXTURE: CubeArt = CubeArt {
    textures: CubeTex::Single(ID_MISSING_TEXTURE),
    cull_self: true,
    cull_others: true,
};

fn uv_cache_from_resource(
    resource: &ResourceId,
    layout: &mut TileAtlasLayout,
) -> Result<UvCache, TileAtlasError> {
    let idx = layout.get_or_make_index_for_texture(resource)?;
    let lower_uv = layout.get_uv_for_index(idx, false, false);
    let higher_uv = layout.get_uv_for_index(idx, true, true);

    Ok(UvCache {
        lower_u: lower_uv.x,
        lower_v: lower_uv.y,
        higher_u: higher_uv.x,
        higher_v: higher_uv.y,
    })
}

fn sides_cache_from_art(
    art: &CubeArt,
    layout: &mut TileAtlasLayout,
) -> Result<Option<SidesCache>, TileAtlasError> {
    match &art {
        CubeTex::Invisible => Ok(None),
        CubeTex::Single(t) => Ok(Some(SidesArray::new_uniform(&uv_cache_from_resource(
            t, layout,
        )?))),
        CubeTex::AllSides(a) => {
            let mut result_array: [UvCache; 6] = [UvCache::default(); 6];
            for (i, uv_cache) in result_array.iter_mut().enumerate() {
                *uv_cache = uv_cache_from_resource(a.get_i(i), layout)?;
            }
            Ok(Some(SidesCache { data: result_array }))
        }
    }
}

fn sides_cache_missing_texture(layout: &mut TileAtlasLayout) -> SidesCache {
    let lower_uv = layout.get_missing_texture_uvs(false, false);
    let higher_uv = layout.get_missing_texture_uvs(true, true);

    let uv = UvCache {
        lower_u: lower_uv.x,
        lower_v: lower_uv.y,
        higher_u: higher_uv.x,
        higher_v: higher_uv.y,
    };

    SidesCache::new_uniform(&uv)
}

fn art_cache_missing_texture(layout: &mut TileAtlasLayout) -> ArtCacheEntry {
    ArtCacheEntry {
        textures: sides_cache_missing_texture(layout),
        tile_info: CubeArtNotes::from(&CUBE_ART_MISSING_TEXTURE),
    }
}

pub trait ArtCache {
    /// Return "None" if invisible block.
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry>;
    /// Should we draw anything for this chunk at all?
    fn is_any_visible(&self) -> bool;
}

pub struct ArtCacheUniform {
    value: ArtCacheEntry,
    missing_texture: ArtCacheEntry,
}
impl ArtCacheUniform {
    pub fn new(value: ArtCacheEntry, missing_texture: ArtCacheEntry) -> Self {
        Self {
            value,
            missing_texture,
        }
    }
}

impl ArtCache for ArtCacheUniform {
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry> {
        if idx != 0 {
            error!("Non-zero index in supposedly-uniform chunk! This shouldn't happen. Using missing texture.");
            return Some(&self.missing_texture);
        }
        match self.value.tile_info.visible {
            true => Some(&self.value),
            false => None,
        }
    }
    fn is_any_visible(&self) -> bool {
        self.value.tile_info.visible
    }
}

pub struct ArtCacheSmall {
    data: [ArtCacheEntry; 256],
    missing_texture: ArtCacheEntry,
}
impl ArtCacheSmall {
    pub fn new(data: [ArtCacheEntry; 256], missing_texture: ArtCacheEntry) -> Self {
        Self {
            data,
            missing_texture,
        }
    }
}

impl ArtCache for ArtCacheSmall {
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry> {
        if idx > (u8::MAX as u16) {
            error!("Over-255 index supposedly-small chunk! This shouldn't happen. Using missing texture.");
            return Some(&self.missing_texture);
        }
        self.data
            .get(idx as usize)
            .and_then(|a| if !a.tile_info.visible { None } else { Some(a) })
    }
    fn is_any_visible(&self) -> bool {
        true
    }
}

pub struct ArtCacheLarge {
    data: HashMap<u16, ArtCacheEntry>,
}

impl ArtCache for ArtCacheLarge {
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry> {
        self.data
            .get(&idx)
            .and_then(|a| if !a.tile_info.visible { None } else { Some(a) })
    }
    fn is_any_visible(&self) -> bool {
        true
    }
}

impl ArtCacheLarge {
    pub fn new(palette: HashMap<u16, ArtCacheEntry>) -> Self {
        Self { data: palette }
    }
}

pub type OutputVertex = Vec3;
pub type OutputUv = Vec2;
#[derive(Default, Debug, Clone)]
pub struct ChunkMesh {
    pub verticies: Vec<OutputVertex>,
    pub uv: Vec<OutputUv>,
}

impl ChunkMesh { 
    pub fn zero() -> Self { 
        ChunkMesh {
            verticies: Vec::default(),
            uv: Vec::default(),
        }
    }
}

pub enum ArtCacheHolder {
    Uniform(ArtCacheUniform),
    Small(ArtCacheSmall),
    Large(ArtCacheLarge),
}

pub struct MesherState<'a> {
    pub art_cache: ArtCacheHolder,
    pub chunk: &'a Chunk<TileId>,
    pub textures_needed: HashSet<ResourceId>,
}

impl<'a> MesherState<'a> {
    pub fn prepare_to_mesh<A: CubeArtMapper<TileId>>(
        chunk: &'a Chunk<TileId>,
        tiles_to_art: &A,
        atlas: &mut TileAtlasLayout,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (inner, mut textures_needed): (ArtCacheHolder, HashSet<ResourceId>) = match &chunk.inner {
            ChunkInner::Uniform(val) => {
                let missing_texture = art_cache_missing_texture(atlas);
                let mut textures_needed = HashSet::new();
                let cube_art = match tiles_to_art.get_art_for_tile(val) {
                    Some(art) => {
                        for t in art.all_textures() {
                            textures_needed.insert(*t);
                        }
                        ArtCacheEntry::new(art, atlas)
                    }
                    None => {
                        warn!(
                            "No art loaded for world-level tile ID {}. Using missing texture.",
                            val
                        );
                        missing_texture
                    }
                };
                let art_cache = ArtCacheUniform::new(cube_art, missing_texture);

                (ArtCacheHolder::Uniform(art_cache), textures_needed)
            }
            ChunkInner::Small(chunk_inner) => {
                let missing_texture = art_cache_missing_texture(atlas);
                let mut textures_needed = HashSet::new();
                let mut art_palette: [ArtCacheEntry; 256] = [ArtCacheEntry::default(); 256];
                //Iterate through the palette
                for i in 0..(chunk_inner.highest_idx + 1) {
                    let tile = chunk_inner.palette[i as usize];
                    let cube_art = match tiles_to_art.get_art_for_tile(&tile) {
                        Some(art) => {
                            for t in art.all_textures() {
                                textures_needed.insert(*t);
                            }
                            ArtCacheEntry::new(art, atlas)
                        }
                        None => {
                            warn!(
                                "No art loaded for world-level tile ID {}. Using missing texture.",
                                tile
                            );
                            missing_texture
                        }
                    };
                    art_palette[i as usize] = cube_art;
                }

                let art_cache = ArtCacheSmall::new(art_palette, missing_texture);
                (ArtCacheHolder::Small(art_cache), textures_needed)
            }
            ChunkInner::Large(chunk_inner) => {
                let missing_texture = art_cache_missing_texture(atlas);
                let mut textures_needed = HashSet::new();
                let mut art_palette: HashMap<u16, ArtCacheEntry> =
                    HashMap::with_capacity(chunk_inner.palette.len());
                //Iterate through the palette
                for (idx, tile) in chunk_inner.palette.iter() {
                    let cube_art = match tiles_to_art.get_art_for_tile(tile) {
                        Some(art) => {
                            for t in art.all_textures() {
                                textures_needed.insert(*t);
                            }
                            ArtCacheEntry::new(art, atlas)
                        }
                        None => {
                            warn!(
                                "No art loaded for world-level tile ID {}. Using missing texture.",
                                tile
                            );
                            missing_texture
                        }
                    };
                    art_palette.insert(*idx, cube_art);
                }

                let art_cache = ArtCacheLarge::new(art_palette);
                (ArtCacheHolder::Large(art_cache), textures_needed)
            }
        };

        // Make sure we're not telling the system to download the missing texture thing
        textures_needed.remove(&ID_MISSING_TEXTURE);
        textures_needed.remove(&ID_PENDING_TEXTURE);

        Ok(Self {
            art_cache: inner,
            chunk,
            textures_needed,
        })
    }

    /// Do we need to render this at all? Used in order to avoid wasting bookkeeping on all-air chunks.
    pub fn needs_draw(&self) -> bool { 
        match &self.art_cache {
            ArtCacheHolder::Uniform(art_cache) => art_cache.is_any_visible(),
            ArtCacheHolder::Small(art_cache) => art_cache.is_any_visible(),
            ArtCacheHolder::Large(art_cache) => art_cache.is_any_visible(),
        }
    }

    pub fn build_mesh(&self) -> Result<ChunkMesh, Box<dyn Error>> {
        match &self.art_cache {
            ArtCacheHolder::Uniform(art_cache) => if art_cache.is_any_visible() { build_mesh(self.chunk, art_cache) } else { Ok(ChunkMesh::zero()) },
            ArtCacheHolder::Small(art_cache) => build_mesh(self.chunk, art_cache),
            ArtCacheHolder::Large(art_cache) => build_mesh(self.chunk, art_cache),
        }
    }
}

// Make a mesh in one single blocking action (does not permit you to share one tile atlas between chunks)
pub fn make_mesh_completely<A: CubeArtMapper<TileId>>(
    texture_size: u32,
    chunk: &Chunk<TileId>,
    tiles_to_art: &A,
) -> Result<(ChunkMesh, TileAtlasLayout), Box<dyn std::error::Error>> {
    let mut atlas = TileAtlasLayout::new(texture_size, 32, 8, None);

    let state = MesherState::prepare_to_mesh(chunk, tiles_to_art, &mut atlas)?;

    Ok((state.build_mesh()?, atlas))
}

macro_rules! offset_unroll {
    ($side:ident, $idx_offset:ident, $idx:ident, $standard_side_index:ident $b:block) => {{
        const $side: VoxelSide = VoxelSide::PosiX;
        const $standard_side_index: usize = posi_x_index!();
        let $idx_offset = chunk::get_pos_x_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaX;
        const $standard_side_index: usize = nega_x_index!();
        let $idx_offset = chunk::get_neg_x_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::PosiY;
        const $standard_side_index: usize = posi_y_index!();
        let $idx_offset = chunk::get_pos_y_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaY;
        const $standard_side_index: usize = nega_y_index!();
        let $idx_offset = chunk::get_neg_y_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::PosiZ;
        const $standard_side_index: usize = posi_z_index!();
        let $idx_offset = chunk::get_pos_z_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaZ;
        const $standard_side_index: usize = nega_z_index!();
        let $idx_offset = chunk::get_neg_z_offset($idx, CHUNK_SIZE);
        $b
    }};
}

#[derive(Copy, Clone, Debug)]
pub struct SideRenderInfo {
    pub side: VoxelSide,
    pub cell: VoxelPos<u16>,
    pub uv: UvCache,
}

#[inline]
fn per_face_step(
    voxel_face: SideRenderInfo,
    vertex_buffer: &mut Vec<Vec3>,
    uv_buffer: &mut Vec<Vec2>,
) {
    for vert_iter in 0..6 {
        let x = voxel_face.cell.x;
        let y = voxel_face.cell.y;
        let z = voxel_face.cell.z;
        let mut temp_vert = get_face_verts(voxel_face.side)[vert_iter];
        temp_vert.position[0] += x as u32;
        temp_vert.position[1] += y as u32;
        temp_vert.position[2] += z as u32;
        let mut u: f32 = 0.0;
        let mut v: f32 = 0.0;

        //Do our UV the hacky way.
        if (vert_iter == 2) || (vert_iter == 3) {
            u = voxel_face.uv.higher_u;
            v = voxel_face.uv.higher_v;
        } else if (vert_iter == 0) || (vert_iter == 5) {
            u = voxel_face.uv.lower_u;
            v = voxel_face.uv.lower_v;
        } else if vert_iter == 1 {
            u = voxel_face.uv.higher_u;
            v = voxel_face.uv.lower_v;
        } else if vert_iter == 4 {
            u = voxel_face.uv.lower_u;
            v = voxel_face.uv.higher_v;
        }

        uv_buffer.push(Vec2::new(u, v));
        vertex_buffer.push(temp_vert.to_rend3_vertex());
    }
}

fn build_mesh<V: Voxel, A: ArtCache>(
    chunk: &Chunk<V>,
    art_cache: &A,
) -> Result<ChunkMesh, Box<dyn Error>> {
    let mut vertex_buffer: Vec<Vec3> = Vec::new();
    let mut uv_buffer: Vec<Vec2> = Vec::new();

    for i in 0..CHUNK_SIZE_CUBED {
        let tile = chunk.get_raw_i(i);
        if let Some(art) = art_cache.get_mapping(tile) {
            // Skip it if it's air.
            if art.tile_info.visible {
                offset_unroll!(SIDE, offset_idx, i, SIDE_INDEX {
                    let mut cull: bool = false;
                    if let Some(neighbor_idx) = offset_idx {
                        let neighbor_tile = chunk.get_raw_i(neighbor_idx);
                        if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                            cull = neighbor_art.tile_info.visible
                                && ( (art.tile_info.cull_self && (tile == neighbor_tile) )
                                || (art.tile_info.cull_others && (tile != neighbor_tile) ) );
                        }
                    }
                    if !cull {
                        let (x,y,z) = chunk_i_to_xyz(i, CHUNK_SIZE);
                        let sri = SideRenderInfo {
                            side : SIDE,
                            cell: vpos!(x as u16, y as u16, z as u16),
                            uv : art.textures.data[SIDE_INDEX] };
                        per_face_step(sri, &mut vertex_buffer, &mut uv_buffer);
                    }
                });
            }
        }
    }

    Ok(ChunkMesh {
        verticies: vertex_buffer,
        uv: uv_buffer,
    })
}
