use ustr::*;

pub trait TileArt {
    /// Which render pass should handle this tile? 
    fn get_render_type(&self) -> u32; /*In the files specifying renderers for individual TileArts, this will be 
    a string which we'll then run through a hashmap to get this u32.*/
    fn is_visible(&self) -> bool;
}
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum BlockTex {
    Invisible, 
    Single(Ustr), 
    AllSides([Ustr;6]),
}
#[derive(Clone, Debug)]
pub struct TileArtSimple { 
    pub textures : BlockTex,
    //pub cull_self : bool, //Do we cull the same material?
    //pub cull_others : bool, //Do we cull materials other than this one?
}
impl TileArt for TileArtSimple {
    fn get_render_type(&self) -> u32 { 1 }
    fn is_visible(&self) -> bool {
        ! (self.textures == BlockTex::Invisible)
    }
}