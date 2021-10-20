pub mod tile;
pub mod chunk;
pub mod voxelstorage;

pub use tile::TileId;
pub use tile::TileCoord;
pub use chunk::*;
//pub use space::Space;
pub use voxelstorage::VoxelError;
pub use voxelstorage::VoxelErrorKind;
pub use voxelstorage::VoxelStorage;
pub use voxelstorage::VoxelStorageBounded;

use crate::common::voxelmath::VoxelPos;
pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;