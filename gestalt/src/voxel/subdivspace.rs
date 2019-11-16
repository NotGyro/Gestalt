//! A dimension.

use parking_lot::RwLock;
use std::sync::Arc;
use std::error::Error;
use std::fmt;
use std::collections::HashMap;
use cgmath::{Point3};

use crate::voxel::{
    voxelstorage::*,
    voxelmath::*,
    subdivmath::*,
    subdivstorage::*
};
use crate::world::CHUNK_SCALE;


/// An error reported upon trying to get or set a voxel which is not currently loaded. 
#[derive(Debug)]
#[allow(dead_code)]
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
pub struct SubdivSpace<C> {
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

/// The "Point" argument here must have a scale equal to the chunk's scale.
#[allow(dead_code)]
pub fn chunkpos_to_block(point: OctPos<i32>, block_scale: Scale) -> OctPos<i32> {
    point.scale_to(block_scale)
}

/// The "Point" argument here must have a scale equal to the chunk's scale.
#[allow(dead_code)]
pub fn chunkpos_to_center(point: OctPos<i32>, result_scale: Scale) -> Point3<f32> {
    let block_pos = chunkpos_to_block(point, result_scale);
    //How many blocks of "result_scale" make up our chunk?
    let chunk_size : i32 = scale_coord(1, result_scale-point.scale);

    Point3::new(block_pos.pos.x as f32 + (chunk_size as f32 * 0.5), 
        block_pos.pos.y as f32 + (chunk_size as f32 * 0.5), 
        block_pos.pos.z as f32 + (chunk_size as f32 * 0.5))
}

impl<L,D,C> SubdivSource<SubdivNode<L, D>, i32> for SubdivSpace<C>
        where L : Voxel,  D : Voxel, C : OctreeSource<L,D,i32> {

    fn get(&self, coord: OctPos<i32>) -> Result<SubdivNode<L, D>, SubdivError> {
        Ok(self.get_details(coord)?.0)
    }

    fn get_max_scale(&self) -> Scale { self.chunk_scale }
    fn get_min_scale(&self) -> Scale { -128 }
}

impl<L,D,C> OctreeSource<L, D, i32> for SubdivSpace<C>
        where L : Voxel,  D : Voxel, C : OctreeSource<L,D,i32> {
    fn get_details(&self, coord: OctPos<i32>) -> Result<(SubdivNode<L, D>, Scale), SubdivError> {
        let chunkpos = blockpos_to_chunk(coord, self.chunk_scale);
        // Do we have a chunk that would contain this block position?
        match self.chunks.get(&chunkpos.pos) {
            Some(chunk_entry_arc) => {
                let chunk_entry = chunk_entry_arc.clone();
                let chunk_start_scaled = chunkpos.scale_to(coord.scale);
                let chunk_size = self.get_chunk_size(coord.scale);
                let bounds : VoxelRange<i32> = VoxelRange{ lower: chunk_start_scaled.pos, 
                                upper: VoxelPos{ x: chunk_start_scaled.pos.x + chunk_size, 
                                                y: chunk_start_scaled.pos.y + chunk_size, 
                                                z: chunk_start_scaled.pos.z + chunk_size } };
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
impl<L, C> SubdivDrain<L, i32> for SubdivSpace<C>
        where L : Voxel, C : SubdivDrain<L, i32> {
    fn set(&mut self, coord: OctPos<i32>, value: L) -> Result<(), SubdivError> {
        let chunkpos = blockpos_to_chunk(coord, self.chunk_scale);
        let chunk_size = self.get_chunk_size(coord.scale);
        // Do we have a chunk that would contain this block position?
        match self.chunks.get_mut(&chunkpos.pos) {
            Some(chunk_entry_arc) => {
                let chunk_start_scaled = chunkpos.scale_to(coord.scale);
                let bounds : VoxelRange<i32> = VoxelRange{ lower: chunk_start_scaled.pos, 
                                upper: VoxelPos{ x: chunk_start_scaled.pos.x + chunk_size, 
                                                y: chunk_start_scaled.pos.y + chunk_size, 
                                                z: chunk_start_scaled.pos.z + chunk_size } };
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

#[allow(dead_code)]
impl<C> SubdivSpace<C> {
    #[allow(dead_code)]
    pub fn new() -> Self {
        SubdivSpace {
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

    /// Returns chunk size in scale block_scl voxels. 
    pub fn get_chunk_size(&self, block_scl: Scale) -> i32 {
        scale_coord(1, block_scl-self.chunk_scale)
    }
    pub fn load_new_chunk(&mut self, chunk_pos : VoxelPos<i32>, chunk: C) {
        self.chunks.insert(chunk_pos, Arc::new( RwLock::new(chunk) ));
    }
}

#[test]
fn test_subdiv_space() {
    use crate::world::TileID;
    use string_cache::DefaultAtom as Atom; 
    use crate::world::tile::*;

    let air_id = TILE_REGISTRY.lock().register_tile(&Atom::from("air"));
    let stone_id = TILE_REGISTRY.lock().register_tile(&Atom::from("stone"));
    let lava_id = TILE_REGISTRY.lock().register_tile(&Atom::from("lava"));

    let mut world : SubdivSpace<NaiveVoxelOctree<TileID, ()>> = SubdivSpace::new();

    assert_eq!(world.get_chunk_size(0), 32);

    let mut chunk : NaiveVoxelOctree<TileID, ()> = NaiveVoxelOctree::new(stone_id.clone(), CHUNK_SCALE);
    chunk.set(opos!((1,0,1) @ 3), air_id).unwrap();
    chunk.set(opos!((0,0,1) @ 3), air_id).unwrap();
    chunk.set(opos!((3,0,0) @ 2), air_id).unwrap();
    
    let mut chunk2 : NaiveVoxelOctree<TileID, ()> = NaiveVoxelOctree::new(air_id.clone(), CHUNK_SCALE);
    chunk2.set(opos!((0,0,0) @ CHUNK_SCALE), lava_id ).unwrap();
    
    let mut chunk3 : NaiveVoxelOctree<TileID, ()> = NaiveVoxelOctree::new(air_id.clone(), CHUNK_SCALE);

    let chunk_1_pos = vpos!(0,0,0);
    let chunk_2_pos = vpos!(1,0,0);
    let chunk_3_pos = vpos!(2,0,0);
    world.load_new_chunk(chunk_1_pos, chunk);
    world.load_new_chunk(chunk_2_pos, chunk2);
    world.load_new_chunk(chunk_3_pos, chunk3);
    assert_eq!(world.get(opos!((40,0,0) @ 0)).unwrap(), SubdivNode::Leaf(lava_id) );
    assert_eq!(world.get(opos!((65,0,0) @ 0)).unwrap(), SubdivNode::Leaf(air_id) );
    assert_eq!(world.get(opos!((9,0,9) @ 0)).unwrap(), SubdivNode::Leaf(air_id) );
    assert_eq!(world.get(opos!((3,3,3) @ 0)).unwrap(), SubdivNode::Leaf(stone_id) );
}