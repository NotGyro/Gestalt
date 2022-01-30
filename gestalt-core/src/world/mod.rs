pub mod chunk;
pub mod tile;
pub mod voxelarray;
pub mod voxelspace;
pub mod voxelstorage;

pub use tile::TileCoord;
pub use tile::TileId;
use uuid::Uuid;

//pub use space::Space;
pub use voxelstorage::VoxelStorage;
pub use voxelstorage::VoxelStorageBounded;

use crate::common::voxelmath::VoxelPos;

pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;

pub type WorldId = Uuid;

use string_cache::DefaultAtom as Atom;
pub type TileName = Atom;
/*
pub struct World { 
    space: VoxelSpace,

}*/