use hashbrown::HashMap;
use image::{RgbaImage, Rgba};

use crate::common::voxelmath::SidesArray;
use crate::resource::ResourceId;
use crate::world::voxelstorage::Voxel;

pub mod voxelmesher;
pub mod tiletextureatlas;

type TextureId = ResourceId;

pub trait CubeArtMapper<V> where V: Voxel { 
    fn get_art_for_tile(&self, tile: &V) -> Option<&CubeArt>;
}

impl<V> CubeArtMapper<V> for HashMap<V, CubeArt> where V: Voxel {
    fn get_art_for_tile(&self, tile: &V) -> Option<&CubeArt> {
        self.get(tile)
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum CubeTex {
    Invisible, 
    Single(TextureId), 
    AllSides(SidesArray<TextureId>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CubeArt { 
    pub textures : CubeTex,
    pub cull_self : bool, //Do we cull the same material?
    pub cull_others : bool, //Do we cull materials other than this one?
}

impl CubeArt {
    pub fn get_render_type(&self) -> u32 { 1 }
    pub fn is_visible(&self) -> bool {
        ! (self.textures == CubeTex::Invisible)
    }
    pub fn all_textures(&self) -> Vec<&TextureId> { 
        match &self.textures {
            CubeTex::Invisible => Vec::default(),
            CubeTex::Single(v) => vec!(v),
            CubeTex::AllSides(sides) => sides.iter().collect(),
        }
    }
    pub fn simple_solid_block(texture: &TextureId) -> Self { 
        CubeArt {
            textures: CubeTex::Single(texture.clone()),
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

pub const AIR_ART: CubeArt = CubeArt { textures: CubeTex::Invisible, cull_self: false, cull_others: false };

pub fn generate_engine_texture_image(width: u32, height: u32, color_foreground: &Rgba<u8>, color_background: &Rgba<u8>) -> RgbaImage { 
    let mut img_base = RgbaImage::new(width, height);
    
    for x in 0..width { 
        for y in 0..height { 
            // The rare logical/boolean XOR. 
            if (x >= width/2) ^ (y >= height/2) { 
                img_base.put_pixel(x, y, color_foreground.clone());
            } 
            else { 
                img_base.put_pixel(x, y, color_background.clone());
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