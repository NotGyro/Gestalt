use std::{collections::HashMap, error::Error};

use glam::{vec3, Vec2, Vec3};
use lazy_static::__Deref;
use nohash::BuildNoHashHasher;
use std::collections::HashSet;
use log::{error, warn};

use crate::common::{FastHashSet, new_fast_hash_set, FastHashMap, new_fast_hash_map};
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

use super::array_texture::{ArrayTextureLayout, ArrayTextureError};
use super::voxel_art::{VoxelArt, CubeArt, CubeTex, VoxelArtMapper};
use crate::world::chunk::CHUNK_SIZE_CUBED;
use crate::world::voxelarray;

/// A side index and voxel cell represented as [side_idx, x, y, z]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
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
    pub fn new(side_idx: u8, x: u8, y: u8, z: u8) -> Self {
        Self([
            side_idx,
            x,
            y,
            z
        ])
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
pub struct IntermediateVertex {
    position: [u8; 3],
}
impl IntermediateVertex {
    pub fn get_x(&self) -> u8 {
        self.position[0]
    }
    pub fn get_y(&self) -> u8 {
        self.position[1]
    }
    pub fn get_z(&self) -> u8 {
        self.position[2]
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct PackedVertex { 
    // 6 bits x, 6 bits y, 6 bits z
    // 1 bit u, 1 bit v, 12 bits texture id
    vertex_data: u32,
}

//Bitmask
//V. U, tex, Z, Y, X
//b0_0_000000000000_000000_000000_000000
impl PackedVertex { 
    pub fn set_x(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_000000_000000_111111;

        self.vertex_data = self.vertex_data & (! bitmask); //clear out value
        self.vertex_data = self.vertex_data | (value & bitmask); //Set our value
    }
    pub fn set_y(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_000000_111111_000000;

        self.vertex_data = self.vertex_data & (! bitmask); //clear out value
        
        let mut val = value;
        val = val << 6;

        self.vertex_data = self.vertex_data | (val & bitmask); //Set our value
    }
    pub fn set_z(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_111111_000000_000000;

        self.vertex_data = self.vertex_data & (! bitmask); //clear out value

        let mut val = value;
        val = val << 12;

        self.vertex_data = self.vertex_data | (val & bitmask); //Set our value
    }

    pub fn set_tex_id(&mut self, texture_idx: u16) {
        let bitmask : u32 = 0b0_0_111111111111_000000_000000_000000;
        self.vertex_data = self.vertex_data & (! bitmask); //clear out value
        self.vertex_data = self.vertex_data | (texture_idx as u32 & bitmask); //Set our value
    }

    pub fn set_u_low(&mut self) {
        let bitmask : u32 = 0b0_1_000000000000_000000_000000_000000;
        self.vertex_data &= !bitmask;
    }
    pub fn set_u_high(&mut self) {
        let bitmask : u32 = 0b0_1_000000000000_000000_000000_000000;
        self.vertex_data |= bitmask;
    }
    pub fn set_v_low(&mut self) {
        let bitmask : u32 = 0b1_0_000000000000_000000_000000_000000;
        self.vertex_data &= !bitmask;
    }
    pub fn set_v_high(&mut self) {
        let bitmask : u32 = 0b1_0_000000000000_000000_000000_000000;
        self.vertex_data |= bitmask;
    }
    pub fn new(x: u8, y: u8, z: u8) -> Self { 
        let mut ret = Self::default();
        ret.set_x(x as u32); 
        ret.set_y(y as u32); 
        ret.set_z(z as u32);
        ret
    }
    pub fn from_vertex_uv(pos: (u8, u8, u8), u : u8, v : u8) -> Self { 
        let mut ret = Self::new(pos.0, pos.1, pos.2);
        if u >= 1 {
            ret.set_u_high();
        }
        if v >= 1 {
            ret.set_v_high();
        }
        return ret;
    }
}
impl From<IntermediateVertex> for PackedVertex {
    fn from(value: IntermediateVertex) -> Self {
        Self::new(value.get_x(), value.get_y(), value.get_z())
    }
}

const POSX_POSY_POSZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [1, 1, 1],
};
const POSX_POSY_NEGZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [1, 1, 0],
};
const POSX_NEGY_NEGZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [1, 0, 0],
};
const POSX_NEGY_POSZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [1, 0, 1],
};
const NEGX_POSY_NEGZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [0, 1, 0],
};
const NEGX_POSY_POSZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [0, 1, 1],
};
const NEGX_NEGY_POSZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [0, 0, 1],
};
const NEGX_NEGY_NEGZ_VERT: IntermediateVertex = IntermediateVertex {
    position: [0, 0, 0],
};

const POSITIVE_X_FACE: [IntermediateVertex; 6] = [
    POSX_POSY_NEGZ_VERT,
    POSX_POSY_POSZ_VERT,
    POSX_NEGY_POSZ_VERT,
    //-Second triangle:
    POSX_NEGY_POSZ_VERT,
    POSX_NEGY_NEGZ_VERT,
    POSX_POSY_NEGZ_VERT,
];

const NEGATIVE_X_FACE: [IntermediateVertex; 6] = [
    //-First triangle:
    NEGX_POSY_POSZ_VERT,
    NEGX_POSY_NEGZ_VERT,
    NEGX_NEGY_NEGZ_VERT,
    //-Second triangle
    NEGX_NEGY_NEGZ_VERT,
    NEGX_NEGY_POSZ_VERT,
    NEGX_POSY_POSZ_VERT,
];

const POSITIVE_Y_FACE: [IntermediateVertex; 6] = [
    //-First triangle:
    NEGX_POSY_NEGZ_VERT,
    NEGX_POSY_POSZ_VERT,
    POSX_POSY_POSZ_VERT,
    //-Second triangle
    POSX_POSY_POSZ_VERT,
    POSX_POSY_NEGZ_VERT,
    NEGX_POSY_NEGZ_VERT,
];

const NEGATIVE_Y_FACE: [IntermediateVertex; 6] = [
    //-First triangle:
    POSX_NEGY_NEGZ_VERT,
    POSX_NEGY_POSZ_VERT,
    NEGX_NEGY_POSZ_VERT,
    //-Second triangle
    NEGX_NEGY_POSZ_VERT,
    NEGX_NEGY_NEGZ_VERT,
    POSX_NEGY_NEGZ_VERT,
];

const POSITIVE_Z_FACE: [IntermediateVertex; 6] = [
    //-First triangle:
    POSX_POSY_POSZ_VERT,
    NEGX_POSY_POSZ_VERT,
    NEGX_NEGY_POSZ_VERT,
    //-Second triangle
    NEGX_NEGY_POSZ_VERT,
    POSX_NEGY_POSZ_VERT,
    POSX_POSY_POSZ_VERT,
];

const NEGATIVE_Z_FACE: [IntermediateVertex; 6] = [
    //-First triangle:
    NEGX_POSY_NEGZ_VERT,
    POSX_POSY_NEGZ_VERT,
    POSX_NEGY_NEGZ_VERT,
    //-Second triangle
    POSX_NEGY_NEGZ_VERT,
    NEGX_NEGY_NEGZ_VERT,
    NEGX_POSY_NEGZ_VERT,
];

fn get_face_verts(side: VoxelSide) -> [IntermediateVertex; 6] {
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
}*/

type ArrayTextureIndex = u16;
type SidesCache = SidesArray<ArrayTextureIndex>;

#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub(super) struct CubeArtNotes {
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
    pub(super) textures: SidesCache,
    pub(super) tile_info: CubeArtNotes,
}

impl ArtCacheEntry {
    fn new(art: &VoxelArt, layout: &mut ArrayTextureLayout) -> Option<Self> {
        let notes = CubeArtNotes::from(art);
        if !notes.visible_this_pass {
            return None;
        }
        let sides_textures = match sides_cache_from_art(art, layout) {
            Ok(Some(sides)) => sides,
            _ => sides_cache_missing_texture(layout),
        };
        Some(Self {
            textures: sides_textures,
            tile_info: notes,
        })
    }
}

const VOXEL_ART_MISSING_TEXTURE: VoxelArt = VoxelArt::SimpleCube(
    CubeArt {
        textures: CubeTex::Single(ID_MISSING_TEXTURE),
        cull_self: true,
        cull_others: true,
    }
);

fn idx_from_resource(
    resource: &ResourceId,
    layout: &mut ArrayTextureLayout,
) -> Result<ArrayTextureIndex, ArrayTextureError> {
    let idx = layout.get_or_make_index_for_texture(resource)?;

    Ok(idx as ArrayTextureIndex)
}

fn sides_cache_from_art(
    art: &VoxelArt,
    layout: &mut ArrayTextureLayout,
) -> Result<Option<SidesCache>, ArrayTextureError> {
    Ok(match &art {
        VoxelArt::Invisible => None,
        VoxelArt::SimpleCube(cube) => Some(match &cube.textures {
            CubeTex::Single(r_id) => {
                SidesCache::new_uniform(
                    &(layout.get_or_make_index_for_texture(r_id)? as ArrayTextureIndex)
                )
            },
            CubeTex::AllSides(sides) => {
                let mut new_sides = SidesCache::default(); 
                for (i, side) in sides.iter().enumerate() {
                    let value = layout.get_or_make_index_for_texture(side)? as ArrayTextureIndex; 
                    new_sides.set_i(value, i)
                }
                new_sides
            },
        }),
    })
}

fn sides_cache_missing_texture(layout: &mut ArrayTextureLayout) -> SidesCache {
    let missing_texture_idx = layout.get_missing_texture_idx();

    SidesCache::new_uniform(&(missing_texture_idx as u16))
}

fn art_cache_missing_texture(layout: &mut ArrayTextureLayout) -> ArtCacheEntry {
    ArtCacheEntry {
        textures: sides_cache_missing_texture(layout),
        tile_info: CubeArtNotes::from(&VOXEL_ART_MISSING_TEXTURE),
    }
}

pub trait ArtCache {
    /// Return "None" if invisible block.
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry>;
    /// Should we draw anything for this chunk at all?
    fn is_any_visible(&self) -> bool;
}

pub struct ArtCacheUniform {
    value: Option<ArtCacheEntry>,
    missing_texture: ArtCacheEntry,
}
impl ArtCacheUniform {
    pub fn new(value: Option<ArtCacheEntry>, missing_texture: ArtCacheEntry) -> Self {
        Self {
            value,
            missing_texture,
        }
    }
}

impl ArtCache for ArtCacheUniform {
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry> {
        #[cfg(debug_assertions)]
        {
            if idx != 0 {
                error!("Non-zero index in supposedly-uniform chunk! This shouldn't happen. Using missing texture.");
                return Some(&self.missing_texture);
            }
        }

        self.value.as_ref()
    }
    fn is_any_visible(&self) -> bool {
        match self.value { 
            Some(val) => { 
                val.tile_info.visible_this_pass
            },
            None => false,
        }
    }
}

pub struct ArtCacheSmall {
    data: [Option<ArtCacheEntry>; 256],
    missing_texture: ArtCacheEntry,
}
impl ArtCacheSmall {
    pub fn new(data: [Option<ArtCacheEntry>; 256], missing_texture: ArtCacheEntry) -> Self {
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
            .get(idx as usize).map(|v| v.as_ref()).flatten()
    }
    fn is_any_visible(&self) -> bool {
        true
    }
}

pub struct ArtCacheLarge {
    data: FastHashMap<u16, Option<ArtCacheEntry>>,
}

impl ArtCache for ArtCacheLarge {
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry> {
        self.data
            .get(&idx).map(|v| v.as_ref()).flatten()
    }
    fn is_any_visible(&self) -> bool {
        true
    }
}

impl ArtCacheLarge {
    pub fn new(palette: FastHashMap<u16, Option<ArtCacheEntry>>) -> Self {
        Self { data: palette }
    }
}

pub(super) type OutputVertex = PackedVertex;

#[derive(Default, Debug, Clone)]
pub struct ChunkMesh {
    pub(super) verticies: Vec<OutputVertex>,
}

impl ChunkMesh { 
    pub fn zero() -> Self { 
        ChunkMesh {
            verticies: Vec::default(),
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
    pub textures_needed: FastHashSet<ResourceId>,
}

impl<'a> MesherState<'a> {
    pub fn prepare_to_mesh<A: VoxelArtMapper<TileId>>(
        chunk: &'a Chunk<TileId>,
        tiles_to_art: &A,
        layout: &mut ArrayTextureLayout,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (inner, mut textures_needed): (ArtCacheHolder, FastHashSet<ResourceId>) = match &chunk.tiles {
            ChunkInner::Uniform(val) => {
                let missing_texture = art_cache_missing_texture(layout);
                let mut textures_needed = new_fast_hash_set();
                let cube_art = match tiles_to_art.get_art_for_tile(val) {
                    Some(art) => {
                        for t in art.all_textures() {
                            textures_needed.insert(*t);
                        }
                        ArtCacheEntry::new(art, layout)
                    }
                    None => {
                        warn!(
                            "No art loaded for world-level tile ID {}. Using missing texture.",
                            val
                        );
                        Some(missing_texture)
                    }
                };
                let art_cache = ArtCacheUniform::new(cube_art, missing_texture);

                (ArtCacheHolder::Uniform(art_cache), textures_needed)
            }
            ChunkInner::Small(chunk_inner) => {
                let missing_texture = art_cache_missing_texture(layout);
                let mut textures_needed = new_fast_hash_set();
                let mut art_palette: [Option<ArtCacheEntry>; 256] = [None; 256];
                //Iterate through the palette
                for i in 0..(chunk_inner.highest_idx + 1) {
                    let tile = chunk_inner.palette[i as usize];
                    let cube_art = match tiles_to_art.get_art_for_tile(&tile) {
                        Some(art) => {
                            for t in art.all_textures() {
                                textures_needed.insert(*t);
                            }
                            ArtCacheEntry::new(art, layout)
                        }
                        None => {
                            warn!(
                                "No art loaded for world-level tile ID {}. Using missing texture.",
                                tile
                            );
                            Some(missing_texture)
                        }
                    };
                    art_palette[i as usize] = cube_art;
                }

                let art_cache = ArtCacheSmall::new(art_palette, missing_texture);
                (ArtCacheHolder::Small(art_cache), textures_needed)
            }
            ChunkInner::Large(chunk_inner) => {
                let missing_texture = art_cache_missing_texture(layout);
                let mut textures_needed = new_fast_hash_set();
                let mut art_palette: FastHashMap<u16, Option<ArtCacheEntry>> = new_fast_hash_map();
                //Iterate through the palette
                for (idx, tile) in chunk_inner.palette.iter().enumerate() {
                    let cube_art = match tiles_to_art.get_art_for_tile(tile) {
                        Some(art) => {
                            for t in art.all_textures() {
                                textures_needed.insert(*t);
                            }
                            ArtCacheEntry::new(art, layout)
                        }
                        None => {
                            warn!(
                                "No art loaded for world-level tile ID {}. Using missing texture.",
                                tile
                            );
                            Some(missing_texture)
                        }
                    };
                    art_palette.insert(idx as u16, cube_art);
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
pub fn make_mesh_completely<A: VoxelArtMapper<TileId>>(
    texture_size: u32,
    chunk: &Chunk<TileId>,
    tiles_to_art: &A,
    max_texture_layers: Option<u32>,
) -> Result<(ChunkMesh, ArrayTextureLayout), Box<dyn std::error::Error>> {
    let mut layout = ArrayTextureLayout::new(
        (texture_size,texture_size), 
        max_texture_layers);

    let state = MesherState::prepare_to_mesh(chunk, tiles_to_art, &mut layout)?;

    Ok((state.build_mesh()?, layout))
}

macro_rules! offset_unroll {
    ($side:ident, $idx_offset:ident, $idx:ident, $standard_side_index:ident $b:block) => {{
        const $side: VoxelSide = VoxelSide::PosiX;
        const $standard_side_index: usize = posi_x_index!();
        let $idx_offset = voxelarray::get_pos_x_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaX;
        const $standard_side_index: usize = nega_x_index!();
        let $idx_offset = voxelarray::get_neg_x_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::PosiY;
        const $standard_side_index: usize = posi_y_index!();
        let $idx_offset = voxelarray::get_pos_y_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaY;
        const $standard_side_index: usize = nega_y_index!();
        let $idx_offset = voxelarray::get_neg_y_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::PosiZ;
        const $standard_side_index: usize = posi_z_index!();
        let $idx_offset = voxelarray::get_pos_z_offset($idx, CHUNK_SIZE);
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaZ;
        const $standard_side_index: usize = nega_z_index!();
        let $idx_offset = voxelarray::get_neg_z_offset($idx, CHUNK_SIZE);
        $b
    }};
}

#[derive(Copy, Clone, Debug)]
pub struct SideRenderInfo {
    pub(super) side_pos: SidePos,
    pub tex: ArrayTextureIndex,
}

#[inline]
fn per_face_step(
    voxel_face: SideRenderInfo,
    vertex_buffer: &mut Vec<OutputVertex>,
) {
    let x = voxel_face.side_pos.get_x();
    let y = voxel_face.side_pos.get_y();
    let z = voxel_face.side_pos.get_z();
    voxel_side_indicies_unroll!(INDEX, {
        let mut temp_vert = get_face_verts(
            VoxelSide::from_id( voxel_face.side_pos.get_side_idx() )
        )[INDEX];
        temp_vert.position[0] += x;
        temp_vert.position[1] += y;
        temp_vert.position[2] += z;
        
        let mut packed_vert: PackedVertex = PackedVertex::from(temp_vert);
        packed_vert.set_tex_id(voxel_face.tex);

        if (INDEX == 2) || (INDEX == 3) {
            packed_vert.set_u_high();
            packed_vert.set_v_high();
        } else if (INDEX == 0) || (INDEX == 5) {
            packed_vert.set_u_low();
            packed_vert.set_v_low();
        } else if INDEX == 1 {
            packed_vert.set_u_high();
            packed_vert.set_v_low();
        } else if INDEX == 4 {
            packed_vert.set_u_low();
            packed_vert.set_v_high();
        }

        vertex_buffer.push(packed_vert);
    });
}

fn build_mesh<V: Voxel, A: ArtCache>(
    chunk: &Chunk<V>,
    art_cache: &A,
) -> Result<ChunkMesh, Box<dyn Error>> {
    let mut vertex_buffer: Vec<OutputVertex> = Vec::new();

    for i in 0..CHUNK_SIZE_CUBED {
        let tile = chunk.get_raw_i(i);
        if let Some(art) = art_cache.get_mapping(tile) {
            // Skip it if it's air.
            if art.tile_info.visible_this_pass {
                offset_unroll!(SIDE, offset_idx, i, SIDE_INDEX {
                    let mut cull: bool = false;
                    if let Some(neighbor_idx) = offset_idx {
                        let neighbor_tile = chunk.get_raw_i(neighbor_idx);
                        if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                            cull = neighbor_art.tile_info.visible_this_pass
                                && ( (art.tile_info.cull_self && (tile == neighbor_tile) )
                                || (art.tile_info.cull_others && (tile != neighbor_tile) ) );
                        }
                    }
                    if !cull {
                        let (x,y,z) = voxelarray::chunk_i_to_xyz(i, CHUNK_SIZE);
                        let sri = SideRenderInfo {
                            side_pos: SidePos::new(SIDE_INDEX as u8,
                                x as u8,
                                y as u8,
                                z as u8,
                            ),
                            tex : art.textures.data[SIDE_INDEX] };
                        per_face_step(sri, &mut vertex_buffer);
                    }
                });
            }
        }
    }

    Ok(ChunkMesh {
        verticies: vertex_buffer,
    })
}
