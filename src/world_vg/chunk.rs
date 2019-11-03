//! A single chunk of blocks.


use std::sync::Arc;
use std::collections::HashSet;

use cgmath::Point3;
use vulkano::device::Device;

use geometry::{Mesh, VertexPositionNormalUVColor, VertexGroup, Material};
use util::Transform;
use mesh_simplifier::{MeshSimplifier, QuadFacing};
use memory::pool::AutoMemoryPool;


/// State used for multithreaded chunk loading. Chunk is dirty and needs to be generated.
pub static CHUNK_STATE_DIRTY: usize = 0;
/// State used for multithreaded chunk loading. Chunk is currently being generated.
pub static CHUNK_STATE_WRITING: usize = 1;
/// State used for multithreaded chunk loading. Chunk is finished being generated.
pub static CHUNK_STATE_CLEAN: usize = 2;


/// Struct representing blocks in a 16x16x16 chunk.
///
/// Encoded in axis order, X, Y, Z. (Z coords are consecutive for a given Y coord, etc).
pub struct Chunk {
    pub ids: [u8; 16*16*16],
    pub position: (i32, i32, i32),
    pub dimension_id: u32,
    pub mesh: Mesh
}


impl Chunk {
    /// Constructs a new (empty) chunk.
    pub fn new(position: (i32, i32, i32), dimension_id: u32) -> Chunk {
        Chunk {
            ids: [0; 16*16*16],
            position,
            dimension_id,
            mesh: Mesh::new()
        }
    }


    /// Converts a flat index to (x, y, z) coordinates.
    #[allow(dead_code)]
    pub fn i_to_xyz(i: usize) -> (i32, i32, i32) { (i as i32/(16*16), (i as i32/16) % 16, i as i32 % 16) }


    /// Converts (x, y, z) coordinates to a flat index.
    #[allow(dead_code)]
    pub fn xyz_to_i(x: i32, y: i32, z: i32) -> usize { ((x * 16*16) + (y * 16) + z) as usize }


    #[allow(dead_code)]
    /// Sets a block at the given index.
    pub fn set_at(&mut self, i: usize, id: u8) {
        self.ids[i] = id;
    }


    /// Replaces the data inside a chunk all at once.
    pub fn replace_data(&mut self, data: &[u8; 16*16*16]) {
        self.ids = *data;
    }


    /// Generates a mesh for the chunk, using [MeshSimplifier].
    pub fn generate_mesh(&mut self, device: Arc<Device>, memory_pool: AutoMemoryPool) {
        let quad_lists = MeshSimplifier::generate_mesh(self);

        // get all unique ids and seperate
        let mut unique_ids = HashSet::new();
        for (_, _, list) in quad_lists.iter() {
            for quad in list.iter() {
                unique_ids.insert(quad.block_id);
            }
        }
        unique_ids.remove(&0); // don't generate anything for air

        let mut mesh = Mesh::new();

        // TODO: currently iterates over the whole quad list [# of unique ids] times. for diverse
        // chunks this will get expensive. needs optimization.
        for id in unique_ids.iter() {
            let mut vertices = Vec::new() as Vec<VertexPositionNormalUVColor>;
            let mut indices = Vec::new() as Vec<u32>;
            let mut o = 0;
            for (facing, layer, list) in quad_lists.iter() {
                for quad in list {
                    if quad.block_id != *id { continue; }
                    match facing {
                        QuadFacing::Left => {
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32, quad.x as f32,          (quad.y+quad.h) as f32 ], normal: [ -1.0, 0.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32, (quad.x+quad.w) as f32, (quad.y+quad.h) as f32 ], normal: [ -1.0, 0.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32, (quad.x+quad.w) as f32, quad.y as f32          ], normal: [ -1.0, 0.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32, quad.x as f32,          quad.y as f32          ], normal: [ -1.0, 0.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Right => {
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32 + 1.0, (quad.x+quad.w) as f32, (quad.y+quad.h) as f32 ], normal: [ 1.0, 0.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32 + 1.0, quad.x as f32,          (quad.y+quad.h) as f32 ], normal: [ 1.0, 0.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32 + 1.0, quad.x as f32,          quad.y as f32          ], normal: [ 1.0, 0.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ *layer as f32 + 1.0, (quad.x+quad.w) as f32, quad.y as f32          ], normal: [ 1.0, 0.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Bottom => {
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, *layer as f32, (quad.y+quad.h) as f32 ], normal: [ 0.0, -1.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          *layer as f32, (quad.y+quad.h) as f32 ], normal: [ 0.0, -1.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          *layer as f32, quad.y as f32          ], normal: [ 0.0, -1.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, *layer as f32, quad.y as f32          ], normal: [ 0.0, -1.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Top => {
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          *layer as f32 + 1.0, (quad.y+quad.h) as f32 ], normal: [ 0.0, 1.0, 0.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, *layer as f32 + 1.0, (quad.y+quad.h) as f32 ], normal: [ 0.0, 1.0, 0.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, *layer as f32 + 1.0, quad.y as f32          ], normal: [ 0.0, 1.0, 0.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          *layer as f32 + 1.0, quad.y as f32          ], normal: [ 0.0, 1.0, 0.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Front => {
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          (quad.y+quad.h) as f32, *layer as f32 ], normal: [ 0.0, 0.0, -1.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, (quad.y+quad.h) as f32, *layer as f32 ], normal: [ 0.0, 0.0, -1.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, quad.y as f32,          *layer as f32 ], normal: [ 0.0, 0.0, -1.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          quad.y as f32,          *layer as f32 ], normal: [ 0.0, 0.0, -1.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                        QuadFacing::Back => {
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, (quad.y+quad.h) as f32, *layer as f32 + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ quad.w as f32, 0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          (quad.y+quad.h) as f32, *layer as f32 + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ 0.0,           0.0 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ quad.x as f32,          quad.y as f32,          *layer as f32 + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ 0.0,           quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                            vertices.push(VertexPositionNormalUVColor { position: [ (quad.x+quad.w) as f32, quad.y as f32,          *layer as f32 + 1.0 ], normal: [ 0.0, 0.0, 1.0 ], uv: [ quad.w as f32, quad.h as f32 ], color: [ 1.0, 1.0, 1.0 ] });
                        },
                    }
                    indices.push(0+o); indices.push(1+o); indices.push(2+o);
                    indices.push(2+o); indices.push(3+o); indices.push(0+o);
                    o += 4;
                }
            }
            mesh.vertex_groups.push(Arc::new(VertexGroup::new(vertices, indices, *id as u8, device.clone(), memory_pool.clone())));
            mesh.transform = Transform::from_position(Point3::new(self.position.0 as f32 * 16.0,
                                                                  self.position.1 as f32 * 16.0,
                                                                  self.position.2 as f32 * 16.0));
        }

        mesh.materials.push(Material { albedo_map_name: String::from(""), specular_exponent: 0.0, specular_strength: 0.6 });
        mesh.materials.push(Material { albedo_map_name: String::from("stone"), specular_exponent: 128.0, specular_strength: 1.0 });
        mesh.materials.push(Material { albedo_map_name: String::from("dirt"), specular_exponent: 16.0, specular_strength: 0.5 });
        mesh.materials.push(Material { albedo_map_name: String::from("grass"), specular_exponent: 64.0, specular_strength: 0.7 });

        self.mesh = mesh;
    }
}