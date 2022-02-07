//! A space made up of multiple chunks - the voxel-only parts of a "world". A "Dimension". Can be multiple per server.
use crate::common::voxelmath::*;
use crate::world::chunk::{CHUNK_SIZE, CHUNK_EXP};

use hashbrown::HashMap;

use std::result::Result;

use crate::world::voxelstorage::*;
use crate::world::{ChunkCoord, ChunkPos, TileCoord, TilePos};

use super::TileId;
use super::chunk;

#[derive(thiserror::Error, Debug, Clone)]
pub enum TileSpaceError {
    #[error("Attempted to access a voxel at position {0}, which is out of bounds on this space.")]
    OutOfBounds(TilePos),
    #[error("Attempted to access a voxel at position {0}, on chunk cell {1}, which did not accept this as in-bounds")]
    ChunkBoundIssue(TilePos, ChunkPos),
    #[error("Attempted to access a voxel position {0}, which is not yet loaded.")]
    NotYetLoaded(TilePos),
    #[error("Chunk data error: {0:?}")]
    ChunkError(#[from] <chunk::Chunk<TileId> as VoxelStorage<TileId, u16>>::Error),
    #[error("Attempted to laod in a new chunk at pos {0:?}, but one was already present in that cell!")]
    LoadExistingChunk(ChunkPos),
}

impl VoxelError for TileSpaceError {
    fn kind(&self) -> VoxelErrorCategory {
        match self {
            TileSpaceError::OutOfBounds(_) => VoxelErrorCategory::OutOfBounds,
            TileSpaceError::ChunkBoundIssue(_, _) => VoxelErrorCategory::OutOfBounds,
            TileSpaceError::NotYetLoaded(_) => VoxelErrorCategory::NotYetLoaded,
            TileSpaceError::ChunkError(e) => match e {
                super::voxelarray::VoxelArrayError::OutOfBounds(_) => VoxelErrorCategory::OutOfBounds,
            },
            TileSpaceError::LoadExistingChunk(_) => VoxelErrorCategory::LoadingIssue,
        }
    }
}

pub struct TileSpace {
    pub(crate) chunks: HashMap<ChunkPos, chunk::Chunk<TileId>>,
}
impl TileSpace {
    pub fn new() -> Self { 
        Self { chunks : HashMap::new() }
    }
    /// Pull in a chunk that has been successfully loaded elsewhere in the engine. 
    pub fn ingest_loaded_chunk(&mut self, pos: ChunkPos, chunk: chunk::Chunk<TileId>) -> Result<(), TileSpaceError> { 
        if self.chunks.contains_key(&pos) { 
            Err(TileSpaceError::LoadExistingChunk(pos))
        }
        else {
            self.chunks.insert(pos, chunk).unwrap();
            Ok(())
        }
    }
}

/// Separate into chunk-local offset and the selecterd chunk cell. Returns offset from chunk, chunk cell from world.
#[inline(always)]
fn world_to_chunk_local_coord(coord: TileCoord) -> (usize, ChunkCoord) {
    let chunk_pos = coord >> CHUNK_EXP;
    let new_value = coord - (chunk_pos * CHUNK_SIZE as TileCoord); // Remainder after we cut the Chunky bit out.
    
    // If we're in a debug build and performance doesn't matter, make sure our math checks out
    #[cfg(debug_assertions)]
    {
        assert!((new_value as usize) < CHUNK_SIZE)
    }

    (new_value as usize, chunk_pos as ChunkCoord)
}

/// Figure out what chunk a given TilePos is in. 
#[inline(always)]
pub fn world_to_chunk_pos(v: &TilePos) -> ChunkPos{
    vpos!(v.x >> CHUNK_EXP as ChunkCoord, v.y >> CHUNK_EXP as ChunkCoord, v.z >> CHUNK_EXP as ChunkCoord)
}

/// Retrieve the world pos corresponding to the (0,0,0) position in our chunk at the given ChunkPos
#[inline(always)]
pub fn chunk_to_world_pos(v: &ChunkPos) -> TilePos {
    vpos!(v.x * CHUNK_SIZE as TileCoord, v.y * CHUNK_SIZE as TileCoord, v.z * CHUNK_SIZE as TileCoord)
}

impl VoxelStorage<TileId, TileCoord> for TileSpace {
    type Error = TileSpaceError;
    
    fn get(&self, pos: TilePos) -> Result<&TileId, TileSpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get(& vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                Ok(chunk.get(vpos!(x as u16, y as u16, z as u16))?)
            },
            None => Err(TileSpaceError::NotYetLoaded(pos)),
        }
    }
    fn set(&mut self, pos: TilePos, value: TileId) -> Result<(), TileSpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get_mut(&vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                Ok((*chunk).set(vpos!(x as u16, y as u16, z as u16), value)?)
            },
            None => Err(TileSpaceError::NotYetLoaded(pos)),
        }
    }
}

impl VoxelSpace<TileId> for TileSpace {
    type ChunkCoord = super::ChunkCoord;
    type WithinChunkCoord = u16;
    type Chunk = chunk::Chunk<TileId>;

    fn is_loaded(&self, voxel: TilePos) -> bool { 
        let (_, chx) = world_to_chunk_local_coord(voxel.x);
        let (_, chy) = world_to_chunk_local_coord(voxel.y);
        let (_, chz) = world_to_chunk_local_coord(voxel.z);
        self.chunks.contains_key(&vpos!(chx,chy,chz))
    }

    /// Try to borrow a chunk immutably. If it isn't loaded yet, returns error.
    fn borrow_chunk(&self, chunk: &VoxelPos<Self::ChunkCoord>) -> Result<&Self::Chunk, Self::Error> {
        self.chunks.get(chunk).ok_or(TileSpaceError::NotYetLoaded(*chunk) )
    }

    /// Try to borrow a chunk mutably. If it isn't loaded yet, returns error.
    fn borrow_chunk_mut(&mut self, chunk: &VoxelPos<Self::ChunkCoord>) -> Result<&mut Self::Chunk, Self::Error> {
        self.chunks.get_mut(chunk).ok_or(TileSpaceError::NotYetLoaded(*chunk) )
    }

    fn get_loaded_chunks(&self) -> Vec<&ChunkPos> {
        self.chunks.keys().collect()
    }

}