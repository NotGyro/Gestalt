pub mod tile;
pub mod chunk;
pub mod space;
pub mod voxelstorage;
pub mod voxelevent; 

pub use tile::TileId;
pub use chunk::Chunk;
pub use chunk::CHUNK_SZ;
pub use chunk::CHUNK_EXP;
pub use chunk::CHUNK_SQUARED;
pub use chunk::CHUNK_VOLUME;
pub use space::Space;
pub use voxelstorage::VoxelError;
pub use voxelstorage::VoxelErrorKind;
pub use voxelstorage::VoxelStorage;
pub use voxelstorage::VoxelStorageBounded;


pub type WorldId = u8;

use crate::util::voxelmath::VoxelPos;
pub type TileCoord = i32; 
pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;