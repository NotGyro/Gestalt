//! A dimension.


use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicUsize;

use std::collections::HashMap;
use cgmath::{Point3, MetricSpace};
use renderer::{LineRenderQueue, RenderQueue};
use world::chunk::{CHUNK_STATE_DIRTY, CHUNK_STATE_GENERATING};
use world::generators::ChunkGenerator;
use world::Chunk;


/// A dimension.
pub struct Dimension {
    /// HashMap<chunk position, (chunk, chunk state)>
    pub chunks: Arc<RwLock< HashMap<(i32, i32, i32), (Arc<RwLock<Chunk>>, Arc<AtomicUsize>)> >>
}


impl Dimension {
    pub fn new() -> Dimension {
        Dimension {
            chunks: Arc::new(RwLock::new(HashMap::new())),
        }
    }


    /// Adds new chunks as the player moves closer to them.
    pub fn load_chunks(&mut self, player_pos: Point3<f32>, queue_arc: Arc<RwLock<RenderQueue>>) {
        const CHUNK_RADIUS: i32 = 2;
        const CHUNK_DISTANCE: f32 = CHUNK_RADIUS as f32 * 64.0;

        let gen = ::world::generators::PerlinGenerator::new(0); // TODO: use seed
        let player_x_in_chunks = (player_pos.x / 64.0) as i32;
        let player_y_in_chunks = (player_pos.y / 64.0) as i32;
        let player_z_in_chunks = (player_pos.z / 64.0) as i32;
        for cx in (player_x_in_chunks-CHUNK_RADIUS)..(player_x_in_chunks+CHUNK_RADIUS+1) {
            for cy in (player_y_in_chunks-CHUNK_RADIUS)..(player_y_in_chunks+CHUNK_RADIUS+1) {
                for cz in (player_z_in_chunks-CHUNK_RADIUS)..(player_z_in_chunks+CHUNK_RADIUS+1) {
                    let chunk_pos = (cx as i32, cy as i32, cz as i32);
                    {
                        let lock = self.chunks.read().unwrap();
                        if lock.contains_key(&chunk_pos) {
                            continue;
                        }
                    }

                    let chunk_world_pos = Point3::new(cx as f32 * 64.0 + 32.0,
                                                      cy as f32 * 64.0 + 32.0,
                                                      cz as f32 * 64.0 + 32.0);
                    let dist = Point3::distance(chunk_world_pos, player_pos);
                    if dist < CHUNK_DISTANCE {
                        let chunks_arc = self.chunks.clone();
                        std::thread::spawn(move || {
                            let mut chunk = Chunk::new(chunk_pos, 0); // TODO: use dimension id
                            let chunk_arc = Arc::new(RwLock::new(chunk));
                            {
                                let mut lock = chunks_arc.write().unwrap();
                                lock.insert(chunk_pos, (chunk_arc.clone(), Arc::new(AtomicUsize::new(CHUNK_STATE_GENERATING))));
                            }
                            let data = gen.generate(chunk_pos);
                            {
                                let mut lock = chunk_arc.write().unwrap();
                                lock.data = data;
                                let old;
                                {
                                    let lock = chunks_arc.read().unwrap();
                                    old = lock.get(&chunk_pos).unwrap().clone();
                                }
                                {
                                    let mut lock = chunks_arc.write().unwrap();
                                    lock.insert(chunk_pos, (old.0, Arc::new(AtomicUsize::new(CHUNK_STATE_DIRTY))));
                                }
                            }
                            {
                                let mut lock = queue_arc.write().unwrap();
                                lock.lines.chunks_changed = true;
                            }
                        });
                        return; // return completely to avoid spawning more threads
                    }
                }
            }
        }
    }


    /// Removes old chunks as the player moves away.
    pub fn unload_chunks(&mut self, player_pos: Point3<f32>, queue: Arc<RwLock<RenderQueue>>) {
        const CHUNK_RADIUS: i32 = 2;
        const CHUNK_DISTANCE: f32 = CHUNK_RADIUS as f32 * 64.0;
        let mut chunks = self.chunks.write().unwrap();
        let old_num = chunks.len();
        chunks.retain(|pos, _| {
            let chunk_center = Point3::new(pos.0 as f32 * 64.0 + 32.0, pos.1 as f32 * 64.0 + 32.0, pos.2 as f32 * 64.0 + 32.0);
            let dist = Point3::distance(chunk_center, player_pos);
            dist < CHUNK_DISTANCE + 4.0 // offset added to prevent load/unload loop on the edge
        });
        if chunks.len() != old_num {
            let mut lock = queue.write().unwrap();
            lock.lines.chunks_changed = true;
        }
    }
}