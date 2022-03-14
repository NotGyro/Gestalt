use crate::resource::image::{
    ImageProvider, InternalImage, ID_MISSING_TEXTURE, ID_PENDING_TEXTURE,
};
use crate::resource::ResourceId;
use log::error;
use glam::Vec2;
use hashbrown::HashMap;
use image::{GenericImage, ImageError};

use super::{generate_missing_texture_image, generate_pending_texture_image};

const INDEX_MISSING_TEXTURE: usize = 0;
const INDEX_PENDING_TEXTURE: usize = 1;

#[derive(thiserror::Error, Debug)]
pub enum TileAtlasError {
    #[error("Tried to add texture {0} into a block texture atlas, but the atlas is at the provided maximum number of tiles which is {1}")]
    OverMax(String, usize),
    #[error("Tried to add texture {0} into a block texture atlas, the image size is {1:?}. The expected image size was {2:?}")]
    WrongImageSize(String, (u32, u32), (u32, u32)),
    #[error("Image compositing error while trying to use the Image library: {0:?}")]
    ImageLibraryError(#[from] ImageError),
}

fn idx_to_xy(index: usize, atlas_width: u32) -> (u32, u32) {
    let x = (index as u32) % atlas_width;
    let y = (index as u32 - x) / atlas_width;

    (x, y)
}

fn _xy_to_idx(x: u32, y: u32, atlas_width: u32) -> usize {
    (x + (y * atlas_width)) as usize
}
/// Get current atlas height from tiles.len() and atlas width
fn height_from_num_tiles(num_tiles: usize, atlas_width: u32) -> u32 {
    (num_tiles as u32).div_ceil(atlas_width)
}

pub struct TileAtlasLayout {
    /// 2D packed array of texture atlas tiles. "packed array" as in index = x + (y * GRID_WIDTH)
    tiles: Vec<ResourceId>,
    /// Pixel width AND height (these textures are for voxels) of images.
    tile_size: u32,
    /// Resource ID to index in tiles up there.
    reverse_index: HashMap<ResourceId, usize>,
    /// Current width in tiles of this texture atlas.
    atlas_width: u32,
    // Current height in tiles of this texture atlas.
    // current_height: u32,
    /// Max total number of tiles of this texture atlas.
    max_tiles: usize,
    /// How many times has this tile atlas changed? 
    revision: u64,
}

impl TileAtlasLayout {
    /// Generates a layout for a texture atlas for tile_size*tile_size texture
    pub fn new(
        tile_size: u32,
        initial_width: u32,
        initial_height: u32,
        max_tiles: Option<usize>,
    ) -> Self {
        //Sanity checks
        let width = if initial_width == 0 {
            32
        } else {
            initial_width
        };
        let mut height = if initial_height == 0 {
            1
        } else {
            initial_height
        };

        //Total number of cells when starting.
        let initial_cells = width * height;

        let mut tiles = Vec::with_capacity(initial_cells as usize);
        let mut reverse_index = HashMap::with_capacity(initial_cells as usize);

        // Make sure we've got the missing texture and any other builtins.
        tiles.push(ID_MISSING_TEXTURE.clone()); //0
        tiles.push(ID_PENDING_TEXTURE.clone()); //1
        for (i, elem) in tiles.iter().enumerate() {
            reverse_index.insert(elem.clone(), i);
        }

        //Many sanity checks
        if height_from_num_tiles(tiles.len(), width) > tiles.len() as u32 {
            height = height_from_num_tiles(tiles.len(), width);
        }
        let initial_cells = width * height;

        let max_tiles = match max_tiles {
            Some(val) => {
                if val < (initial_cells as usize) {
                    initial_cells as usize
                } else {
                    val
                }
            }
            None => usize::MAX,
        };

        TileAtlasLayout {
            tiles,
            tile_size,
            reverse_index,
            atlas_width: width,
            //current_height: height,
            max_tiles,
            revision: 0, 
        }
    }
    /// Get resolution in pixels.
    pub fn calc_resolution(&self) -> (u32, u32) {
        (
            (self.atlas_width * self.tile_size),
            (height_from_num_tiles(self.tiles.len(), self.atlas_width) * self.tile_size),
        )
    }

    pub fn get_index_for_texture(&self, resource: &ResourceId) -> Option<usize> {
        self.reverse_index.get(resource).map(|elem| *elem)
    }

    pub fn get_or_make_index_for_texture(
        &mut self,
        resource: &ResourceId,
    ) -> Result<usize, TileAtlasError> {
        match self.get_index_for_texture(resource) {
            Some(idx) => Ok(idx),
            None => {
                self.revision += 1; 

                let idx = self.tiles.len();
                if idx >= self.max_tiles {
                    return Err(TileAtlasError::OverMax(
                        resource_debug!(resource),
                        self.max_tiles,
                    ));
                }
                //Insert this into the tiles.
                self.tiles.push(resource.clone());
                //Make sure we can look it up the other way, too.
                self.reverse_index.insert(resource.clone(), idx);

                //Make sure current_height goes up if we made a new row.
                let (_x, y) = idx_to_xy(idx, self.atlas_width);

                #[cfg(debug_assertions)]
                {
                    assert!((y + 1) >= height_from_num_tiles(self.tiles.len(), self.atlas_width));
                }

                #[cfg(debug_assertions)]
                {
                    assert_eq!(self.tiles.get(idx), Some(resource));
                }

                Ok(idx)
            }
        }
    }

    /// Returns the UV of the specific texture's position on the grid.
    /// higher_x and higher_y, when set, give you the higher-x and higher-y coordinates on the rectangle,
    /// respectively. If both are false, you get the top-left corner.
    pub fn get_uv_for_index(&self, index: usize, higher_x: bool, higher_y: bool) -> Vec2 {
        let (mut x, mut y) = idx_to_xy(index, self.atlas_width);
        if higher_x {
            x += 1;
        }
        if higher_y {
            y += 1;
        }
        //X, Y, self.atlas_width, and self.current_height are all measured in "tiles" - that's the unit
        let u = (x as f32) / (self.atlas_width as f32);
        let v = (y as f32) / (height_from_num_tiles(self.tiles.len(), self.atlas_width) as f32);

        Vec2::new(u, v)
    }

    /// Returns the UV of the specific texture's position on the grid. If this texture wasn't registered, try to add it one.
    /// higher_x and higher_y, when set, give you the higher-x and higher-y coordinates on the rectangle,
    /// respectively. If both are false, you get the top-left corner.
    pub fn get_or_make_uv_for_texture(
        &mut self,
        resource: &ResourceId,
        higher_x: bool,
        higher_y: bool,
    ) -> Result<Vec2, TileAtlasError> {
        let idx = self.get_or_make_index_for_texture(resource)?;
        Ok(self.get_uv_for_index(idx, higher_x, higher_y))
    }

    pub fn get_missing_texture_uvs(&self, higher_x: bool, higher_y: bool) -> Vec2 {
        self.get_uv_for_index(INDEX_MISSING_TEXTURE, higher_x, higher_y)
    }
    pub fn get_pending_texture_uvs(&self, higher_x: bool, higher_y: bool) -> Vec2 {
        self.get_uv_for_index(INDEX_PENDING_TEXTURE, higher_x, higher_y)
    }
    pub fn get_revision(&self) -> u64 { 
        self.revision
    }
    pub fn get_max_tiles(&self) -> usize { 
        self.max_tiles
    }
    pub fn get_tile_count(&self) -> usize { 
        self.tiles.len()
    }
}

pub fn build_tile_atlas<TextureSource: ImageProvider>(
    layout: &TileAtlasLayout,
    texture_source: &mut TextureSource,
) -> Result<InternalImage, TileAtlasError> {
    let missing_texture = generate_missing_texture_image(layout.tile_size, layout.tile_size);
    let pending_texture = generate_pending_texture_image(layout.tile_size, layout.tile_size);

    let (resolution_width, resolution_height) = layout.calc_resolution();

    let mut atlas = InternalImage::new(resolution_width, resolution_height);

    for (tile_index, resource_tile) in layout.tiles.iter().enumerate() {
        //The rare mutable binding to an immutable reference shows its face again! Cool.
        let mut texture_to_use = match texture_source.load_image(resource_tile) {
            crate::resource::ResourceStatus::Pending => &pending_texture,
            crate::resource::ResourceStatus::Errored(_) => &missing_texture,
            crate::resource::ResourceStatus::Ready(image) => image,
        };

        if resource_tile == &ID_PENDING_TEXTURE {
            texture_to_use = &pending_texture;
        } else if resource_tile == &ID_MISSING_TEXTURE {
            texture_to_use = &missing_texture;
        }

        let (x, y) = idx_to_xy(tile_index, layout.atlas_width);

        //Is it the wrong size?
        if !((texture_to_use.width() == layout.tile_size)
            && (texture_to_use.height() == layout.tile_size))
        {
            
            error!(
                "Tried to add texture {} into a block texture atlas, but the image size is ({},{}). The expected image size was ({},{})",
                resource_debug!(resource_tile),
                texture_to_use.width(), texture_to_use.height(),
                layout.tile_size, layout.tile_size
            );
            //Rebind
            texture_to_use = &missing_texture;
        }

        let mut sub_image = atlas.sub_image(
            x * layout.tile_size,
            y * layout.tile_size,
            layout.tile_size,
            layout.tile_size,
        );
        sub_image.copy_from(texture_to_use, 0, 0)?;
    }

    return Ok(atlas);
}
