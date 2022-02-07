pub mod chunk;
pub mod voxelarray;
pub mod tilespace;
pub mod voxelstorage;

use uuid::Uuid;

//pub use space::Space;
pub use voxelstorage::VoxelStorage;
pub use voxelstorage::VoxelStorageBounded;

use crate::common::identity::NodeIdentity;
use crate::common::voxelmath::VoxelPos;

/// Tiles as they are interacted with in the world (not as stored in a chunk, necessarily) - as in, what a Space will return when you call world_voxel_space.get(x, y, z)
pub type TileId = u32;

/// One coorinate (worldspace) of a tile in a 3D 3-coordinate system (i.e. x: TileCoord, y: TileCoord, z: TileCoord)
pub type TileCoord = i32;

pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;

pub struct WorldId {
    pub uuid: Uuid,
    /// Either us or the server we're mirroring this from. 
    pub host: NodeIdentity,
}

use self::tilespace::TileSpace;
use self::tilespace::TileSpaceError;

pub struct World {
    pub world_id: WorldId,
    pub space: TileSpace,
}

impl World {
    pub fn get_tile(&self, coord: TilePos) -> Result<&TileId, TileSpaceError> {
        self.space.get(coord)
    }
    pub fn set_tile(&mut self, coord: TilePos, tile: TileId) -> Result<(), TileSpaceError> {
        self.space.set(coord, tile)
    }
}