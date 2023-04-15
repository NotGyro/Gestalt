//! Structures used by the rest of the engine to describe the desired appearance of a voxel
//! to the renderer.
//! This should very closely resemble text-format data files the user can create manually.

use std::collections::HashMap;

use crate::{world::voxelstorage::Voxel, common::voxelmath::{SidesArray, VoxelSide}, resource::ResourceId};

pub trait VoxelArtMapper<V>
where
    V: Voxel,
{
    fn get_art_for_tile(&self, tile: &V) -> Option<&VoxelArt>;
}

impl<V> VoxelArtMapper<V> for HashMap<V, VoxelArt>
where
    V: Voxel,
{
    fn get_art_for_tile(&self, tile: &V) -> Option<&VoxelArt> {
        self.get(tile)
    }
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum CubeTex {
    Single(ResourceId),
    AllSides(Box<SidesArray<ResourceId>>),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CubeArt {
    pub textures: CubeTex,
    pub cull_self: bool,   //Do we cull the same material?
    pub cull_others: bool, //Do we cull materials other than this one?
}

impl CubeArt {
    pub fn texture_for_side(&self, side: VoxelSide) -> &ResourceId { 
        match &self.textures {
            CubeTex::Single(r_id) => r_id,
            CubeTex::AllSides(sides_array) => sides_array.get(side),
        }
    }
    pub fn get_all_sides<'a>(&'a self) -> Vec<&'a ResourceId> { 
        match &self.textures {
            CubeTex::Single(r_id) => vec!(r_id),
            CubeTex::AllSides(sides_array) => sides_array.get_all().to_vec(),
        }
    }
    pub fn simple_solid_block(texture: &ResourceId) -> Self {
        CubeArt {
            textures: CubeTex::Single(*texture),
            cull_self: true,
            cull_others: true,
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum VoxelArtKind {
    /// For air, empty-space, etc, anything that doesn't render. 
    Invisible,
    /// Just a bloxel in the strictest sense.
    SimpleCube,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoxelArt {
    /// For air, empty-space, etc, anything that doesn't render. 
    Invisible,
    /// Just a bloxel in the strictest sense.
    SimpleCube(CubeArt)
}

impl VoxelArt {
    pub fn get_kind(&self) -> VoxelArtKind { 
        match self {
            VoxelArt::Invisible => VoxelArtKind::Invisible,
            VoxelArt::SimpleCube(_) => VoxelArtKind::SimpleCube,
        }
    }
}