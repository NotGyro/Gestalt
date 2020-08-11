//! A space made up of multiple chunks - the voxel-only parts of a "world". A "Dimension". Can be multiple per server.
use crate::world::tile::*;
use crate::world::chunk::*;
use crate::world::chunk::CHUNK_SZ;

use hashbrown::HashMap;

use std::fmt::{Display, Debug};
use std::fmt;
use std::error;
use std::error::Error;
use std::result::Result;

pub type TileCoord = i32; 
pub type TilePos = crate::util::voxelmath::VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = crate::util::voxelmath::VoxelPos<ChunkCoord>;

#[allow(dead_code)]
pub enum SpaceErrorKind {
    OutOfBounds,
    ChunkBoundIssue,
    NotYetLoaded,
    Other,
}
/// An error reported upon trying to get or set a voxel outside of our range.
#[derive(Debug)]
#[allow(dead_code)]
pub enum SpaceError {
    OutOfBounds(TilePos),
    ChunkBoundIssue(TilePos, ChunkPos),
    NotYetLoaded(TilePos),
    Other(Box<dyn error::Error + 'static>),
}

impl SpaceError {
    #[allow(dead_code)]
    fn kind(&self) -> SpaceErrorKind {
        match self {
            SpaceError::OutOfBounds(_) => SpaceErrorKind::OutOfBounds,
            SpaceError::ChunkBoundIssue(_,_) => SpaceErrorKind::ChunkBoundIssue,
            SpaceError::NotYetLoaded(_) => SpaceErrorKind::NotYetLoaded,
            SpaceError::Other(_) => SpaceErrorKind::Other,
        }
    }
}

impl Display for SpaceError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            SpaceError::OutOfBounds(pos) => write!(f, "Attempted to access a voxel at position {}, which is out of bounds on this space.", pos),
            SpaceError::ChunkBoundIssue(pos, chunkpos) => 
                write!(f, "Attempted to access a voxel at position {}, on chunk cell {}, which did not accept this as in-bounds.", pos, chunkpos),
            SpaceError::NotYetLoaded(pos) => write!(f, "Attempted to access a voxel position {}, which is not yet loaded.", pos),
            SpaceError::Other(err) => write!(f, "Other voxel error: {}", err),
        }
    }
}
impl Error for SpaceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None //I would love to have it to handle Other correctly but nope, the sized variablre requirement isn't having it.
    }
}

pub struct Space {
    /// HashMap<chunk position, chunk>
    pub chunks: HashMap<(ChunkCoord, ChunkCoord, ChunkCoord), Chunk>,
}

/// Separate into chunk-local offset and the selecterd chunk cell. Returns offset from chunk, chunk cell from world.
#[inline(always)]
fn world_to_chunk_local_coord(v: TileCoord) -> (usize, ChunkCoord) {
    let chp = v >> CHUNK_EXP;
    let nv = v - (chp * CHUNK_SZ as i32); // Remainder after we cut the Chunky bit out.
    (nv as usize, chp as ChunkCoord)
}

impl Space {
    pub fn new() -> Self { 
        Space { chunks : HashMap::new() }
    }
    pub fn get(&self, pos: TilePos) -> Result<TileID, SpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get(&(chx, chy, chz)) {
            Some(chunk) => {
                return Result::Ok(chunk.get(x, y, z));
            },
            None => return Result::Err(SpaceError::NotYetLoaded(pos)),
        }
    }
    pub fn set(&mut self, pos: TilePos, value: TileID) -> Result<(), SpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get_mut(&(chx, chy, chz)) {
            Some(chunk) => {
                (*chunk).set(x, y, z, value);
                return Result::Ok(());
            },
            None => return Result::Err(SpaceError::NotYetLoaded(pos)),
        }
    }
}