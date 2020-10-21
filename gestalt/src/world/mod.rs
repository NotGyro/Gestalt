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

use crate::common::voxelmath::VoxelPos;
pub type TileCoord = i32; 
pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;

use crate::entity::*;
use crate::common::message::*;
use std::error::Error;
use std::time::{Instant, Duration};

pub struct World {
    pub space : Space,
    pub entity_world : legion::World,
    pub schedule : legion::Schedule,
    pub entity_resources : legion::Resources,
    pub last_update : Instant,
    pub tick : u64,
}

impl World { 
    pub fn new() -> Self { 
        World {
            space: Space::new(),
            entity_world: legion::world::World::default(),
            schedule : legion::Schedule::builder().add_system(update_positions_system()).build(),
            entity_resources: legion::Resources::default(),
            last_update : Instant::now(),
            tick : 0,
        }
    }

    pub fn update(&mut self) -> Result<(), Box<dyn Error>> {
        let elapsed = self.last_update.elapsed();
        self.last_update = Instant::now();
        self.entity_resources.insert(TimeStep{0: elapsed.as_secs_f32()});
        self.schedule.execute(&mut self.entity_world, &mut self.entity_resources);
        Ok(())
    }
}