//! A dimension.


use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicUsize;

use std::collections::HashMap;
use cgmath::{Point3, MetricSpace};
use renderer::LineRenderQueue;
use world::chunk::CHUNK_STATE_DIRTY;
use world::generators::ChunkGenerator;
use world::Chunk;


/// A dimension.
pub struct Dimension {
    /// HashMap<chunk position, (chunk, chunk state)>
    pub chunks: HashMap<(i32, i32, i32), (Arc<RwLock<Chunk>>, Arc<AtomicUsize>)>
}


impl Dimension {
    pub fn new() -> Dimension {
        Dimension {
            chunks: HashMap::new(),
        }
    }


    /// Adds new chunks as the player moves closer to them, and removes old chunks as the player
    /// moves away.
    pub fn load_unload_chunks(&mut self, player_pos: Point3<f32>, queue: &mut LineRenderQueue) {
        const CHUNK_RADIUS: i32 = 2;
        const CHUNK_DISTANCE: f32 = CHUNK_RADIUS as f32 * 64.0;
        self.chunks.retain(|pos, _| {
            let chunk_center = Point3::new(pos.0 as f32 * 64.0 + 32.0, pos.1 as f32 * 64.0 + 32.0, pos.2 as f32 * 64.0 + 32.0);
            let dist = Point3::distance(chunk_center, player_pos);
            dist < CHUNK_DISTANCE + 4.0 // offset added to prevent load/unload loop on the edge
        });

        let gen = ::world::generators::PerlinGenerator::new(0); // TODO: use seed
        let player_x_in_chunks = (player_pos.x / 64.0) as i32;
        let player_y_in_chunks = (player_pos.y / 64.0) as i32;
        let player_z_in_chunks = (player_pos.z / 64.0) as i32;
        for cx in (player_x_in_chunks-CHUNK_RADIUS)..(player_x_in_chunks+CHUNK_RADIUS+1) {
            for cy in (player_y_in_chunks-CHUNK_RADIUS)..(player_y_in_chunks+CHUNK_RADIUS+1) {
                for cz in (player_z_in_chunks-CHUNK_RADIUS)..(player_z_in_chunks+CHUNK_RADIUS+1) {
                    let chunk_pos = (cx as i32, cy as i32, cz as i32);
                    if self.chunks.contains_key(&chunk_pos) {
                        continue;
                    }

                    let chunk_world_pos = Point3::new(cx as f32 * 64.0 + 32.0,
                                                      cy as f32 * 64.0 + 32.0,
                                                      cz as f32 * 64.0 + 32.0);
                    let dist = Point3::distance(chunk_world_pos, player_pos);
                    if dist < CHUNK_DISTANCE {
                        let mut chunk = Chunk::new(chunk_pos, 0); // TODO: use dimension id
                        chunk.data = gen.generate(chunk_pos);
                        self.chunks.insert(chunk_pos, (Arc::new(RwLock::new(chunk)), Arc::new(AtomicUsize::new(CHUNK_STATE_DIRTY))));
                        queue.chunks_changed = true;
                    }
                }
            }
        }
    }
}