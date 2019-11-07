//! Various utility types.


mod aabb;
pub use self::aabb::AABB;
mod transform;
pub use self::transform::Transform;
pub mod logger;
pub mod event;
pub mod config;


use cgmath::Deg;


pub struct Camera {
    /// Field of fiew.
    pub fov: Deg<f32>
}


impl Camera {
    /// Creates a new Camera.
    pub fn new() -> Camera {
        Camera {
            fov: Deg(45.0)
        }
    }
}


pub mod cube {
    use ::geometry::VertexPositionColorAlpha;
    use world::CHUNK_SIZE_F32;


    pub fn generate_chunk_debug_line_vertices(x: i32, y: i32, z: i32, a: f32) -> [VertexPositionColorAlpha; 8] {
        let x = x as f32 * CHUNK_SIZE_F32;
        let y = y as f32 * CHUNK_SIZE_F32;
        let z = z as f32 * CHUNK_SIZE_F32;
        [
            // top
            VertexPositionColorAlpha { position: [ x,                y+CHUNK_SIZE_F32, z+CHUNK_SIZE_F32 ], color: [ 1.0, 1.0, 1.0, a ] },
            VertexPositionColorAlpha { position: [ x+CHUNK_SIZE_F32, y+CHUNK_SIZE_F32, z+CHUNK_SIZE_F32 ], color: [ 1.0, 1.0, 1.0, a ] },
            VertexPositionColorAlpha { position: [ x+CHUNK_SIZE_F32, y+CHUNK_SIZE_F32, z                ], color: [ 1.0, 1.0, 1.0, a ] },
            VertexPositionColorAlpha { position: [ x,                y+CHUNK_SIZE_F32, z                ], color: [ 1.0, 1.0, 1.0, a ] },
            // bottom
            VertexPositionColorAlpha { position: [ x,                y, z+CHUNK_SIZE_F32 ], color: [ 1.0, 1.0, 1.0, a ] },
            VertexPositionColorAlpha { position: [ x+CHUNK_SIZE_F32, y, z+CHUNK_SIZE_F32 ], color: [ 1.0, 1.0, 1.0, a ] },
            VertexPositionColorAlpha { position: [ x+CHUNK_SIZE_F32, y, z                ], color: [ 1.0, 1.0, 1.0, a ] },
            VertexPositionColorAlpha { position: [ x,                y, z                ], color: [ 1.0, 1.0, 1.0, a ] },
        ]
    }


    pub fn generate_chunk_debug_line_indices(offset: u32) -> [u32; 24] {
        let o = offset * 8;
        [
            0+o,  1+o,  1+o,  2+o,  2+o,  3+o, 3+o, 0+o, // top
            0+o,  4+o,  1+o,  5+o,  2+o,  6+o, 3+o, 7+o, // middle
            4+o,  5+o,  5+o,  6+o,  6+o,  7+o, 7+o, 4+o, // bottom
        ]
    }
}
