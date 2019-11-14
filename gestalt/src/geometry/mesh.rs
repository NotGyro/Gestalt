//! A mesh object, made up of a set of vertex groups, a list of associated materials, and a transform.
//!
//! The vertgroup / material separation is necessary because a set of geometry can only be rendered
//! with one material at a time, so meshes with multiple materials are broken into multiple vertex groups.

use std::sync::Arc;

use crate::geometry::{VertexGroup, Material};
use crate::renderer::ChunkRenderQueueEntry;
use crate::util::Transform;


/// A mesh object, made up of a set of vertex groups, a list of associated materials, and a transform.
///
/// See [module-level documentation](self).
#[derive(Debug)]
pub struct Mesh {
    pub transform: Transform,
    pub vertex_groups: Vec<Arc<VertexGroup>>,
    pub materials: Vec<Material>
}


impl Mesh {
    /// Creates a new mesh with an identity transform and no geometry or materials.
    pub fn new() -> Mesh {
        Mesh {
            transform: Transform::new(),
            vertex_groups: Vec::new(),
            materials: Vec::new(),
        }
    }


    /// Returns a render queue object with the information necessary to render the mesh.
    ///
    /// Stored in [Renderer.chunk_mesh_queue](::renderer::Renderer::render_queue) and used in
    /// [ChunkRenderPipeline](::pipeline::chunk_pipeline::ChunkRenderPipeline).
    pub fn queue(&self) -> Vec<ChunkRenderQueueEntry> {
        let mut result = Vec::new();
        for vg in self.vertex_groups.iter() {
            result.push(ChunkRenderQueueEntry {
                vertex_group: vg.clone(),
                material: self.materials[vg.material_id as usize].clone(),
                transform: self.transform.to_matrix()
            });
        }
        result
    }
}