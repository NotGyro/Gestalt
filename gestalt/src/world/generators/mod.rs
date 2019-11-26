//! World voxel data generators.

use cgmath::{Vector3, MetricSpace};

use crate::voxel::array_storage::{ArrayStorageType, CHUNK_SIZE, CHUNK_ELEMENTS, xyz_to_index};
use toolbox::noise::OctavePerlinNoise;

const CHUNK_SIZE_F32: f32 = CHUNK_SIZE as f32;


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
    fn generate(&self, pos: (i32, i32, i32)) -> ArrayStorageType;
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
    fn generate(&self, _pos: (i32, i32, i32)) -> ArrayStorageType {
        let mut data = [0u8; CHUNK_ELEMENTS];

        let center = Vector3::new(CHUNK_SIZE_F32 / 2.0, CHUNK_SIZE_F32 / 2.0, CHUNK_SIZE_F32 / 2.0);

        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let pos = Vector3::new(x as f32, y as f32, z as f32);
                    let dist = pos.distance(center);
                    if dist < 10.0 {
                        data[xyz_to_index(x, y, z)] = 1;
                    }
                }
            }
        }

        data
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
    fn generate(&self, pos: (i32, i32, i32)) -> ArrayStorageType {
        let mut data = [0u8; CHUNK_ELEMENTS];

        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let height_norm = self.perlin.value(
                        pos.0 as f32 * CHUNK_SIZE_F32 + x as f32,
                        pos.2 as f32 * CHUNK_SIZE_F32 + z as f32)
                    / 2.0 + 0.5;
                let height_abs = height_norm as f32 * 32.0;

                for y in 0..CHUNK_SIZE {
                    if (pos.1 as f32 * CHUNK_SIZE_F32) + y as f32 <= height_abs {
                        if (pos.1 as f32 * CHUNK_SIZE_F32) + y as f32 + 1.0 > height_abs {
                            data[xyz_to_index(x, y, z)] = GRASS_ID;
                        }
                        else if (pos.1 as f32 * CHUNK_SIZE_F32) + y as f32 + 4.0 > height_abs {
                            data[xyz_to_index(x, y, z)] = DIRT_ID;
                        }
                        else {
                            data[xyz_to_index(x, y, z)] = STONE_ID;
                        }
                    }
                }
            }
        }

        data
    }
}