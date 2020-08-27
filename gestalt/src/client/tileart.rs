use ustr::*;

pub trait TileArt {
    /// Which render pass should handle this tile? 
    fn get_render_type(&self) -> u32; /*In the files specifying renderers for individual TileArts, this will be 
    a string which we'll then run through a hashmap to get this u32.*/
}
#[derive(Clone, Debug)]
pub struct TileArtSimple { 
    pub texture_name : Ustr,
    pub visible: bool,
    //pub cull_self : bool, //Do we cull the same material?
    //pub cull_others : bool, //Do we cull materials other than this one?
}
impl Default for TileArtSimple {
    fn default() -> Self { TileArtSimple {texture_name: ustr("missing"), visible: false} }
}
impl TileArt for TileArtSimple {
    fn get_render_type(&self) -> u32 { 1 }
}