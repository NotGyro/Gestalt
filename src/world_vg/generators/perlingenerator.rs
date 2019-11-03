//! Simple world generator using perlin noise.


use noise::{NoiseFn, Perlin, Seedable};
use super::WorldGenerator;
use world_vg::Chunk;


/// Simple world generator using perlin noise.
pub struct PerlinGenerator {
    perlin: Perlin,
    scale: f64,
    offset: f64,
    block_type_noise: Perlin,
    block_type_scale: f64,
}


impl PerlinGenerator {
    /// Creates a new `PerlinGenerator`
    pub fn new() -> PerlinGenerator {
        let perlin = Perlin::new();
        perlin.set_seed(1);

        let block_type_noise = Perlin::new();
        perlin.set_seed(50);

        PerlinGenerator {
            perlin,
            scale: 0.008126,
            offset: 0.26378,
            block_type_noise,
            block_type_scale: 0.063647,
        }
    }
}


impl WorldGenerator for PerlinGenerator {
    fn generate(&self, pos: (i32, i32, i32), dimension_id: u32) -> Chunk {
        let mut chunk = Chunk::new(pos, dimension_id);
        let mut data = [0u8; 16*16*16];
        for x in 0..16 {
            for z in 0..16 {
                let height_norm = self.perlin.get([((pos.0*16 + x) as f64 + self.offset) * self.scale, ((pos.2*16 + z) as f64 + self.offset) * self.scale]) / 2.0 + 0.5;
                let height_abs = height_norm as f32 * 32.0;
                for y in 0..16 {
                    if (pos.1 as f32 * 16.0) + y as f32 <= height_abs {
                        let block_type_val = self.block_type_noise.get([((pos.0*16 + x) as f64) * self.block_type_scale, ((pos.2*16 + z) as f64) * self.block_type_scale]) / 2.0 + 0.5;
                        let block_id = ((block_type_val * 3.0) + 1.0) as u8;
                        data[Chunk::xyz_to_i(x, y, z)] =  block_id;
                    }
                }
            }
        }
        chunk.replace_data(&data);

        chunk
    }
}