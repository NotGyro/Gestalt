//! A space made up of multiple chunks - the voxel-only parts of a "world". A "Dimension". Can be multiple per server.
use crate::world::tile::*;
use crate::world::chunk::*;
use crate::common::voxelmath::*;

use hashbrown::HashMap;

use std::result::Result;

use crate::world::voxelstorage::*;
use crate::world::{ChunkCoord, ChunkPos, TileCoord, TilePos};

use super::chunk;

#[derive(thiserror::Error, Debug, Clone)]
pub enum VoxelSpaceError {
    #[error("Attempted to access a voxel at position {0}, which is out of bounds on this space.")]
    OutOfBounds(TilePos),
    #[error("Attempted to access a voxel at position {0}, on chunk cell {1}, which did not accept this as in-bounds")]
    ChunkBoundIssue(TilePos, ChunkPos),
    #[error("Attempted to access a voxel position {0}, which is not yet loaded.")]
    NotYetLoaded(TilePos),
    #[error("Chunk data error: {0:?}")]
    ChunkError(#[from] <chunk::Chunk<TileId> as VoxelStorage<TileId, u16>>::Error)
}

impl VoxelError for VoxelSpaceError {
    fn kind(&self) -> VoxelErrorCategory {
        match self {
            VoxelSpaceError::OutOfBounds(_) => VoxelErrorCategory::OutOfBounds,
            VoxelSpaceError::ChunkBoundIssue(_, _) => VoxelErrorCategory::OutOfBounds,
            VoxelSpaceError::NotYetLoaded(_) => VoxelErrorCategory::NotYetLoaded,
            VoxelSpaceError::ChunkError(e) => match e {
                super::voxelarray::VoxelArrayError::OutOfBounds(_) => VoxelErrorCategory::OutOfBounds,
            },
        }
    }
}

pub struct VoxelSpace {
    /// HashMap<chunk position, chunk>
    pub chunks: HashMap<ChunkPos, Chunk<TileId>>,
}

/// Separate into chunk-local offset and the selecterd chunk cell. Returns offset from chunk, chunk cell from world.
#[inline(always)]
fn world_to_chunk_local_coord(coord: TileCoord) -> (usize, ChunkCoord) {
    let chunk_pos = coord >> CHUNK_EXP;
    let new_value = coord - (chunk_pos * CHUNK_SIZE as i32); // Remainder after we cut the Chunky bit out.
    
    // If we're in a debug build and performance doesn't matter, make sure our math checks out
    #[cfg(debug_assertions)]
    {
        assert!((new_value as usize) < CHUNK_SIZE)
    }

    (new_value as usize, chunk_pos as ChunkCoord)
}

/// Figure out what chunk a given TilePos is in. 
#[inline(always)]
pub fn world_to_chunk_pos(v: TilePos) -> ChunkPos{
    vpos!(v.x >> CHUNK_EXP as i32, v.y >> CHUNK_EXP as i32, v.z >> CHUNK_EXP as i32)
}

/// Retrieve the world pos corresponding to the (0,0,0) position in our chunk at the given ChunkPos
#[inline(always)]
pub fn chunk_to_world_pos(v: ChunkPos) -> TilePos {
    vpos!(v.x * CHUNK_SIZE as i32, v.y * CHUNK_SIZE as i32, v.z * CHUNK_SIZE as i32)
}

impl VoxelSpace {
    pub fn new() -> Self { 
        Self { chunks : HashMap::new() }
    }
    pub fn get(&self, pos: TilePos) -> Result<&TileId, VoxelSpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get(& vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                Ok(chunk.get(vpos!(x as u16, y as u16, z as u16))?)
            },
            None => Err(VoxelSpaceError::NotYetLoaded(pos)),
        }
    }
    pub fn set(&mut self, pos: TilePos, value: TileId) -> Result<(), VoxelSpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get_mut(&vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                Ok((*chunk).set(vpos!(x as u16, y as u16, z as u16), value)?)
            },
            None => Err(VoxelSpaceError::NotYetLoaded(pos)),
        }
    }
    pub fn is_loaded(&self, voxel: TilePos) -> bool { 
        let (_, chx) = world_to_chunk_local_coord(voxel.x);
        let (_, chy) = world_to_chunk_local_coord(voxel.y);
        let (_, chz) = world_to_chunk_local_coord(voxel.z);
        self.chunks.contains_key(&vpos!(chx,chy,chz))
    }

    /// Try to borrow a chunk immutably. If it isn't loaded yet, returns None.
    pub fn borrow_chunk(&self, chunk: ChunkPos) -> Option<&Chunk<TileId>> {
        self.chunks.get(&chunk)
    }

    /// Try to borrow a chunk mutably. If it isn't loaded yet, returns None.
    pub fn borrow_chunk_mut(&mut self, chunk: ChunkPos) -> Option<&mut Chunk<TileId>> {
        self.chunks.get_mut(&chunk)
    }

    pub fn get_loaded_chunks(&self) -> Vec<&ChunkPos> {
        self.chunks.keys().collect()
    }
}

impl VoxelStorage<TileId, TileCoord> for VoxelSpace {
    type Error = VoxelSpaceError;

    fn get(&self, coord: VoxelPos<TileCoord>) -> Result<&TileId, Self::Error> {
        VoxelSpace::get(self, coord)
    }

    fn set(&mut self, coord: VoxelPos<TileCoord>, value: TileId) -> Result<(), Self::Error> {
        VoxelSpace::set(self, coord, value)
    }
}