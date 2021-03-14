//! A space made up of multiple chunks - the voxel-only parts of a "world". A "Dimension". Can be multiple per server.
use crate::world::tile::*;
use crate::world::chunk::*;
use crate::common::voxelmath::*;

use hashbrown::HashMap;

use std::error::Error;
use std::result::Result;
use ustr::*;

use crate::world::voxelstorage::*;
use crate::world::{ChunkCoord, ChunkPos, TileCoord, TilePos};

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
    pub fn get(&self, pos: TilePos) -> Result<TileId, VoxelError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get(& vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                return Result::Ok(chunk.get(x, y, z));
            },
            None => return Result::Err(VoxelError::NotYetLoaded(pos)),
        }
    }
    pub fn set(&mut self, pos: TilePos, value: TileId) -> Result<(), VoxelError> {
        let (x, chx) = world_to_chunk_local_coord(pos.x);
        let (y, chy) = world_to_chunk_local_coord(pos.y);
        let (z, chz) = world_to_chunk_local_coord(pos.z);
        match self.chunks.get_mut(&vpos!(chx,chy,chz) ) {
            Some(chunk) => {
                (*chunk).set(x, y, z, value);
                return Result::Ok(());
            },
            None => return Result::Err(VoxelError::NotYetLoaded(pos)),
        }
    }
    pub fn is_loaded(&self, voxel: TilePos) -> bool { 
        let (_, chx) = world_to_chunk_local_coord(voxel.x);
        let (_, chy) = world_to_chunk_local_coord(voxel.y);
        let (_, chz) = world_to_chunk_local_coord(voxel.z);
        self.chunks.contains_key(&vpos!(chx,chy,chz))
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
            let chunk = Chunk{revision: 0, inner: ChunkInner::Uniform(ustr("air"))};
            self.chunks.insert(pos, chunk);
        }
        else if pos.y == 0  {
            let mut chunk = Chunk{revision: 0, inner: ChunkInner::Uniform(ustr("stone"))};
            let grass_id = chunk.add_to_palette(ustr("grass"));
            let dirt_id = chunk.add_to_palette(ustr("dirt"));
            for x in 0..CHUNK_SZ {
                for y in (CHUNK_SZ - 6)..CHUNK_SZ {
                    for z in 0..CHUNK_SZ {
                        if x % 2 == 0 { 
                            chunk.set_raw(x, y, z, dirt_id);
                        }
                        else {
                            if y == (CHUNK_SZ-1) {
                                chunk.set_raw(x, y, z, grass_id);
                            }
                            else if y >= (CHUNK_SZ-4) {
                                chunk.set_raw(x, y, z, dirt_id);
                            }
                        }
                    }
                }
            }
            self.chunks.insert(pos, chunk);
        }
        else { 
            //Necessarily, pos.y < 0
            let chunk = Chunk{revision: 0, inner: ChunkInner::Uniform(ustr("stone"))};
            self.chunks.insert(pos, chunk);
        }
        Ok(())
    }
    pub fn get_loaded_chunks(&self) -> Vec<ChunkPos> {
        self.chunks.keys().map(|c| c.clone()).collect()
    }
}