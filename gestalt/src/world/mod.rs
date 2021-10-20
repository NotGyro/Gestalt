pub mod tile;
pub mod chunk;
pub mod space;
pub mod voxelstorage;
pub mod voxelevent; 

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

use std::error::Error;
use std::time::{Instant};
/*
pub struct World {
    pub space : Space,
    pub last_update : Instant,
    pub tick : u64,
}

impl World { 
    pub fn new() -> Self { 
        World {
            space: Space::new(),

            last_update : Instant::now(),
            tick : 0,
        }
    }

    pub fn update(&mut self) -> Result<(), Box<dyn Error>> {
        let _elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();
        Ok(())
    }
}*/