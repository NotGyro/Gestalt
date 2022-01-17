use std::{collections::HashMap, error::Error};

use glam::{Vec3, vec3};

use crate::{
    common::voxelmath::*, 
    world::{
        chunk::{Chunk, CHUNK_SIZE, ChunkData},
        voxelstorage::Voxel,
    },
    resource::{ResourceIdOrMeta},
};
use crate::common::voxelmath::VoxelPos;

use super::{SimpleTileArt, BlockTex, TextureId};

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

#[derive(Copy, Clone, Debug)]
struct SideRenderInfo {
    pub side : VoxelSide,
    pub cell: VoxelPos<u16>,
    pub tex_idx : u32,
}

type SidesTextures = SidesArray<usize>;
type ArtEntry = (SidesTextures, SimpleTileArt);

#[derive(thiserror::Error, Debug, Clone)]
pub enum TextureLookupError {
    #[error("Texture ID has not been loaded or is not valid: {0:?}")]
    UnrecognizedTexture(ResourceIdOrMeta),
    #[error("Tried to look up a texture but no texture mapping found for tile ID {0}")]
    UnrecognizedTile(String),
    #[error("Tried to associate tile {} with texture {0:?}, but that texture (which should have been loaded into the renderer already) has not been loaded in to the renderer.")]
    FileNotLoaded(String, ResourceIdOrMeta),
}

pub struct TextureArrayDyn<V: Voxel> { 
    //The key here is "texture name," so here we've got a texture name to Texture Array layer mapping.
    tex_mapping : HashMap<TextureId, usize>,
    //Tile to idx&art
    pub tile_mapping : HashMap<V, ArtEntry>,
    //Cached image data, indexable by tex_mapping's values
    tex_data : Vec<Vec<u8>>,
    pub textures : Option<()>,
    pub tex_width : u32, 
    pub tex_height : u32, 
    pub max_tex : usize,
}
impl<V: Voxel> TextureArrayDyn<V> {
    pub fn new(twidth : u32, theight : u32, tmax : usize) -> Self { 
        TextureArrayDyn {
            tex_mapping : HashMap::default(),
            tile_mapping : HashMap::default(),
            tex_data : Vec::new(),
            max_tex : tmax,
            tex_width : twidth,
            tex_height : theight,
            textures : None,
        }
    }
    pub fn has_tex(&self, texture_id: &TextureId) -> bool { self.tex_mapping.contains_key(texture_id) }
    pub fn index_for_tex(&self, texture_id: &TextureId) -> Result<usize, TextureLookupError> {
        Ok(
            *self.tex_mapping
                .get(texture_id)
                .ok_or(
                    TextureLookupError::UnrecognizedTexture( resource_debug!( texture_id.clone() ) )
                )?
        ) 
    }
    pub fn art_for_tile(&self, tile: &V) -> Result<&ArtEntry, TextureLookupError> { 
        Ok(
            self.tile_mapping.get(tile).ok_or(
                TextureLookupError::UnrecognizedTile(format!("{:?}", tile))
            )?      
        )
    }
    pub fn has_tile(&self, tile: &V) -> bool { self.tile_mapping.contains_key(tile) }

    pub fn add_tex(&mut self, texture_id: &TextureId) -> Result<(), TextureLookupError> {
        /*
        TODO
        let idx = self.tex_data.len();
        self.tex_mapping.insert(texture_id.clone(), idx);
        let image = Self::ld_image(texname, self.tex_width, self.tex_height)?; 
        self.tex_data.push(image);
        assert!(self.tex_data.len() < self.max_tex);*/
        Ok(())
    }
    
    pub fn ld_image(path: String, size_x : u32, size_y : u32) -> Result<Vec<u8>, Box<dyn Error>>  {        
        /*
        let path = Path::new(path_name.as_str());

        let image = image::open(path)?.to_rgba();
        let image_dimensions = image.dimensions();
        assert_eq!(image_dimensions, (size_x, size_y));
        
        info!("Loaded texture file: {}", path_name.clone());*/

        Ok(Vec::default())
    }
    
    pub fn associate_tile(&mut self, tile: V, art: SimpleTileArt) -> Result<(), Box<dyn std::error::Error>> {
        match &art.textures {
            BlockTex::Invisible => {
                
            },
            BlockTex::Single(texture_name) => {
                self.add_tex(&texture_name)?;
                let tex = self.tex_mapping.get(&texture_name);
                let tex_unwrap = tex.ok_or(TextureLookupError::FileNotLoaded(format!("{:?}", tile), resource_debug!(&texture_name) ) )?;
                let sides_textures= SidesArray::new_uniform(tex_unwrap);
                self.tile_mapping.insert(tile, (sides_textures, art));
                //self.rebuild(display);
            },
            BlockTex::AllSides(sides) => {
                let mut sides_textures= SidesArray::new_uniform(&0);
                for (idx, side) in sides.data.iter().enumerate() {
                    self.add_tex(side)?;
                    let tex = self.tex_mapping.get(&side);
                    let tex_unwrap = tex.ok_or(TextureLookupError::FileNotLoaded(format!("{:?}", tile), resource_debug!(&side) ) )?;

                    sides_textures.set_i(*tex_unwrap, idx);
                }
                self.tile_mapping.insert(tile, (sides_textures, art));
                //self.rebuild(display);
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
enum ArtCacheInner {
    Uniform( ArtEntry ),
    Small( Vec<ArtEntry> ),
    Large( HashMap<u16, ArtEntry>),
}
#[derive(Clone)]
struct ArtCache(Option<ArtCacheInner>);

impl ArtCache {
    pub fn build_from<V:Voxel>(chunk: &Chunk<V>, tex: &TextureArrayDyn<V>) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(match &chunk.data {
            ChunkData::Uniform(val) => {
                if tex.has_tile(val) {
                    let art = tex.art_for_tile(val)?;
                    ArtCache{0: Some(
                        ArtCacheInner::Uniform( art.clone() )
                    )}
                }
                else {
                    ArtCache{0: None}
                }
            }
            ChunkData::Small(inner) => {
                let mut list: Vec<ArtEntry> = Vec::with_capacity(256);
                for i in 0..inner.palette.len() {
                    let value = &inner.palette[i];
                    let art = tex.art_for_tile(value)?;
                    list.push(art.clone());
                }
                if list.len() != 0 {
                    ArtCache{0: Some(
                        ArtCacheInner::Small(list)
                    )}
                }
                else {
                    ArtCache{0: None}
                }
            }
            ChunkData::Large(inner) => {
                let mut list: HashMap<u16, ArtEntry> = HashMap::new();
                for (key, value) in inner.palette.iter() {
                    let art = tex.art_for_tile(value)?;
                    list.insert(*key, art.clone());
                }
                if list.len() != 0 {
                    ArtCache{0: Some(
                        ArtCacheInner::Large(list)
                    )}
                }
                else {
                    ArtCache{0: None}
                }
            }
        })
    }
    #[inline(always)]
    pub fn get_mapping(&self, idx: u16) -> Option<ArtEntry> {
        if let Some(inner) = &self.0 {
            match inner {
                ArtCacheInner::Uniform(single) => {
                    if idx == 0 {
                        Some(single.clone())
                    }            
                    else { 
                        None
                    }
                } 
                ArtCacheInner::Small(list) => { 
                    if idx >= list.len() as u16 {
                        None
                    } 
                    else {
                        Some(list[idx as usize].clone())
                    }
                }
                ArtCacheInner::Large(list)  => {
                    list.get(&idx).map(|e| e.clone())
                }
            }
        }
        else {
            None
        }
    }
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

pub fn make_voxel_mesh<V: Voxel>(chunk : &Chunk<V>, textures : &TextureArrayDyn<V>)
                            -> Result<Vec<u8>, Box<dyn Error>> {
    let art_cache = ArtCache::build_from(chunk, &textures)?;
    let mut drawable : Vec<SideRenderInfo> = Vec::new();

    for i in 0..CHUNK_SIZE_CUBED {
        let tile = chunk.get_raw_i(i);
        if let Some(art) = art_cache.get_mapping(tile) {
            // Skip it if it's air.
            if art.1.is_visible() {
                offset_unroll!(SIDE, offset_idx, i, SIDE_INDEX {
                    let mut cull: bool = false;
                    if let Some(neighbor_idx) = offset_idx {
                        let neighbor_tile = chunk.get_raw_i(neighbor_idx);
                        if let Some(neighbor_art) = art_cache.get_mapping(neighbor_tile) {
                            cull = neighbor_art.1.is_visible()
                        }
                    }
                    if !cull {
                        let (x,y,z) = chunk_i_to_xyz(i, CHUNK_SIZE);
                        let vri = SideRenderInfo {
                            side : SIDE,
                            cell: vpos!(x as u16, y as u16, z as u16),
                            tex_idx : art.0.data[SIDE_INDEX] as u32 };
                        drawable.push(vri);
                    }
                });
            }
        }
    }
    //println!("Found {} drawable cubes.", drawable.len());
    Ok(mesh_step(drawable))
}

#[inline(always)]
fn mesh_step(drawable : Vec<SideRenderInfo>) -> Vec<u8> {
    //TODO: The stuff which will make only certain sides even try to render, depending on the player's current angle.
    let mut localbuffer : Vec<u8> = Vec::new();
    for voxel in drawable.iter() {
        for vert_iter in 0..6 {
            let x = voxel.cell.x;
            let y = voxel.cell.y;
            let z = voxel.cell.z;
            let mut temp_vert = get_face_verts(voxel.side)[vert_iter];
            temp_vert.position[0] += x as u32;
            temp_vert.position[1] += y as u32;
            temp_vert.position[2] += z as u32;
            let mut u : u32 = 0;
            let mut v : u32 = 0;

            //Do our UV the hacky way.
            if (vert_iter == 2) || (vert_iter == 3) {
                u = 0;
                v = 1;
            }
            else if (vert_iter == 0) || (vert_iter == 5) {
                u = 1;
                v = 0;
            }
            else if vert_iter == 1 {
                u = 0;
                v = 0;
            }
            else if vert_iter == 4 {
                u = 1;
                v = 1;
            }
            let mut pv = PackedVertex::from_vertex_uv(temp_vert, u, v);
            pv.set_tex_id(voxel.tex_idx);
            let bytes = pv.to_bytes();
            localbuffer.extend_from_slice(&bytes);
        }
    }
    localbuffer
}