use noise::{Perlin, Seedable, NoiseFn};

struct Octave {
    pub generator: Perlin,
    pub scale: f32,
    pub influence: f32,
}

pub struct OctavePerlinNoise {
    octaves: Vec<Octave>
}

impl OctavePerlinNoise {
    pub fn new(seed: u32, num_octaves: u8, spread: f32, persistence: f32) -> Self {
        let mut octaves = Vec::new();
        for i in 0..num_octaves {
            let seed_u64: u64 = seed as u64 + 19u64.pow(i as u32);
            let p = Perlin::new();
            p.set_seed(seed_u64 as u32);
            octaves.push(Octave {
                generator: p,
                scale: 0.0071 * spread.powf(i as f32),
                influence: persistence.powf(i as f32),
            });
        }
        Self { octaves }
    }

    pub fn value(&self, x: f32, y: f32) -> f32 {
        let mut sum: f32 = 0.0;
        for o in self.octaves.iter() {
            let (scaled_x, scaled_y) = (x * o.scale + o.scale * 100.0, y * o.scale - o.scale * 100.0);
            sum += o.generator.get([scaled_x as f64, scaled_y as f64]) as f32 * o.influence;
        }
        sum
    }
}