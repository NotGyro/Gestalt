//! World voxel data generators.

use cgmath::{Vector3, MetricSpace};

use crate::voxel::subdivstorage::{NaiveVoxelOctree, NaiveOctreeNode, SubdivVoxelDrain};
use crate::world::{CHUNK_SIZE, CHUNK_SIZE_F32, CHUNK_SCALE};
use crate::util::noise::OctavePerlinNoise;


/// Trait for world voxel generators.
pub trait ChunkGenerator {
    /// Generate a chunk.
    ///
    /// # Arguments
    ///
    /// * `pos` - Position in chunks.
    ///
    /// # Returns
    ///
    /// The octree generated for the given chunk.
    fn generate(&self, pos: (i32, i32, i32)) -> NaiveVoxelOctree<u8, ()>;
}


#[allow(dead_code)]
/// Test generator. Generates a sphere in every chunk.
pub struct SphereGenerator {
    seed: u32,
}

#[allow(dead_code)]
impl SphereGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            seed
        }
    }
}

#[allow(dead_code)]
impl ChunkGenerator for SphereGenerator {
    fn generate(&self, _pos: (i32, i32, i32)) -> NaiveVoxelOctree<u8, ()> {
        let mut tree = NaiveVoxelOctree{scale: CHUNK_SCALE , root: NaiveOctreeNode::new_leaf(0)};

        let center = Vector3::new(CHUNK_SIZE_F32 / 2.0, CHUNK_SIZE_F32 / 2.0, CHUNK_SIZE_F32 / 2.0);

        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let pos = Vector3::new(x as f32, y as f32, z as f32);
                    let dist = pos.distance(center);
                    if dist < 10.0 {
                        tree.set(opos!((x, y, z) @ 0), 1).unwrap();
                    }
                }
            }
        }

        tree
    }
}


/// Simple perlin noise terrain generator.
pub struct PerlinGenerator {
    /// perlin noise generator
    perlin: OctavePerlinNoise,
}

impl PerlinGenerator {
    /// Creates a new PerlinGenerator.
    ///
    /// # Arguments
    ///
    /// * `seed` - The random seed for this generator.
    pub fn new(seed: u32) -> PerlinGenerator {
        let perlin = OctavePerlinNoise::new(seed, 5, 1.7, 0.45);

        PerlinGenerator {
            perlin,
        }
    }
}

const STONE_ID: u8 = 1;
const DIRT_ID: u8 = 2;
const GRASS_ID: u8 = 3;

impl ChunkGenerator for PerlinGenerator {
    /// See [ChunkGenerator]
    fn generate(&self, pos: (i32, i32, i32)) -> NaiveVoxelOctree<u8, ()> {
        let mut tree = NaiveVoxelOctree{scale : CHUNK_SCALE , root: NaiveOctreeNode::new_leaf(0)};

        const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;

        for x in 0..CHUNK_SIZE_I32 {
            for z in 0..CHUNK_SIZE_I32 {
                let height_norm = self.perlin.value(
                        pos.0 as f32 * CHUNK_SIZE_F32 + x as f32,
                        pos.2 as f32 * CHUNK_SIZE_F32 + z as f32)
                    / 2.0 + 0.5;
                let height_abs = height_norm as f32 * 32.0;

                for y in 0..CHUNK_SIZE_I32 {
                    if (pos.1 as f32 * CHUNK_SIZE_F32) + y as f32 <= height_abs {
                        if (pos.1 as f32 * CHUNK_SIZE_F32) + y as f32 + 1.0 > height_abs {
                            tree.set(opos!((x, y, z) @ 0), GRASS_ID).unwrap();
                        }
                        else if (pos.1 as f32 * CHUNK_SIZE_F32) + y as f32 + 4.0 > height_abs {
                            tree.set(opos!((x, y, z) @ 0), DIRT_ID).unwrap();
                        }
                        else {
                            tree.set(opos!((x, y, z) @ 0), STONE_ID).unwrap();
                        }
                    }
                }
            }
        }

        tree
    }
}