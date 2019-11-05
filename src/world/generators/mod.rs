use voxel::subdivmath::OctPos;
use voxel::subdivstorage::{NaiveVoxelOctree, NaiveOctreeNode, SubdivVoxelDrain};
use cgmath::{Vector3, MetricSpace};

pub trait ChunkGenerator {
    fn generate(pos: (u32, u32, u32), seed: u32) -> NaiveVoxelOctree<u32, ()>;
}

pub struct SphereGenerator { }

impl ChunkGenerator for SphereGenerator {
    fn generate(_pos: (u32, u32, u32), _seed: u32) -> NaiveVoxelOctree<u32, ()> {
        let mut tree = NaiveVoxelOctree{scale : 6 , root: NaiveOctreeNode::new_leaf(0)};

        let center = Vector3::new(32.0f32, 32.0, 32.0);

        for x in 0..64 {
            for y in 0..64 {
                for z in 0..64 {
                    let pos = Vector3::new(x as f32, y as f32, z as f32);
                    let dist = pos.distance(center);
                    if dist < 30.0 {
                        tree.set(opos!((x, y, z) @ 0), 1).unwrap();
                    }
                }
            }
        }

        tree
    }
}