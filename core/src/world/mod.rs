pub mod tile;
pub mod voxelstorage;
pub mod voxelarray;
pub mod voxelpalette;

pub use tile::TileCoord;
pub use tile::TileId;
use uuid::Uuid;

//pub use space::Space;
pub use voxelstorage::VoxelError;
pub use voxelstorage::VoxelErrorKind;
pub use voxelstorage::VoxelStorage;
pub use voxelstorage::VoxelStorageBounded;

use crate::common::voxelmath::VoxelPos;

use self::voxelarray::VoxelArrayStatic;
use self::voxelpalette::VoxelPalette;

pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;

pub type WorldId = Uuid;

pub const CHUNK_SIZE: usize = 16;

pub type Chunk = VoxelPalette<TileId,u8,VoxelArrayStatic<u8, CHUNK_SIZE>,u16>;