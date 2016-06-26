pub trait MaterialArt {
    pub fn get_render_type(&self) -> u32; /*In the files specifying renderers for individual MaterialArts, this will be 
    a string which we'll then run through a hashmap to get this u32.*/
}

pub struct MatArtSimple { 
    pub TextureName : String;
}

impl MaterialArt for MatArtSimple {
    pub fn get_render_type(&self) -> u32 { 1 }
}