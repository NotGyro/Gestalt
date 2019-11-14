use serde::Deserialize;
use hashbrown::HashMap;
use std::boxed::Box;
use std::result::Result;
use std::error::Error;
use std::path::Path;

use std::sync::Arc;
use parking_lot::RwLock;

use vek::{Mat4, Vec3};
use image::RgbaImage;

use crate::voxel::voxelmath::*;
use crate::voxel::subdivstorage::*;
use crate::voxel::subdivmath::*;
use crate::util::config::ConfigString;
use crate::world::tile::TileID;
use super::render_traits::*;


// -------------------------------- Tile Art  --------------------------------

#[allow(dead_code)] pub type RenderPassID = u8;
#[allow(dead_code)] pub type Pixel = rgb::RGBA8;

/* macro_rules! top_axis {
    () => { VoxelAxis::PosiY }
}
macro_rules! bottom_axis {
    () => { VoxelAxis::NegaY }
}
macro_rules! front_axis {
    () => { VoxelAxis::PosiZ }
}
macro_rules! back_axis {
    () => { VoxelAxis::NegaZ }
}
macro_rules! right_axis {
    () => { VoxelAxis::PosiX }
}
macro_rules! left_axis {
    () => { VoxelAxis::NegaX }
}*/

macro_rules! top_index {
    () => {  posi_y_index!() }
}
macro_rules! bottom_index {
    () => { nega_y_index!() }
}
macro_rules! front_index {
    () => { posi_z_index!() }
}
macro_rules! back_index {
    () => { nega_z_index!() }
}
macro_rules! right_index {
    () => { posi_x_index!() }
}
macro_rules! left_index {
    () => { nega_x_index!() }
}

#[derive(Clone, Deserialize)]
pub struct CubeTextureConfig { 
    pub top: String,
    pub bottom: String,
    pub front: String, 
    pub back: String, 
    pub right: String,
    pub left: String,
}

#[derive(Clone, Deserialize)]
pub struct Color {
    pub r : u8,
    pub g : u8,
    pub b : u8,
}
impl From<rgb::RGB8> for Color {
    #[inline(always)]
    fn from(color: rgb::RGB8) -> Self {
        Color {
            r: color.r,
            g: color.g,
            b: color.b,
        }
    }
}

impl Into<rgb::RGB8> for Color {
    #[inline(always)]
    fn into(self) -> rgb::RGB8 {
        rgb::RGB8 {
            r: self.r,
            g: self.g, 
            b: self.b,
        }
    }
}
impl Into<rgb::RGBA8> for Color {
    #[inline(always)]
    fn into(self) -> rgb::RGBA8 {
        rgb::RGBA8 {
            r: self.r,
            g: self.g, 
            b: self.b,
            a: 255,
        }
    }
}

#[derive(Clone, Deserialize)]
enum ArtBlockConfig {
    SingleTexture(String),
    SixTextures(CubeTextureConfig),
    SolidColor(Color),
}
#[derive(Clone)]
struct ArtTexturedBlock {
    pub textures: [usize; 6],
}

enum ArtBlock {
    Textured(ArtTexturedBlock), 
    SolidColor(Color),
}

/*
lazy_static! {
    static ref TILE_TO_ART: RwLock<HashMap<TileID, ArtTexturedBlock>> = RwLock::new(HashMap::new());
}*/

struct TextureManager { 
    //The key here is "texture name," so here we've got a texture name to texture ID mapping.
    pub tex_mapping : HashMap<String, usize>,
    pub textures : Vec<Arc<RwLock<RgbaImage>>>,
}
impl TextureManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        TextureManager {
            tex_mapping : HashMap::new(),
            textures : Vec::new(),
        }
    }
    #[inline(always)]
    pub fn index_for_tex(&self, name : &String) -> Option<&usize> { self.tex_mapping.get(name) }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn get_tex(&self, idx: usize) -> Option<Arc<RwLock<RgbaImage>>> { self.textures.get(idx).map(#[inline(always)] |arc| {arc.clone()}) }

    pub fn process_art(&mut self, art: ArtBlockConfig) -> Result<ArtBlock, Box<dyn Error>> {
        match art {
            ArtBlockConfig::SingleTexture(filename) => {
                let mut result = ArtTexturedBlock{ textures: [0,0,0,0,0,0], };
                let id = self.load_or_add_image(&filename)?;
                for i in 0..6 {
                    result.textures[i] = id;
                }
                Ok(ArtBlock::Textured(result))
            },
            ArtBlockConfig::SixTextures(cube) => {
                let mut result = ArtTexturedBlock{ textures: [0,0,0,0,0,0], };

                result.textures[top_index!()] = self.load_or_add_image(&cube.top)?;
                result.textures[bottom_index!()] = self.load_or_add_image(&cube.bottom)?;
                result.textures[front_index!()] = self.load_or_add_image(&cube.front)?;
                result.textures[back_index!()] = self.load_or_add_image(&cube.back)?;
                result.textures[right_index!()] = self.load_or_add_image(&cube.right)?;
                result.textures[left_index!()] = self.load_or_add_image(&cube.left)?;

                Ok(ArtBlock::Textured(result))
            },
            ArtBlockConfig::SolidColor(color) => Ok(ArtBlock::SolidColor(color)),
        }
    }

    fn load_or_add_image(&mut self, path_file : &String) -> Result<usize, Box<dyn Error>> {
        match self.index_for_tex(path_file) {
            Some(id) => Ok(*id),
            None => { 
                let image = Self::ld_image(path_file)?;
                let id = self.textures.len();
                self.textures.push(Arc::new(RwLock::new(image)));
                self.tex_mapping.insert(path_file.clone(), id);
                Ok(id)
            },
        }
    }
    
    fn ld_image(path_file : &String) -> Result<RgbaImage, Box<dyn Error>> {        
        let path = Path::new(path_file.as_str());

        let image = image::open(path)?.to_rgba();
        info!("Loaded texture: {}", path_file.clone());
        return Ok(image);
    }
}

// -------------------------------- Vertex types --------------------------------

type TileVertexPos = Vec3<f32>;

#[allow(dead_code)] const ONE : f32 = 1.0;
#[allow(dead_code)] const ZER : f32 = 0.0;

#[allow(dead_code)] const POSX_POSY_POSZ_VERT : TileVertexPos  = TileVertexPos{ x: ONE, y: ONE, z: ONE };
#[allow(dead_code)] const POSX_POSY_NEGZ_VERT : TileVertexPos  = TileVertexPos{ x: ONE, y: ONE, z: ZER };
#[allow(dead_code)] const POSX_NEGY_NEGZ_VERT : TileVertexPos  = TileVertexPos{ x: ONE, y: ZER, z: ZER };
#[allow(dead_code)] const POSX_NEGY_POSZ_VERT : TileVertexPos  = TileVertexPos{ x: ONE, y: ZER, z: ZER };
#[allow(dead_code)] const NEGX_POSY_NEGZ_VERT : TileVertexPos  = TileVertexPos{ x: ONE, y: ZER, z: ONE };
#[allow(dead_code)] const NEGX_POSY_POSZ_VERT : TileVertexPos  = TileVertexPos{ x: ZER, y: ONE, z: ONE };
#[allow(dead_code)] const NEGX_NEGY_POSZ_VERT : TileVertexPos  = TileVertexPos{ x: ZER, y: ZER, z: ONE };
#[allow(dead_code)] const NEGX_NEGY_NEGZ_VERT : TileVertexPos  = TileVertexPos{ x: ZER, y: ZER, z: ZER };

#[allow(dead_code)]
const POSITIVE_X_FACE : [TileVertexPos; 6] = [
        POSX_POSY_NEGZ_VERT,
        POSX_POSY_POSZ_VERT,
        POSX_NEGY_POSZ_VERT,
        //-Second triangle:
        POSX_NEGY_POSZ_VERT,
        POSX_NEGY_NEGZ_VERT,
        POSX_POSY_NEGZ_VERT ];

#[allow(dead_code)]
const NEGATIVE_X_FACE : [TileVertexPos; 6] = [
        //-First triangle:
        NEGX_POSY_POSZ_VERT,
        NEGX_POSY_NEGZ_VERT,
        NEGX_NEGY_NEGZ_VERT,
        //-Second triangle
        NEGX_NEGY_NEGZ_VERT,
        NEGX_NEGY_POSZ_VERT,
        NEGX_POSY_POSZ_VERT ];

#[allow(dead_code)]
const POSITIVE_Y_FACE : [TileVertexPos; 6] = [
        //-First triangle:
        NEGX_POSY_NEGZ_VERT,
        NEGX_POSY_POSZ_VERT,
        POSX_POSY_POSZ_VERT,
        //-Second triangle
        POSX_POSY_POSZ_VERT,
        POSX_POSY_NEGZ_VERT,
        NEGX_POSY_NEGZ_VERT ];

#[allow(dead_code)]
const NEGATIVE_Y_FACE : [TileVertexPos; 6] = [
        //-First triangle:
        POSX_NEGY_NEGZ_VERT,
        POSX_NEGY_POSZ_VERT,
        NEGX_NEGY_POSZ_VERT,
        //-Second triangle
        NEGX_NEGY_POSZ_VERT,
        NEGX_NEGY_NEGZ_VERT,
        POSX_NEGY_NEGZ_VERT ];

#[allow(dead_code)]
const POSITIVE_Z_FACE : [TileVertexPos; 6] = [
        //-First triangle:
        POSX_POSY_POSZ_VERT,
        NEGX_POSY_POSZ_VERT,
        NEGX_NEGY_POSZ_VERT,
        //-Second triangle
        NEGX_NEGY_POSZ_VERT,
        POSX_NEGY_POSZ_VERT,
        POSX_POSY_POSZ_VERT ];

#[allow(dead_code)]
const NEGATIVE_Z_FACE : [TileVertexPos; 6] = [
        //-First triangle:
        NEGX_POSY_NEGZ_VERT,
        POSX_POSY_NEGZ_VERT,
        POSX_NEGY_NEGZ_VERT,
        //-Second triangle
        POSX_NEGY_NEGZ_VERT,
        NEGX_NEGY_NEGZ_VERT,
        NEGX_POSY_NEGZ_VERT ];

#[allow(dead_code)]
const FULL_CUBE : [[TileVertexPos; 6]; 6] = [
    POSITIVE_X_FACE,
    NEGATIVE_X_FACE,
    POSITIVE_Y_FACE,
    NEGATIVE_Y_FACE,
    POSITIVE_Z_FACE,
    NEGATIVE_Z_FACE
];

#[allow(dead_code)]
fn get_face_verts(side: VoxelAxis) -> [TileVertexPos; 6] {
    match side {
        VoxelAxis::PosiX => return POSITIVE_X_FACE,
        VoxelAxis::NegaX => return NEGATIVE_X_FACE,
        VoxelAxis::PosiY => return POSITIVE_Y_FACE,
        VoxelAxis::NegaY => return NEGATIVE_Y_FACE,
        VoxelAxis::PosiZ => return POSITIVE_Z_FACE,
        VoxelAxis::NegaZ => return NEGATIVE_Z_FACE,
    }
}


// -------------------------------- Meshing / rendering code begins here --------------------------------


struct TileVertex { 
    pub position: TileVertexPos,
    pub art_index: usize,
    pub uv: [f32; 2],
}

/// Each of these represents just one level of detail.
/// This, of course, means more draw calls,
/// but it also means you can shape which chunks have
/// which levels of detail around the player as they move
/// through the world, without needing to rebuild any meshes to do so.
#[allow(dead_code)]
struct ChunkCubesMesh {
    data: Vec<TileVertex>,
}

impl ChunkCubesMesh {
    #[allow(dead_code)]
    pub fn new() -> Self {
        ChunkCubesMesh {
            data: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.data.clear();
    }

    #[allow(dead_code)]
    pub fn rebuild<T> (&mut self, 
                    _art_mapping: &HashMap<TileID, ArtTexturedBlock>, _texture_manager: &TextureManager,
                    _chunk: &T,
                    _source_range: VoxelRange<i32>, _scale: Scale)
                            -> Result<(), Box<dyn Error>>
                            where T : OctreeSource<TileID, (), i32> {
        Ok(())
    }
}

// -------------------------------- Renderer proper --------------------------------
#[allow(dead_code)]
pub struct EucRenderer {
    texture_manager: TextureManager,
    art_mapping: HashMap<TileID, ArtBlock>,
    // TODO: Bigger world.
    mesh : ChunkCubesMesh,
    /// The largest / lowest detail blocks we are set up for. The range is [lod_coarse, lod_fine]
    pub lod_coarse : Scale,
    /// The smallest / highest detail blocks we are set up for. The range is [lod_coarse, lod_fine]
    pub lod_fine : Scale,
    // Model matrix is calculated on the fly per chunk, and scaled to match our level of detail.
    pub projection_matrix: Mat4<f32>,
    pub view_matrix: Mat4<f32>,
}
impl EucRenderer {
    #[allow(dead_code)]
    pub fn new(lod_coarse: Scale, lod_fine : Scale, projection_matrix: &Mat4<f32>,) -> Self {
        EucRenderer {
            texture_manager : TextureManager::new(),
            art_mapping: HashMap::new(),
            mesh: ChunkCubesMesh::new(),
            lod_coarse: lod_coarse,
            lod_fine: lod_fine,
            projection_matrix: projection_matrix.clone(),
            //The mainloop which constructs this EucRenderer should be updating this to reflect the player's position.
            view_matrix: Mat4::zero(),
        }
    }
    
    #[inline(always)]
    #[allow(dead_code)]
    pub fn update_view_matrix(&mut self, player_matrix: &Mat4<f32>) { 
        self.view_matrix = player_matrix.clone();
    }

    #[allow(dead_code)]
    pub fn update_projection_matrix(&mut self, projection: &Mat4<f32>) { 
        self.projection_matrix = projection.clone();
    }
}
impl OctreeRenderer for EucRenderer {
	fn reg_tile_art(&mut self, tile : TileID, art : ConfigString) -> Result<(), Box<dyn Error>> {
        let config : ArtBlockConfig = art.deserialize()?;
        let processed_art = self.texture_manager.process_art(config)?;
        self.art_mapping.insert(tile, processed_art);
        Ok(())
    }
}