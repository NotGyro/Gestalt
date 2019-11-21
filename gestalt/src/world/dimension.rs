//! A dimension.

use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicUsize, AtomicU32, Ordering};
use std::collections::HashMap;
use cgmath::{Point3, MetricSpace};

use crate::world::chunk::{CHUNK_STATE_DIRTY, CHUNK_STATE_GENERATING};
use crate::world::{
    Chunk, CHUNK_SIZE_F32,
    generators::ChunkGenerator,
};
use phosphor::renderer::RenderInfo;


/// A dimension.
pub struct Dimension {
    /// HashMap<chunk position, (chunk, chunk state)>
    pub chunks: Arc<RwLock< HashMap<(i32, i32, i32), (Arc<RwLock<Chunk>>, Arc<AtomicUsize>)> >>,
    next_chunk_id: Arc<AtomicU32>
}


/// Radius in meters around the player to load chunks.
const CHUNK_DISTANCE: f32 = 192.0;
/// Radius in chunks.
const CHUNK_RADIUS: i32 = (CHUNK_DISTANCE / crate::world::CHUNK_SIZE_F32) as i32;
/// Offset added to distance before removing chunks. (Must be > 0 to prevent load/unload loops)
const UNLOAD_OFFSET: f32 = 48.0;

pub const TEST_SEED: u32 = 0;


impl Dimension {
    pub fn new() -> Dimension {
        Dimension {
            chunks: Arc::new(RwLock::new(HashMap::new())),
            // start at 1 and skip 0 during check since its used as the clear value
            next_chunk_id: Arc::new(AtomicU32::new(1))
        }
    }


    /// Adds new chunks as the player moves closer to them.
    pub fn load_chunks(&mut self, player_pos: Point3<f32>, info: &RenderInfo) {
        let gen = crate::world::generators::PerlinGenerator::new(TEST_SEED); // TODO: use seed
        let player_x_in_chunks = (player_pos.x / CHUNK_SIZE_F32) as i32;
        let player_y_in_chunks = (player_pos.y / CHUNK_SIZE_F32) as i32;
        let player_z_in_chunks = (player_pos.z / CHUNK_SIZE_F32) as i32;

        let mut nearby_positions = Vec::new();

        // check chunks is CHUNK_RADIUS
        for cx in (player_x_in_chunks-CHUNK_RADIUS)..(player_x_in_chunks+CHUNK_RADIUS+1) {
            for cy in (player_y_in_chunks - CHUNK_RADIUS)..(player_y_in_chunks + CHUNK_RADIUS + 1) {
                for cz in (player_z_in_chunks - CHUNK_RADIUS)..(player_z_in_chunks + CHUNK_RADIUS + 1) {
                    let chunk_pos = (cx as i32, cy as i32, cz as i32);
                    let chunk_world = Chunk::chunk_pos_to_center_ws(chunk_pos);
                    let dist = Point3::distance(Point3::new(chunk_world.0, chunk_world.1, chunk_world.2), player_pos);
                    if dist < CHUNK_DISTANCE {
                        // only add chunks if they're close enough in a circular radius, not a square
                        nearby_positions.push(chunk_pos);
                    }
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
            let id_arc = self.next_chunk_id.clone();
            let info_arc = info.clone();
            std::thread::spawn(move || {
                let id = id_arc.fetch_add(1, Ordering::Relaxed);
                let chunk = Chunk::new(id, chunk_pos, 0); // TODO: use dimension id
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
                    let mut lock = info_arc.render_queues.write().unwrap();
                    lock.lines.chunks_changed = true;
                }
            });
            return; // return completely to avoid spawning more threads
        }
    }


    /// Removes old chunks as the player moves away.
    pub fn unload_chunks(&mut self, player_pos: Point3<f32>, info: &RenderInfo) {
        let mut chunks = self.chunks.write().unwrap();
        let old_num = chunks.len();
        chunks.retain(|pos, _| {
            let center = Chunk::chunk_pos_to_center_ws((pos.0, pos.1, pos.2));
            let dist = Point3::distance(Point3::new(center.0 as f32, center.1 as f32, center.2 as f32), player_pos);
            dist < CHUNK_DISTANCE + UNLOAD_OFFSET // offset added to prevent load/unload loop on the edge
        });
        if chunks.len() != old_num {
            let mut lock = info.render_queues.write().unwrap();
            lock.lines.chunks_changed = true;
        }
    }
}


/// Global dimension registry.
pub struct DimensionRegistry {
    pub dimensions: HashMap<u32, Dimension>
}


impl DimensionRegistry {
    pub fn new() -> DimensionRegistry {
        DimensionRegistry {
            dimensions: HashMap::new()
        }
    }


    /// Gets the dimension with the given id, or None if one couldn't be found.
    pub fn get(&mut self, id: u32) -> Option<&mut Dimension> {
        self.dimensions.get_mut(&id)
    }
}