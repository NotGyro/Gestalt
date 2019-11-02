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

// TODO: Rewrite this to use the standard Futures API.
/// State used for multithreaded chunk loading. Chunk is dirty and needs to be generated.
pub static CHUNK_STATE_DIRTY: usize = 0;
/// State used for multithreaded chunk loading. Chunk mesh is currently being generated.
pub static CHUNK_STATE_WRITING: usize = 1;
/// State used for multithreaded chunk loading. Chunk is finished being generated.
pub static CHUNK_STATE_CLEAN: usize = 2;

/// A space is a "world"-like paged space for subdivided voxels (octrees and octree-like things)
/// C is our chunk type.
pub struct SubSpace<C> {
    pub chunks: HashMap<VoxelPos<i32>, 
                        Arc<RwLock<
                            C
                            >>
                        >,
    //Cubic chunks are a mandatory engine feature - they may be *stored* differently but all logic assumes cubic chunks.
    chunk_scale: Scale,
}
/*
/// This is only valid at / assumes Scale=0
pub fn blockpos_to_chunk(point: VoxelPos<i32>, chunk_size : u32) -> VoxelPos<i32> {
    VoxelPos{ x: (point.x as f32 / chunk_size as f32).floor() as i32, 
        y: (point.y as f32 / chunk_size as f32).floor() as i32, 
        z: (point.z as f32 / chunk_size as f32).floor() as i32, }
}

/// This is only valid at / assumes Scale=0
pub fn chunkpos_to_block(point: VoxelPos<i32>, chunk_size : u32) -> VoxelPos<i32> { 
    VoxelPos{ x: point.x * chunk_size as i32, 
        y: point.y * chunk_size as i32, 
        z: point.z * chunk_size as i32, }
}

/// This is only valid at / assumes Scale=0
pub fn chunkpos_to_center(point: VoxelPos<i32>, chunk_size : u32) -> Point3<f32> { 
    let block_pos = chunkpos_to_block(point, chunk_size);
    Point3::new(block_pos.x as f32 + (chunk_size as f32 * 0.5), 
        block_pos.y as f32 + (chunk_size as f32 * 0.5), 
        block_pos.z as f32 + (chunk_size as f32 * 0.5))
}

#[test]
fn test_chunkpos() { 
    assert!(blockpos_to_chunk(vpos!(6, -1, 7), vpos!(16, 16, 16)) == vpos!(0, -1, 0));
    assert!(blockpos_to_chunk(vpos!(17, -25, 2), vpos!(8, 24, 4)) == vpos!(2, -2, 0));
}*/

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
    let mut chunk_size : i32 = scale_coord(1, result_scale-point.scale) - 1;

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
    pub fn new(chunk_root_scale: Scale) -> Self {
        SubSpace {
            chunks: HashMap::new(),
            chunk_scale: chunk_root_scale,
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
    //Returns chunk size in scale 0 voxels. 
    pub fn get_chunk_size(&self) -> i32 { 
        ( scale_coord(1, -self.chunk_scale) - 1 )
    }
}