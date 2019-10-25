//! A dimension.

extern crate parking_lot;

use self::parking_lot::RwLock;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::error::Error;
use std::fmt;

use std::collections::HashMap;
use cgmath::{Point3};
use voxel::voxelstorage::*;
use voxel::voxelmath::*;
use voxel::subdivmath::*;
use voxel::subdivstorage::*;

use world::CHUNK_SCALE;

/// An error reported upon trying to get or set a voxel which is not currently loaded. 
#[derive(Debug)]
pub enum ChunkedSubdivError<T, S> where T : VoxelCoord, S : VoxelCoord {
    NotLoaded(OctPos<T>, VoxelPos<T>),
    ChunkBoundsInvalid(OctPos<T>, VoxelPos<T>, VoxelSize<S>, VoxelSize<S>),
    SubdivError(SubdivError),
}
impl<T, S> fmt::Display for ChunkedSubdivError<T, S> where T : VoxelCoord, S : VoxelCoord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChunkedSubdivError::NotLoaded(blockpos, pos) => write!(f, "Chunk at {} not yet loaded, cannot access block {}", pos, blockpos),
            ChunkedSubdivError::ChunkBoundsInvalid(blockpos, chunkpos, expectedchunksize, actualchunksize) => write!(f, 
                                "Failed attempt to access block {}: Chunk size invalid. Chunk at {} is supposed to be of size {}, and it is {}.", 
                                blockpos, chunkpos, expectedchunksize, actualchunksize),
            ChunkedSubdivError::SubdivError(serr) => serr.fmt(f),
        }
    }
}
impl<T, S> Error for ChunkedSubdivError<T, S> where T : VoxelCoord, S : VoxelCoord {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

// TODO: Rewrite this to use the standard Futures API.
/// State used for multithreaded chunk loading. Chunk is dirty and needs to be generated.
pub static CHUNK_STATE_DIRTY: usize = 0;
/// State used for multithreaded chunk loading. Chunk mesh is currently being generated.
pub static CHUNK_STATE_WRITING: usize = 1;
/// State used for multithreaded chunk loading. Chunk is finished being generated.
pub static CHUNK_STATE_CLEAN: usize = 2;

pub type ChunkEntry<L,D> = NaiveVoxelOctree<L,D>;
/// A "world"-like paged space for subdivided voxels (octrees and octree-like things)
pub struct SubSpace<L : Voxel, D : Voxel + LODData<L>> {
    pub chunks: HashMap<VoxelPos<i32>, 
                        Arc<RwLock<
                            ChunkEntry<L,D>
                            >>
                        >,
    chunk_size: VoxelSize<u32>, //Calculated at construction time from 
}

pub fn blockpos_to_chunk(point: VoxelPos<i32>, chunk_size : VoxelSize<u32>) -> VoxelPos<i32> {
    vpos!((point.x as f32 / chunk_size.x as f32).floor() as i32, 
        (point.y as f32 / chunk_size.y as f32).floor() as i32, 
        (point.z as f32 / chunk_size.z as f32).floor() as i32)
}

pub fn chunkpos_to_block(point: VoxelPos<i32>, chunk_size : VoxelSize<u32>) -> VoxelPos<i32> { 
    vpos!(point.x * chunk_size.x as i32, 
        point.y * chunk_size.y as i32, 
        point.z * chunk_size.z as i32)
}

pub fn chunkpos_to_center(point: VoxelPos<i32>, chunk_size : VoxelSize<u32>) -> Point3<f32> { 
    let block_pos = chunkpos_to_block(point, chunk_size);
    Point3::new(block_pos.x as f32 + (chunk_size.x as f32 * 0.5), 
        block_pos.y as f32 + (chunk_size.y as f32 * 0.5), 
        block_pos.z as f32 + (chunk_size.z as f32 * 0.5))
}

#[test]
fn test_chunkpos() { 
    assert!(blockpos_to_chunk(vpos!(6, -1, 7), vpos!(16, 16, 16)) == vpos!(0, -1, 0));
    assert!(blockpos_to_chunk(vpos!(17, -25, 2), vpos!(8, 24, 4)) == vpos!(2, -2, 0));
}

impl VoxelStorage<BlockID, i32> for Dimension {
    fn get(&self, coord: VoxelPos<i32>) -> Result<BlockID, VoxelError>{
        let size = self.chunk_size.clone();
        let chunkpos = blockpos_to_chunk(coord, size);
        // Do we have a chunk that would contain this block position?
        match self.chunks.get(&chunkpos) {
            Some(chunk_entry_arc) => {
                let chunk_entry = chunk_entry_arc.clone();
                let bounds = chunk_entry.bounds.clone();
                let chunk_size = bounds.get_size_unsigned();
                if chunk_size != size {
                    return Err(VoxelError::Other(
                        Box::new(ChunkedVoxelError::ChunkBoundsInvalid(coord, chunkpos, size, chunk_size, bounds))));
                }
                match bounds.get_local_unsigned(coord) {
                    Some(pos) => {
                        // Block until we can get a valid voxel.
                        let locked = chunk_entry.data.read();
                        return Ok(locked.get(vpos!(pos.x as u8, pos.y as u8, pos.z as u8))?);
                    },
                    // Position is not inside our chunk's bounds.
                    None => return Err(VoxelError::Other(
                        Box::new(ChunkedVoxelError::ChunkBoundsInvalid(coord, chunkpos, size, chunk_size, bounds)))),
                }
            },
            // Chunk not currently loaded or generated.
            None => return Err(VoxelError::NotYetLoaded(format!("{}", coord))),
        }
    }
    fn set(&mut self, coord: VoxelPos<i32>, value: BlockID) -> Result<(), VoxelError>{
        let size = self.chunk_size.clone();
        // Do we have a chunk that would contain this block position?
        let chunkpos = blockpos_to_chunk(coord, size);
        let rslt = self.chunks.get(&chunkpos).cloned();
        match rslt {
            Some(chunk_entry) => {
                let bounds = chunk_entry.bounds.clone();
                let chunk_size = bounds.get_size_unsigned();
                if chunk_size != size {
                    return Err(VoxelError::Other(
                        Box::new(ChunkedVoxelError::ChunkBoundsInvalid(coord, chunkpos, size, chunk_size, bounds))));
                }
                match bounds.get_local_unsigned(coord) {
                    Some(pos) => {
                        // Block until we can write.
                        let mut locked = chunk_entry.data.write();
                        let position = vpos!(pos.x as u8, pos.y as u8, pos.z as u8);
                        let current = locked.get(position)?;
                        if current != value {
                            chunk_entry.state.store(CHUNK_STATE_DIRTY, Ordering::Relaxed); //Mark for remesh.
                            locked.set(position, value)?;
                        }
                    },
                    // Position is not inside our chunk's bounds.
                    None => return Err(VoxelError::Other(
                        Box::new(ChunkedVoxelError::ChunkBoundsInvalid(coord, chunkpos, size, chunk_size, bounds)))),
                }
            },
            // Chunk not currently loaded or generated.
            None => return Err(VoxelError::NotYetLoaded(format!("{}", coord))),
        }
        Ok(())
    }
}

impl Dimension {
    pub fn new() -> Dimension {
        Dimension {
            chunks: HashMap::new(),
            chunk_size: vpos!(16, 16, 16),
        }
    }

    pub fn is_chunk_loaded(&self, chunk_pos : VoxelPos<i32> ) -> bool {self.chunks.contains_key(&chunk_pos)}

    pub fn loaded_chunk_list(&self) -> Vec<VoxelPos<i32>> {
        let mut result = Vec::new();
        for pos in self.chunks.keys() {
            result.push(*pos);
        }
        result
    }
}