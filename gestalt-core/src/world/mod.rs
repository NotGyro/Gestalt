pub mod chunk;
pub mod voxelarray;
pub mod tilespace;
pub mod voxelstorage;

use bimap::BiMap;
use hashbrown::HashMap;
use uuid::Uuid;

use string_cache::DefaultAtom as Atom;

//pub use space::Space;
pub use voxelstorage::VoxelStorage;
pub use voxelstorage::VoxelStorageBounded;

use crate::common::voxelmath::VoxelPos;
use crate::script::ModuleId;

/// Tiles as they are interacted with in the world (not as stored in a chunk, necessarily) - as in, what a Space will return when you call chunk.get(x, y, z)
pub type TileStateId = u32;

/// One coorinate (worldspace) of a tile in a 3D 3-coordinate system (i.e. x: TileCoord, y: TileCoord, z: TileCoord)
pub type TileCoord = i32;

pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;

pub type WorldId = Uuid;

use self::tilespace::TileSpace;
use self::tilespace::TileSpaceError;


pub type TileId = u32; 
/// Unlocalized name of a tile, as namespaced by a ModuleId.
pub type TileName = Atom;

type ModuleIdInternal = u32;

#[derive(Debug, Clone, Hash, PartialOrd, PartialEq)]
pub struct WorldModuleHandle { 
    internal: ModuleIdInternal,
}

pub struct TileDef { 

}

/// How TileDefs are stored inside a world. 
struct TileDefInternal {

}

#[derive(Debug, Clone, Hash, PartialOrd, PartialEq)]
struct QualifiedTileName {
    name: TileName,
    mod_id: WorldModuleHandle,
}

/// The point at which Gestalt knows more about tiles 
/// than just "here is an integer."
struct TileRegistry {
    named_tiles: BiMap<QualifiedTileName, TileId>,
    ///Which tile maps are consumed by which tile IDs?
    states_taken: BiMap<TileStateId, TileId>,
    /// *Persistently* attached modules... i.e., modules that own tile IDs. 
    attached_modules: BiMap<ModuleIdInternal, ModuleId>,
}

pub struct World {
    pub world_id: WorldId,
    pub space: TileSpace,
    tile_registry: TileRegistry, 
}

impl World {
    pub fn get_tile_state(&self, coord: TilePos) -> Result<&TileStateId, TileSpaceError> {
        self.space.get(coord)
    }
    pub fn set_tile_state(&mut self, coord: TilePos, tile: TileStateId) -> Result<(), TileSpaceError> {
        self.space.set(coord, tile)
    }
}