//! A dimension.


use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicUsize;

use std::collections::HashMap;
use cgmath::{Point3, MetricSpace};
use renderer::RenderQueue;
use world::chunk::{CHUNK_STATE_DIRTY, CHUNK_STATE_GENERATING};
use world::generators::ChunkGenerator;
use world::Chunk;
use world::CHUNK_SIZE_F32;


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
        const CHUNK_RADIUS: i32 = 3;

        let gen = ::world::generators::PerlinGenerator::new(0); // TODO: use seed
        let player_x_in_chunks = (player_pos.x / CHUNK_SIZE_F32) as i32;
        let player_y_in_chunks = (player_pos.y / CHUNK_SIZE_F32) as i32;
        let player_z_in_chunks = (player_pos.z / CHUNK_SIZE_F32) as i32;

        let mut nearby_positions = Vec::new();

        // check chunks is CHUNK_RADIUS
        for cx in (player_x_in_chunks-CHUNK_RADIUS)..(player_x_in_chunks+CHUNK_RADIUS+1) {
            for cy in (player_y_in_chunks - CHUNK_RADIUS)..(player_y_in_chunks + CHUNK_RADIUS + 1) {
                for cz in (player_z_in_chunks - CHUNK_RADIUS)..(player_z_in_chunks + CHUNK_RADIUS + 1) {
                    nearby_positions.push((cx as i32, cy as i32, cz as i32));
                }
            }
        }

        // sort by closest to player
        nearby_positions.sort_by(|a, b| {
            let a_world = Chunk::chunk_pos_to_center_ws(*a);
            let b_world = Chunk::chunk_pos_to_center_ws(*b);
            let pdist_a = Point3::distance(Point3::new(a_world.0, a_world.1, a_world.2), player_pos);
            let pdist_b = Point3::distance(Point3::new(b_world.0, b_world.1, b_world.2), player_pos);
            pdist_a.partial_cmp(&pdist_b).unwrap()
        });

        // starting with closest
        for chunk_pos in nearby_positions {
            {
                let lock = self.chunks.read().unwrap();
                // if the chunk pos already exists, the chunk has already begun (or completed) processing
                if lock.contains_key(&chunk_pos) {
                    continue;
                }
            }
            let chunks_arc = self.chunks.clone();
            std::thread::spawn(move || {
                let chunk = Chunk::new(chunk_pos, 0); // TODO: use dimension id
                let chunk_arc = Arc::new(RwLock::new(chunk));
                {
                    let mut lock = chunks_arc.write().unwrap();
                    lock.insert(chunk_pos, (chunk_arc.clone(), Arc::new(AtomicUsize::new(CHUNK_STATE_GENERATING))));
                }
                let data = gen.generate(chunk_pos);
                {
                    let mut lock = chunk_arc.write().unwrap();
                    lock.data = data;
                    {
                        let mut lock = chunks_arc.write().unwrap();
                        match lock.get(&chunk_pos) {
                            Some(x) => {
                                let old = x.clone();
                                lock.insert(chunk_pos, (old.0, Arc::new(AtomicUsize::new(CHUNK_STATE_DIRTY))));
                            }
                            None => {
                                // chunk destroyed on another thread
                            }
                        }
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


    /// Removes old chunks as the player moves away.
    pub fn unload_chunks(&mut self, player_pos: Point3<f32>, queue: Arc<RwLock<RenderQueue>>) {
        const CHUNK_RADIUS: i32 = 3;
        const CHUNK_DISTANCE: f32 = CHUNK_RADIUS as f32 * CHUNK_SIZE_F32;
        let mut chunks = self.chunks.write().unwrap();
        let old_num = chunks.len();
        chunks.retain(|pos, _| {
            let center = Chunk::chunk_pos_to_center_ws((pos.0, pos.1, pos.2));
            let dist = Point3::distance(Point3::new(center.0 as f32, center.1 as f32, center.2 as f32), player_pos);
            dist < CHUNK_DISTANCE + 4.0 // offset added to prevent load/unload loop on the edge
        });
        if chunks.len() != old_num {
            let mut lock = queue.write().unwrap();
            lock.lines.chunks_changed = true;
        }
    }
}