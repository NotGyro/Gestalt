//! Global registry types.


use std::collections::HashMap;
use std::sync::Arc;
use std::path::Path;

use vulkano::format::R8G8B8A8Srgb;
use vulkano::image::immutable::ImmutableImage;
use vulkano::device::Queue;

use crate::world::Dimension;


/// Global texture registry.
pub struct TextureRegistry {
    textures: HashMap<String, Arc<ImmutableImage<R8G8B8A8Srgb>>>
}


impl TextureRegistry {
    pub fn new() -> TextureRegistry {
        TextureRegistry {
            textures: HashMap::new()
        }
    }


    /// Loads the textures from disk, and onto the GPU.
    pub fn load(&mut self, queue: Arc<Queue>) {
        let tex_names = [
            String::from("stone"),
            String::from("dirt"),
            String::from("grass"),
            String::from("test_albedo"),
            String::from("test_normal"),
            String::from("white"),
            String::from("black"),
            String::from("grey_50"),
            String::from("gradient"),
            String::from("checker"),
        ];

        for name in tex_names.iter().clone() {
            let (texture, _future) = {
                let mut path_str = String::from("textures/");
                path_str.push_str(&name);
                path_str.push_str(".png");
                let image = ::image::open(Path::new(&path_str)).unwrap().to_rgba();
                let (w, h) = image.dimensions();
                let image_data = image.into_raw().clone();

                ::vulkano::image::immutable::ImmutableImage::from_iter(
                    image_data.iter().cloned(),
                    ::vulkano::image::Dimensions::Dim2d { width: w, height: h },
                    ::vulkano::format::R8G8B8A8Srgb,
                    queue.clone()).unwrap()
            };
            self.textures.insert(name.to_string(), texture);
        }
    }


    /// Gets a handle to the texture with the given name, or None if one couldn't be found.
    pub fn get(&self, name: &str) -> Option<Arc<ImmutableImage<R8G8B8A8Srgb>>> {
        match self.textures.get(name) {
            Some(arc) => Some(arc.clone()),
            None => None
        }
    }
}


/// Global dimension registry.
pub struct DimensionRegistry {
    pub dimensions: HashMap<u32, Dimension>
}


impl DimensionRegistry {
    pub fn new() -> DimensionRegistry {
        DimensionRegistry {
            dimensions: HashMap::new()
        }
    }


    /// Gets the dimension with the given id, or None if one couldn't be found.
    pub fn get(&mut self, id: u32) -> Option<&mut Dimension> {
        self.dimensions.get_mut(&id)
    }
}