use crate::resource::ResourceId;

use super::TextureHandle;

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum BillboardStyle { 
    Spherical,
    Cylindrical,
}

// I originally started writing a whole system of sprite resource, UV, 
// and array-texture-index selection. However, I realized I was falling into "Waterfall" again.
// I didn't understand the problem domain well enough to start generalizing and abstracting in it.
// So, we'll figure out how to structure things like this as we go along. 
#[derive(Clone, Debug)]
pub struct BillboardDrawable {
    pub texture: ResourceId,
    /// Size in-world (in meters) that the sprite should appear as. 
    pub width: f32,
    /// Size in-world (in meters) that the sprite should appear as. 
    pub height: f32,
    pub style: BillboardStyle,
    pub(in crate::client::render) texture_handle: Option<TextureHandle>,
}

impl BillboardDrawable {
    pub fn new(base_texture: ResourceId, style: BillboardStyle) -> Self {
        Self {
            texture: base_texture,
            width: 1.0,
            height: 1.0,
            style,
            texture_handle: None, // Uninitialized, will get lazy-loaded. 
        }
    }
    pub fn set_size(&mut self, width: f32, height: f32) { 
        self.width = width;
        self.height = height;
    }
}