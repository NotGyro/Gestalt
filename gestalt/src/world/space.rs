//! A space made up of multiple chunks - the voxel-only parts of a "world". A "Dimension". Can be multiple per server.
use crate::world::tile::*;
use crate::world::chunk::*;
use crate::world::chunk::CHUNK_SZ;
use crate::util::voxelmath::*;

use hashbrown::HashMap;

use std::fmt::{Display, Debug};
use std::fmt;
use std::error;
use std::error::Error;
use std::result::Result;
use ustr::*;

pub type TileCoord = i32; 
pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;

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
    pub chunks: HashMap<ChunkPos, Chunk>,
}

/// Separate into chunk-local offset and the selecterd chunk cell. Returns offset from chunk, chunk cell from world.
#[inline(always)]
fn world_to_chunk_local_coord(v: TileCoord) -> (usize, ChunkCoord) {
    let chp = v >> CHUNK_EXP;
    let nv = v - (chp * CHUNK_SZ as i32); // Remainder after we cut the Chunky bit out.
    (nv as usize, chp as ChunkCoord)
}
#[inline(always)]
pub fn world_to_chunk_pos(v: TilePos) -> ChunkPos{
    vpos!(v.x >> CHUNK_EXP as i32, v.y >> CHUNK_EXP as i32, v.z >> CHUNK_EXP as i32)
}

#[inline(always)]
pub fn chunk_to_world_pos(v: ChunkPos) -> TilePos {
    vpos!(v.x * CHUNK_SZ as i32, v.y * CHUNK_SZ as i32, v.z * CHUNK_SZ as i32)
}


impl Space {
    pub fn new() -> Self { 
        Space { chunks : HashMap::new() }
    }
    pub fn get(&self, pos: TilePos) -> Result<TileId, SpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get(& vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                return Result::Ok(chunk.get(x, y, z));
            },
            None => return Result::Err(SpaceError::NotYetLoaded(pos)),
        }
    }
    pub fn set(&mut self, pos: TilePos, value: TileId) -> Result<(), SpaceError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get_mut(& vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                (*chunk).set(x, y, z, value);
                return Result::Ok(());
            },
            None => return Result::Err(SpaceError::NotYetLoaded(pos)),
        }
    }
    pub fn borrow_chunk(&self, chunk: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&chunk)
    }

    pub fn load_or_gen_chunk(&mut self, pos: ChunkPos) -> Result<(), Box<dyn Error>> { 
        //TODO: Loading from disk.
        self.gen_chunk(pos)
    }
    pub fn gen_chunk(&mut self, pos: ChunkPos) -> Result<(), Box<dyn Error>> {
        if pos.y > 0 {
            //Surface chunk, all air.
            let chunk = Chunk{revision_number: 0, inner: ChunkInner::Uniform(ustr("air"))};
            self.chunks.insert(pos, chunk);
        }
        else if pos.y == 0  {
            let mut chunk = Chunk{revision_number: 0, inner: ChunkInner::Uniform(ustr("stone"))};
            let grass_id = chunk.add_to_palette(ustr("grass"));
            let dirt_id = chunk.add_to_palette(ustr("dirt"));
            for x in 0..CHUNK_SZ {
                for y in (CHUNK_SZ - 6)..CHUNK_SZ {
                    for z in 0..CHUNK_SZ {
                        if y == (CHUNK_SZ-1) {
                            chunk.set_raw(x, y, z, grass_id);
                        }
                        else if y >= (CHUNK_SZ-4) {
                            chunk.set_raw(x, y, z, dirt_id);
                        }
                    }
                }
            }
            self.chunks.insert(pos, chunk);
        }
        else { 
            //Necessarily, pos.y < 0
            let chunk = Chunk{revision_number: 0, inner: ChunkInner::Uniform(ustr("stone"))};
            self.chunks.insert(pos, chunk);
        }
        Ok(())
    }
    pub fn get_loaded_chunks(&self) -> Vec<ChunkPos> {
        self.chunks.keys().map(|c| c.clone()).collect()
    }
}