use cgmath::{Point3, Quaternion, Vector3, Matrix4, EuclideanSpace};


/// A 3D transform, with position, rotation, and scale.
#[derive(Clone, Debug)]
pub struct Transform {
    pub position: Point3<f32>,
    pub rotation: Quaternion<f32>,
    pub scale: Vector3<f32>
}


#[allow(dead_code)]
impl Transform {
    /// Creates an identity transform.
    pub fn identity() -> Transform {
        Transform {
            position: Point3::new(0.0, 0.0, 0.0),
            rotation: Quaternion::from_sv(0.0, Vector3::new(0.0, 0.0, 0.0)),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    /// Creates a transform with the given position.
    pub fn from_position(position: Point3<f32>) -> Transform {
        Transform {
            position,
            rotation: Quaternion::from_sv(0.0, Vector3::new(0.0, 0.0, 0.0)),
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    /// Creates a transform with the given rotation.
    pub fn from_rotation(rotation: Quaternion<f32>) -> Transform {
        Transform {
            position: Point3::new(0.0, 0.0, 0.0),
            rotation,
            scale: Vector3::new(1.0, 1.0, 1.0),
        }
    }

    /// Creates a transform with the given scale.
    pub fn from_scale(scale: Vector3<f32>) -> Transform {
        Transform {
            position: Point3::new(0.0, 0.0, 0.0),
            rotation: Quaternion::from_sv(0.0, Vector3::new(0.0, 0.0, 0.0)),
            scale
        }
    }

    /// Creates a transform with the given uniform scale (scaling all axes by the same amount).
    pub fn from_uniform_scale(scale: f32) -> Transform {
        Transform {
            position: Point3::new(0.0, 0.0, 0.0),
            rotation: Quaternion::from_sv(0.0, Vector3::new(0.0, 0.0, 0.0)),
            scale: Vector3::new(scale, scale, scale),
        }
    }

    /// Generates a 4x4 transformation matrix from this transform.
    pub fn to_matrix(&self) -> Matrix4<f32> {
        Matrix4::from_translation(self.position.to_vec())
            * Matrix4::from(self.rotation)
            * Matrix4::from_nonuniform_scale(self.scale.x, self.scale.y, self.scale.z)
    }
}