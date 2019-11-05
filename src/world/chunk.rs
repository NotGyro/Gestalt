//! A single chunk of blocks.

use std::sync::Arc;

use vulkano::device::Device;

use geometry::Mesh;
use memory::pool::AutoMemoryPool;
use voxel::subdivstorage::{NaiveVoxelOctree, NaiveOctreeNode};


/// State used for multithreaded chunk loading. Chunk is dirty and needs to be generated.
pub static CHUNK_STATE_DIRTY: usize = 0;
/// State used for multithreaded chunk loading. Chunk is currently being generated.
pub static CHUNK_STATE_WRITING: usize = 1;
/// State used for multithreaded chunk loading. Chunk is finished being generated.
pub static CHUNK_STATE_CLEAN: usize = 2;


pub struct Chunk {
    pub data: NaiveVoxelOctree<u8, ()>,
    pub position: (i32, i32, i32),
    pub dimension_id: u32,
    pub mesh: Mesh
}


impl Chunk {
    /// Constructs a new (empty) chunk.
    pub fn new(position: (i32, i32, i32), dimension_id: u32) -> Chunk {
        Chunk {
            data: NaiveVoxelOctree{scale : 6 , root: NaiveOctreeNode::new_leaf(0)},
            position,
            dimension_id,
            mesh: Mesh::new()
        }
    }

    /// Generates a mesh for the chunk, using [::octree_mesher::OctreeMesher].
    pub fn generate_mesh(&mut self, device: Arc<Device>, memory_pool: AutoMemoryPool) {
        ::octree_mesher::OctreeMesher::generate_mesh(self, device, memory_pool);
    }
}