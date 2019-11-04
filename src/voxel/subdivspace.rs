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

use world::TileID;

/// An error reported upon trying to get or set a voxel which is not currently loaded. 
#[derive(Debug)]
pub enum ChunkedSubdivError<T, S> where T : VoxelCoord, S : VoxelCoord {
    NotLoaded(OctPos<T>, VoxelPos<S>),
    SubdivError(SubdivError),
}
impl<T, S> fmt::Display for ChunkedSubdivError<T, S> where T : VoxelCoord, S : VoxelCoord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ChunkedSubdivError::NotLoaded(blockpos, pos) => write!(f, "Chunk at {} not yet loaded, cannot access block {}", pos, blockpos),
            ChunkedSubdivError::SubdivError(serr) => serr.fmt(f),
        }
    }
}
impl<T, S> Error for ChunkedSubdivError<T, S> where T : VoxelCoord, S : VoxelCoord {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// A space is a "world"-like paged space for subdivided voxels (octrees and octree-like things)
/// C is our chunk type.
pub struct SubSpace<C> {
    pub chunks: HashMap<VoxelPos<i32>, 
                        Arc<RwLock<
                            C
                            >>
                        >,
    //Cubic chunks are a mandatory engine feature - they may be *stored* differently but all logic assumes cubic chunks.
    pub chunk_scale: Scale,
}

pub fn blockpos_to_chunk(point: OctPos<i32>, chunk_scale: Scale) -> OctPos<i32> {
    point.scale_to(chunk_scale)
}

///The "Point" argument here must have a scale equal to the chunk's scale. 
pub fn chunkpos_to_block(point: OctPos<i32>, block_scale: Scale) -> OctPos<i32> {
    point.scale_to(block_scale)
}

/// The "Point" argument here must have a scale equal to the chunk's scale. 
pub fn chunkpos_to_center(point: OctPos<i32>, result_scale: Scale) -> Point3<f32> {
    let block_pos = chunkpos_to_block(point, result_scale);
    //How many blocks of "result_scale" make up our chunk?
    let chunk_size : i32 = scale_coord(1, result_scale-point.scale) - 1;

    Point3::new(block_pos.pos.x as f32 + (chunk_size as f32 * 0.5), 
        block_pos.pos.y as f32 + (chunk_size as f32 * 0.5), 
        block_pos.pos.z as f32 + (chunk_size as f32 * 0.5))
}

impl<L,D,C> SubVoxelSource<SubNode<L, D>, i32> for SubSpace<C>
        where L : Voxel,  D : Voxel + LODData<L>, C : SubOctreeSource<L,D,i32> {

    fn get(&self, coord: OctPos<i32>) -> Result<SubNode<L, D>, SubdivError> {
        Ok(self.get_details(coord)?.0)
    }

    fn get_max_scale(&self) -> Scale { self.chunk_scale }
    fn get_min_scale(&self) -> Scale { -128 }
}

impl<L,D,C> SubOctreeSource<L, D, i32> for SubSpace<C>
        where L : Voxel,  D : Voxel + LODData<L>, C : SubOctreeSource<L,D,i32> {
    fn get_details(&self, coord: OctPos<i32>) -> Result<(SubNode<L, D>, Scale), SubdivError> {
                let chunkpos = blockpos_to_chunk(coord, self.chunk_scale);
        // Do we have a chunk that would contain this block position?
        match self.chunks.get(&chunkpos.pos) {
            Some(chunk_entry_arc) => {
                let chunk_entry = chunk_entry_arc.clone();

                let chunk_size = self.get_chunk_size();
                let bounds : VoxelRange<i32> = VoxelRange{ lower: VoxelPos{x:0,y:0,z:0}, 
                                upper: VoxelPos{ x: chunk_size, y: chunk_size, z: chunk_size } };
                match bounds.get_local_unsigned(coord.pos) {
                    Some(pos) => {
                        // Block until we can get a valid voxel.
                        let locked = &chunk_entry.read();
                        return Ok(locked.get_details( opos!((pos.x as i32, pos.y as i32, pos.z as i32) @ coord.scale) )?);
                    },
                    // Position is not inside our chunk's bounds.
                    None => return Err(SubdivError::OutOfBounds),
                }
            },
            // Chunk not currently loaded or generated.
            None => return Err(SubdivError::NotYetLoaded),
        }
    }
}
impl<L, C> SubVoxelDrain<L, i32> for SubSpace<C>
        where L : Voxel, C : SubVoxelDrain<L, i32> {
    fn set(&mut self, coord: OctPos<i32>, value: L) -> Result<(), SubdivError> {
        let chunkpos = blockpos_to_chunk(coord, self.chunk_scale);
        let chunk_size = self.get_chunk_size();
        // Do we have a chunk that would contain this block position?
        match self.chunks.get_mut(&chunkpos.pos) {
            Some(chunk_entry_arc) => {
                let bounds : VoxelRange<i32> = VoxelRange{ lower: VoxelPos{x:0,y:0,z:0}, 
                                upper: VoxelPos{ x: chunk_size, y: chunk_size, z: chunk_size } };
                match bounds.get_local_unsigned(coord.pos) {
                    Some(pos) => {
                        // Block until we can get a valid voxel.
                        let locked = &mut chunk_entry_arc.write();
                        return locked.set( opos!((pos.x as i32, pos.y as i32, pos.z as i32) @ coord.scale) , value);
                    },
                    // Position is not inside our chunk's bounds.
                    None => return Err(SubdivError::OutOfBounds),
                }
            },
            // Chunk not currently loaded or generated.
            None => return Err(SubdivError::NotYetLoaded),
        }
    }
}

impl<C> SubSpace<C> {
    pub fn new() -> Self {
        SubSpace {
            chunks: HashMap::new(),
            chunk_scale: CHUNK_SCALE,
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

    /// Remove all pairs such that f(&position, &mut chunk) returns false.
    pub fn retain_chunks<F>(&mut self, f: F) 
            where F: FnMut(&VoxelPos<i32>, &mut Arc<RwLock<C>>) -> bool {
        self.chunks.retain(f);
        // This does not return a bool - it just looks that way because of the closure signature.
    } 

    /// Returns chunk size in scale 0 voxels. 
    pub fn get_chunk_size(&self) -> i32 {
        ( scale_coord(1, -self.chunk_scale) - 1 )
    }
    pub fn load_new_chunk(&mut self, chunk_pos : VoxelPos<i32>, chunk: C) {
        self.chunks.insert(chunk_pos, Arc::new( RwLock::new(chunk) ));
    }
}
/*
#[test]
fn test_subdiv_space() {
    let mut world : SubSpace<NaiveVoxelOctree<String, ()>> = SubSpace::new(6);
}*/