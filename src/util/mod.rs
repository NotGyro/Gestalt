//! Various utility types.


mod aabb;
pub use self::aabb::AABB;
mod transform;
pub use self::transform::Transform;
pub mod logger;
pub mod event;
pub mod config;


use cgmath::{Deg, Vector3, Point3, EuclideanSpace, dot, Matrix4};
use cgmath::Transform as _;


pub struct Camera {
    /// Field of fiew. Note that this is the horizontal half-angle, i.e. fov = 45 means a 90 degree horizontal FOV.
    pub fov: Deg<f32>
}


impl Camera {
    /// Creates a new Camera.
    pub fn new() -> Camera {
        Camera {
            fov: Deg(45.0) // 90 degrees
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Plane {
    n: Vector3<f32>,
    d: f32
}

#[derive(Clone, Debug)]
pub struct FrustumPlanes {
    pub left: Plane,
    pub right: Plane,
    pub bottom: Plane,
    pub top: Plane,
    pub front: Plane,
    pub rear: Plane,
}


pub fn view_to_frustum(pitch: f32, yaw: f32, fov: Deg<f32>, aspect: f32, near_z: f32, far_z: f32) -> FrustumPlanes {
    let forward = Vector3::new(0.0, 0.0, 1.0);

    let neg_yaw = -yaw + 180.0 - 90.0 - fov.0 + 5.0;
    let pos_yaw = -yaw + 180.0 + 90.0 + fov.0 - 5.0;
    let neg_pitch = -pitch - 90.0 - (fov.0 / aspect) + 5.0;
    let pos_pitch = -pitch + 90.0 + (fov.0 / aspect) - 5.0;

    let norm_left = (Matrix4::from_angle_y(Deg(pos_yaw)) * Matrix4::from_angle_x(Deg(pitch))).transform_vector(forward);
    let norm_right = (Matrix4::from_angle_y(Deg(neg_yaw)) * Matrix4::from_angle_x(Deg(pitch))).transform_vector(forward);

    let norm_bottom = (Matrix4::from_angle_y(Deg(-yaw + 180.0)) * Matrix4::from_angle_x(Deg(neg_pitch))).transform_vector(forward);
    let norm_top    = (Matrix4::from_angle_y(Deg(-yaw + 180.0)) * Matrix4::from_angle_x(Deg(pos_pitch))).transform_vector(forward);

    let left   = Plane { n: norm_left,   d: 0.0 };
    let right  = Plane { n: norm_right,  d: 0.0 };
    let bottom = Plane { n: norm_bottom, d: 0.0 };
    let top    = Plane { n: norm_top,    d: 0.0 };
    let front  = Plane { n: Vector3::new(0.0, 0.0, -1.0), d: near_z };
    let rear   = Plane { n: Vector3::new(0.0, 0.0,  1.0), d: far_z };

    FrustumPlanes { left, right, bottom, top, front, rear }
}

#[allow(dead_code)]
pub fn aabb_plane_intersection(bmin: Point3<f32>, bmax: Point3<f32>, p: Plane) -> bool {
    // Convert AABB to center-extents representation
    let c = (bmax + bmin.to_vec()) * 0.5; // Compute AABB center
    let e = bmax - c.to_vec(); // Compute positive extents

    // Compute the projection interval radius of b onto L(t) = b.c + t * p.n
    let r = e.x*((p.n.x).abs()) + e.y*((p.n.y).abs()) + e.z*((p.n.z).abs());

    // Compute distance of box center from plane
    let s = dot(p.n, c.to_vec()) - p.d;

    // Intersection occurs when distance s falls within [-r,+r] interval
    s.abs() <= r
}

pub fn aabb_frustum_intersection(bmin: Point3<f32>, bmax: Point3<f32>, p: FrustumPlanes) -> bool {
    for plane in &[p.left, p.right, p.top, p.bottom] {
        let mut closest_pt = Vector3::new(0.0, 0.0, 0.0);

        closest_pt.x = if plane.n.x > 0.0 { bmin.x } else { bmax.x };
        closest_pt.y = if plane.n.y > 0.0 { bmin.y } else { bmax.y };
        closest_pt.z = if plane.n.z > 0.0 { bmin.z } else { bmax.z };

        if dot(plane.n, closest_pt) > 0.0 {
            return false;
        }
    }
    true
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
