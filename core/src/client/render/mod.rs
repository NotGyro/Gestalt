use crate::common::voxelmath::SidesArray;
use crate::resource::ResourceId;

pub mod voxelmesher; 

type TextureId = ResourceId;

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum BlockTex {
    Invisible, 
    Single(TextureId), 
    AllSides(SidesArray<TextureId>),
}
#[derive(Clone, Debug)]
pub struct SimpleTileArt { 
    pub textures : BlockTex,
    //pub cull_self : bool, //Do we cull the same material?
    //pub cull_others : bool, //Do we cull materials other than this one?
}

impl SimpleTileArt {
    pub fn get_render_type(&self) -> u32 { 1 }
    pub fn is_visible(&self) -> bool {
        ! (self.textures == BlockTex::Invisible)
    }
}