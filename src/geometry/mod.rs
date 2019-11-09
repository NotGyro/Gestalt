//! Geometry related types.
//!
//! Includes mesh, vertex, and vertexgroup types. `Material` is here too, because I couldn't find a
//! better place for it.


pub mod mesh;
pub mod vertex;
pub mod vertexgroup;

pub use self::mesh::Mesh;
pub use self::vertex::{VertexPositionNormalUVColor, VertexPositionColorAlpha, VertexPosition, VertexPositionUV};
pub use self::vertexgroup::VertexGroup;


/// Shader parameters for a given material.
#[derive(Clone, Debug)]
pub struct Material {
    /// Name of albedo map, used to look up texture in the [TextureRegistry](::registry::TextureRegistry).
    pub albedo_map_name: String,
    /// Exponent used in specular lighting calculation. Higher values have sharper highlights.
    pub specular_exponent: f32,
    /// Intensity of specular highlights.
    pub specular_strength: f32
}
