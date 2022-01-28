use std::{collections::HashMap, error::Error};

use glam::{Vec3, vec3, Vec2};
use hashbrown::HashSet;
use log::{warn, error};

use crate::{
    common::voxelmath::*, 
    world::{
        chunk::{Chunk, CHUNK_SIZE, ChunkData},
        voxelstorage::Voxel, TileId,
    }, resource::{ResourceId, image::{ID_MISSING_TEXTURE, ID_PENDING_TEXTURE}},
};
use crate::common::voxelmath::VoxelPos;

use super::{CubeArt, CubeTex, tiletextureatlas::{TileAtlasError, TileAtlasLayout}, CubeArtMapper};

use crate::world::chunk::{self as chunk, chunk_i_to_xyz, CHUNK_SIZE_CUBED};

#[derive(Copy, Clone)]
pub struct PackedVertex {
    vertexdata : u32,
}

#[derive(Copy, Clone)]
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
        Vec3::new( self.get_x() as f32, self.get_y() as f32, self.get_z() as f32 )
    }
}

//Bitmask
//V. U, tex, Z, Y, X
//b0_0_000000000000_000000_000000_000000
impl PackedVertex {
    pub fn set_x(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_000000_000000_111111;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_y(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_000000_111111_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val << 6;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_z(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_000000000000_111111_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val << 12;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_tex_id(&mut self, value : u32) {
        let bitmask : u32 = 0b0_0_111111111111_000000_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        let mut val = value;
        val = val << 18;
        val = val & bitmask;
        self.vertexdata = self.vertexdata | val; //Set our value
    }
    pub fn set_u_high(&mut self, value : bool) {
        let bitmask : u32 = 0b0_1_000000000000_000000_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        if value {
            self.vertexdata = self.vertexdata | bitmask;
        }
        //No need for an else case - if our value is low the bit will remain 0 from clearing it out.
    }
    pub fn set_v_high(&mut self, value : bool) {
        let bitmask : u32 = 0b1_0_000000000000_000000_000000_000000;
        self.vertexdata = self.vertexdata & (! bitmask); //clear out value
        if value {
            self.vertexdata = self.vertexdata | bitmask;
        }
    }
    
    pub fn get_x(&self) -> u32 {
        let bitmask : u32 = 0b0_0_000000000000_000000_000000_111111;
        let val = self.vertexdata & bitmask;
        return val;
    }
    pub fn get_y(&self) -> u32 {
        let bitmask : u32 = 0b0_0_000000000000_000000_111111_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 6;
        return val;
    }
    pub fn get_z(&self) -> u32 {
        let bitmask : u32 = 0b0_0_000000000000_111111_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 12;
        return val;
    }
    pub fn get_tex_id(&self) -> u32 {
        let bitmask : u32 = 0b0_0_111111111111_000000_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 18;
        return val;
    }
    pub fn get_u_high(&self) -> bool {
        let bitmask : u32 = 0b0_1_000000000000_000000_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 30;
        return val > 0;
    }
    pub fn get_v_high(&self) -> bool {
        let bitmask : u32 = 0b1_0_000000000000_000000_000000_000000;
        let mut val = self.vertexdata & bitmask;
        val = val >> 31;
        return val > 0;
    }
    
    pub fn from_vertex_uv(vert : Vertex, u : u32, v : u32) -> PackedVertex { 
        let mut ret = PackedVertex { vertexdata : 0,};
        ret.set_x(vert.position[0]);
        ret.set_y(vert.position[1]);
        ret.set_z(vert.position[2]);
        if u >= 1 {
            ret.set_u_high(true);
        }
        if v >= 1 {
            ret.set_v_high(true);
        }
        return ret;
    }

    pub fn to_bytes(self) -> [u8; 4] { 
        self.vertexdata.to_le_bytes()
    }
}

impl Into<Vec3> for Vertex {
    fn into(self) -> Vec3 {
        vec3(self.position[0] as f32, self.position[1] as f32, self.position[2] as f32)
    }
}

impl Into<Vec3> for PackedVertex {
    fn into(self) -> Vec3 {
        vec3(self.get_x() as f32, self.get_y() as f32, self.get_z() as f32)
    }
}

impl Into<PackedVertex> for Vertex {
    fn into(self) -> PackedVertex {
        let mut result = PackedVertex {
            vertexdata: 0,
        };
        result.set_x(self.get_x());
        result.set_y(self.get_y());
        result.set_z(self.get_z());
        result
    }
}

const POSX_POSY_POSZ_VERT : Vertex  = Vertex{ position : [1,1,1]};
const POSX_POSY_NEGZ_VERT : Vertex  = Vertex{ position : [1,1,0]};
const POSX_NEGY_NEGZ_VERT : Vertex  = Vertex{ position : [1,0,0]};
const POSX_NEGY_POSZ_VERT : Vertex  = Vertex{ position : [1,0,1]};
const NEGX_POSY_NEGZ_VERT : Vertex  = Vertex{ position : [0,1,0]};
const NEGX_POSY_POSZ_VERT : Vertex  = Vertex{ position : [0,1,1]};
const NEGX_NEGY_POSZ_VERT : Vertex  = Vertex{ position : [0,0,1]};
const NEGX_NEGY_NEGZ_VERT : Vertex  = Vertex{ position : [0,0,0]};

const POSITIVE_X_FACE : [Vertex; 6] = [
		POSX_POSY_NEGZ_VERT,
		POSX_POSY_POSZ_VERT,
		POSX_NEGY_POSZ_VERT,
		//-Second triangle:
		POSX_NEGY_POSZ_VERT,
		POSX_NEGY_NEGZ_VERT,
		POSX_POSY_NEGZ_VERT ];

const NEGATIVE_X_FACE : [Vertex; 6] = [
		//-First triangle:
		NEGX_POSY_POSZ_VERT,
		NEGX_POSY_NEGZ_VERT,
		NEGX_NEGY_NEGZ_VERT,
		//-Second triangle
		NEGX_NEGY_NEGZ_VERT,
		NEGX_NEGY_POSZ_VERT,
		NEGX_POSY_POSZ_VERT ];

const POSITIVE_Y_FACE : [Vertex; 6] = [
		//-First triangle:
		NEGX_POSY_NEGZ_VERT,
		NEGX_POSY_POSZ_VERT,
		POSX_POSY_POSZ_VERT,
		//-Second triangle
		POSX_POSY_POSZ_VERT,
		POSX_POSY_NEGZ_VERT,
		NEGX_POSY_NEGZ_VERT ];
		
const NEGATIVE_Y_FACE : [Vertex; 6] = [
		//-First triangle:
		POSX_NEGY_NEGZ_VERT,
		POSX_NEGY_POSZ_VERT,
		NEGX_NEGY_POSZ_VERT,
		//-Second triangle
		NEGX_NEGY_POSZ_VERT,
		NEGX_NEGY_NEGZ_VERT,
		POSX_NEGY_NEGZ_VERT ];

const POSITIVE_Z_FACE : [Vertex; 6] = [
		//-First triangle:
		POSX_POSY_POSZ_VERT,
		NEGX_POSY_POSZ_VERT,
		NEGX_NEGY_POSZ_VERT,
		//-Second triangle
		NEGX_NEGY_POSZ_VERT,
		POSX_NEGY_POSZ_VERT,
        POSX_POSY_POSZ_VERT ];

const NEGATIVE_Z_FACE : [Vertex; 6] = [
		//-First triangle:
		NEGX_POSY_NEGZ_VERT,
		POSX_POSY_NEGZ_VERT,
		POSX_NEGY_NEGZ_VERT,
		//-Second triangle
		POSX_NEGY_NEGZ_VERT,
		NEGX_NEGY_NEGZ_VERT,
		NEGX_POSY_NEGZ_VERT ];

fn get_face_verts(side: VoxelSide) -> [Vertex; 6] {
    match side {
        VoxelSide::PosiX => return POSITIVE_X_FACE,
        VoxelSide::NegaX => return NEGATIVE_X_FACE,
        VoxelSide::PosiY => return POSITIVE_Y_FACE,
        VoxelSide::NegaY => return NEGATIVE_Y_FACE,
        VoxelSide::PosiZ => return POSITIVE_Z_FACE,
        VoxelSide::NegaZ => return NEGATIVE_Z_FACE,
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

#[derive(Copy, Clone, Default, Debug, PartialEq)]
// We record the associated U,V values in this implementation (for the Texture Atlas)
pub struct UvCache {
    pub(crate) lower_u: f32, 
    pub(crate) lower_v: f32,
    pub(crate) higher_u: f32, 
    pub(crate) higher_v: f32,
}

type SidesCache = SidesArray<UvCache>;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CubeArtNotes { 
    pub visible : bool,
    pub cull_self : bool, //Do we cull the same material?
    pub cull_others : bool, //Do we cull materials other than this one?
}

impl Default for CubeArtNotes {
    fn default() -> Self {
        Self { visible: false, cull_self: false, cull_others: false, }
    }
}

impl From<&CubeArt> for CubeArtNotes {
    fn from(art: &CubeArt) -> Self {
        Self { 
            visible: art.is_visible(), 
            cull_self: art.cull_self,
            cull_others: art.cull_others,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Default)]
struct ArtCacheEntry {
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
            }
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

fn uv_cache_from_resource(resource: &ResourceId, layout: &mut TileAtlasLayout) -> Result<UvCache, TileAtlasError> { 
    let idx = layout.get_or_make_index_for_texture(resource)?;
    let lower_uv = layout.get_uv_for_index(idx, false, false);
    let higher_uv = layout.get_uv_for_index(idx, true, true);

    Ok( 
        UvCache { 
            lower_u: lower_uv.x,
            lower_v: lower_uv.y,
            higher_u: higher_uv.x,
            higher_v: higher_uv.y,
        }
    )
}

fn sides_cache_from_art(art: &CubeArt, layout: &mut TileAtlasLayout) -> Result<Option<SidesCache>, TileAtlasError> { 
    match &art.textures {
        CubeTex::Invisible => Ok(None),
        CubeTex::Single(t) => {
            Ok(Some(
                SidesArray::new_uniform( &uv_cache_from_resource(t, layout)?)
            ))
        },
        CubeTex::AllSides(a) => {
            let mut result_array: [UvCache; 6] = [UvCache::default(); 6];
            for i in 0..6 { 
                result_array[i] = uv_cache_from_resource(a.get_i(i), layout)?;
            }
            Ok(Some(
                SidesCache { 
                    data: result_array,
                }
            ))
        },
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

trait ArtCache {
    /// Return "None" if invisible block.
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry>;
    /// Should we draw anything for this chunk at all?
    fn is_any_visible(&self) -> bool;
}

struct ArtCacheUniform {
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

struct ArtCacheSmall{
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
        self.data.get(idx as usize)
            .and_then(
            |a| {
                if !a.tile_info.visible { 
                    None
                }
                else {
                    Some(a)
                }
            }
        )
    }
    fn is_any_visible(&self) -> bool {
        true
    }
}


struct ArtCacheLarge{
    data: HashMap<u16, ArtCacheEntry>
}

impl ArtCache for ArtCacheLarge {
    fn get_mapping(&self, idx: u16) -> Option<&ArtCacheEntry> {
        self.data.get(&idx)
            .and_then(
            |a| {
                if !a.tile_info.visible { 
                    None
                }
                else {
                    Some(a)
                }
            }
        )
    }
    fn is_any_visible(&self) -> bool {
        true
    }
}

impl ArtCacheLarge { 
    pub fn new(palette: HashMap<u16, ArtCacheEntry>) -> Self { 
        Self { 
            data: palette
        }
    }
}

pub type CollectCullOutput = Vec<SideRenderInfo>;
#[derive(Default, Debug, Clone)]
pub struct MeshStepOutput { 
    pub verticies: Vec<Vec3>, 
    pub uv: Vec<Vec2>,
}

trait ArtCacheHolder {
    fn collect_cull_step(&self, chunk : &Chunk<TileId>) -> Result<CollectCullOutput, Box<dyn Error>>;
}

struct UniformChunkMesherState { 
    art_cache: ArtCacheUniform
}
impl ArtCacheHolder for UniformChunkMesherState {
    fn collect_cull_step(&self, chunk : &Chunk<TileId>) -> Result<CollectCullOutput, Box<dyn Error>> {
        collect_cull(chunk, &self.art_cache)
    }
}
struct SmallChunkMesherState { 
    art_cache: ArtCacheSmall
}
impl ArtCacheHolder for SmallChunkMesherState {
    fn collect_cull_step(&self, chunk : &Chunk<TileId>) -> Result<CollectCullOutput, Box<dyn Error>> {
        collect_cull(chunk, &self.art_cache)
    }
}

struct LargeChunkMesherState { 
    art_cache: ArtCacheLarge
}
impl ArtCacheHolder for LargeChunkMesherState {
    fn collect_cull_step(&self, chunk : &Chunk<TileId>) -> Result<CollectCullOutput, Box<dyn Error>> {
        collect_cull(chunk, &self.art_cache)
    }
}

pub struct MesherState {
    /// The type signatures of MesherState, MesherStateInner, etc is so weird 
    /// because it is intended to produce a situation where - there is a vtable
    /// lookup when you call collect_cull_step(), but no vtable step between
    /// anything INSIDE of collect_cull() and art_cache.get_mapping().
    inner: Box<dyn ArtCacheHolder>,

    pub textures_needed: HashSet<ResourceId>,
}

impl ArtCacheHolder for MesherState {
    fn collect_cull_step(&self, chunk : &Chunk<TileId>) -> Result<CollectCullOutput, Box<dyn Error>> {
        self.inner.collect_cull_step(chunk)
    }
}

impl MesherState {
    pub fn prepare_to_mesh<A:CubeArtMapper<TileId>>(chunk: &Chunk<TileId>, tiles_to_art: &A, atlas: &mut TileAtlasLayout) -> Result<Self, Box<dyn std::error::Error>> {
        let (inner, mut textures_needed): (Box<dyn ArtCacheHolder>, HashSet<ResourceId>) = match &chunk.data {
            ChunkData::Uniform(val) => { 
                let missing_texture = art_cache_missing_texture(atlas);
                let mut textures_needed = HashSet::new();
                let cube_art = match tiles_to_art.get_art_for_tile(val) { 
                    Some(art) => {
                        for t in art.all_textures() { 
                            textures_needed.insert(t.clone());
                        }
                        ArtCacheEntry::new(art, atlas)
                    },
                    None => {
                        warn!("No art loaded for world-level tile ID {}. Using missing texture.", val);
                        missing_texture.clone()
                    }
                };
                let art_cache = ArtCacheUniform::new(cube_art, missing_texture);

                (Box::new(UniformChunkMesherState {
                    art_cache,
                }), textures_needed)
            },
            ChunkData::Small(chunk_inner) => {
                let missing_texture = art_cache_missing_texture(atlas);
                let mut textures_needed = HashSet::new();
                let mut art_palette: [ArtCacheEntry; 256] = [ArtCacheEntry::default(); 256];
                //Iterate through the palette
                for i in 0..(chunk_inner.highest_idx+1) { 
                    let tile = chunk_inner.palette[i as usize];
                    let cube_art = match tiles_to_art.get_art_for_tile(&tile) { 
                        Some(art) => {
                            for t in art.all_textures() { 
                                textures_needed.insert(t.clone());
                            }
                            ArtCacheEntry::new(art, atlas)
                        },
                        None => {
                            warn!("No art loaded for world-level tile ID {}. Using missing texture.", tile);
                            missing_texture.clone()
                        }
                    };
                    art_palette[i as usize] = cube_art;
                }
                
                let art_cache = ArtCacheSmall::new(art_palette, missing_texture);
                (Box::new(SmallChunkMesherState {
                    art_cache,
                }), textures_needed)
            },
            ChunkData::Large(chunk_inner) => {
                let missing_texture = art_cache_missing_texture(atlas);
                let mut textures_needed = HashSet::new();
                let mut art_palette: HashMap<u16, ArtCacheEntry> = HashMap::with_capacity(chunk_inner.palette.len());
                //Iterate through the palette
                for (idx, tile) in chunk_inner.palette.iter() { 
                    
                    let cube_art = match tiles_to_art.get_art_for_tile(tile) { 
                        Some(art) => {
                            for t in art.all_textures() { 
                                textures_needed.insert(t.clone());
                            }
                            ArtCacheEntry::new(art, atlas)
                        },
                        None => {
                            warn!("No art loaded for world-level tile ID {}. Using missing texture.", tile);
                            missing_texture.clone()
                        }
                    };
                    art_palette.insert(*idx, cube_art);
                }
                
                let art_cache = ArtCacheLarge::new(art_palette);
                (Box::new(LargeChunkMesherState {
                    art_cache,
                }), textures_needed)
            },
        };
        
        // Make sure we're not telling the system to download the missing texture thing
        textures_needed.remove(&ID_MISSING_TEXTURE);
        textures_needed.remove(&ID_PENDING_TEXTURE);

        Ok(Self {
            inner,
            textures_needed,
        })
    }
    pub fn mesh_step(drawable: CollectCullOutput) -> MeshStepOutput { 
        mesh_step(drawable)
    }
}

/// Make a mesh in one single blocking action (does not permit you to share one tile atlas between chunks)
pub fn make_mesh<A:CubeArtMapper<TileId>>(texture_size: u32, chunk: &Chunk<TileId>, tiles_to_art: &A) -> Result<(MeshStepOutput, TileAtlasLayout), Box<dyn std::error::Error>> {
    let mut atlas = TileAtlasLayout::new(texture_size, 32, 8, None);

    let state = MesherState::prepare_to_mesh(chunk, tiles_to_art, &mut atlas)?;
    let drawable = state.collect_cull_step(chunk)?;

    println!("{:?}", state.textures_needed);
    Ok((mesh_step(drawable),atlas))
}

macro_rules! offset_unroll {
    ($side:ident, $idx_offset:ident, $idx:ident, $standard_side_index:ident $b:block) => { 
        {
            const $side : VoxelSide = VoxelSide::PosiX;
            const $standard_side_index : usize = posi_x_index!();
            let $idx_offset = chunk::get_pos_x_offset($idx, CHUNK_SIZE);
            $b
        }
        {
            const $side : VoxelSide = VoxelSide::NegaX;
            const $standard_side_index : usize = nega_x_index!();
            let $idx_offset = chunk::get_neg_x_offset($idx, CHUNK_SIZE);
            $b
        }
        {
            const $side : VoxelSide = VoxelSide::PosiY;
            const $standard_side_index : usize = posi_y_index!();
            let $idx_offset = chunk::get_pos_y_offset($idx, CHUNK_SIZE);
            $b
        }
        {
            const $side : VoxelSide = VoxelSide::NegaY;
            const $standard_side_index : usize = nega_y_index!();
            let $idx_offset = chunk::get_neg_y_offset($idx, CHUNK_SIZE);
            $b
        }
        {
            const $side : VoxelSide = VoxelSide::PosiZ;
            const $standard_side_index : usize = posi_z_index!();
            let $idx_offset = chunk::get_pos_z_offset($idx, CHUNK_SIZE);
            $b
        }
        {
            const $side : VoxelSide = VoxelSide::NegaZ;
            const $standard_side_index : usize = nega_z_index!();
            let $idx_offset = chunk::get_neg_z_offset($idx, CHUNK_SIZE);
            $b
        }
    };
}

#[derive(Copy, Clone, Debug)]
pub struct SideRenderInfo {
    pub side : VoxelSide,
    pub cell: VoxelPos<u16>,
    pub uv : UvCache,
}

fn collect_cull<V: Voxel, A: ArtCache>(chunk : &Chunk<V>, art_cache : &A)
                            -> Result<CollectCullOutput, Box<dyn Error>> {
    let mut drawable : Vec<SideRenderInfo> = Vec::new();

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
                        drawable.push(sri);
                    }
                });
            }
        }
    }
    
    Ok(drawable)
}

pub fn mesh_step(drawable : CollectCullOutput) -> MeshStepOutput {
    let mut vertex_buffer : Vec<Vec3> = Vec::new();
    let mut uv_buffer : Vec<Vec2> = Vec::new();
    for voxel_face in drawable.iter() {
        for vert_iter in 0..6 {
            let x = voxel_face.cell.x;
            let y = voxel_face.cell.y;
            let z = voxel_face.cell.z;
            let mut temp_vert = get_face_verts(voxel_face.side)[vert_iter];
            temp_vert.position[0] += x as u32;
            temp_vert.position[1] += y as u32;
            temp_vert.position[2] += z as u32;
            let mut u : f32 = 0.0;
            let mut v : f32 = 0.0;

            //Do our UV the hacky way.
            if (vert_iter == 2) || (vert_iter == 3) {
                u = voxel_face.uv.higher_u;
                v = voxel_face.uv.higher_v;
            }
            else if (vert_iter == 0) || (vert_iter == 5) {
                u = voxel_face.uv.lower_u;
                v = voxel_face.uv.lower_v;
            }
            else if vert_iter == 1 {
                u = voxel_face.uv.higher_u;
                v = voxel_face.uv.lower_v;
            }
            else if vert_iter == 4 {
                u = voxel_face.uv.lower_u;
                v = voxel_face.uv.higher_v;
            }

            uv_buffer.push(Vec2::new(u, v));
            vertex_buffer.push(temp_vert.to_rend3_vertex());
        }
    }
    MeshStepOutput { verticies: vertex_buffer, uv: uv_buffer }
}