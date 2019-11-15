//! A vertex group type, which holds vertex and index buffers and a material id.
//!
//! Material id is a `u8` which corresponds to the index of a material in the owning [Mesh](super::Mesh).

use std::sync::Arc;

use vulkano::buffer::BufferUsage;
use vulkano::device::Device;

use crate::buffer::CpuAccessibleBufferXalloc;
use crate::geometry::PBRPipelineVertex;
use crate::memory::xalloc::XallocMemoryPool;


// TODO: linking vertgroup to material by id field is probably fragile
// TODO: storing vertex data as a Vec *and* in the buffer is probably unnecessary.
/// Vertex group object. Material id is a `u8` which corresponds to the index of a material in the owning [Mesh](super::Mesh).
///
/// See [module-level documentation](self).
#[derive(Debug)]
pub struct VertexGroup {
    /// Vertex data. Set this and call [update_vertex_buffer](VertexGroup::update_vertex_buffer) to update the buffer.
    pub vertices: Vec<PBRPipelineVertex>,
    /// Vertex buffer. Cpu-accessible, managed by [AutoMemoryPool](::memory::pool::AutoMemoryPool).
    pub vertex_buffer: Option<Arc<CpuAccessibleBufferXalloc<[PBRPipelineVertex]>>>,
    /// Index data. Set this and call [update_index_buffer](VertexGroup::update_index_buffer) to update the buffer.
    pub indices: Vec<u32>,
    /// Index buffer. Cpu-accessible, managed by [AutoMemoryPool](::memory::pool::AutoMemoryPool).
    pub index_buffer: Option<Arc<CpuAccessibleBufferXalloc<[u32]>>>,
    /// Corresponds to the index of a material in the owning [Mesh](super::Mesh).
    pub material_id: u8,
}


impl VertexGroup {
    /// Constructs a new `VertexGroup` with the given parameters.
    pub fn new(verts: Vec<PBRPipelineVertex>, idxs: Vec<u32>, mat_id: u8, device: Arc<Device>, memory_pool: XallocMemoryPool) -> VertexGroup {
        let mut group = VertexGroup {
            vertices: verts.to_vec(),
            vertex_buffer: None,
            indices: idxs.to_vec(),
            index_buffer: None,
            material_id: mat_id
        };
        group.update_buffers(device, memory_pool);
        group
    }


    /// Updates both buffers with data from their respective `Vec`s.
    pub fn update_buffers(&mut self, device: Arc<Device>, memory_pool: XallocMemoryPool) {
        self.update_vertex_buffer(device.clone(), memory_pool.clone());
        self.update_index_buffer(device, memory_pool);
    }


    /// Updates the vertex buffer with data from `vertex_buffer`.
    pub fn update_vertex_buffer(&mut self, device: Arc<Device>, memory_pool: XallocMemoryPool) {
        self.vertex_buffer = Some(CpuAccessibleBufferXalloc::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), self.vertices.iter().cloned()).expect("failed to create vertex buffer"));
    }


    /// Updates the index buffer with data from `index_buffer`.
    pub fn update_index_buffer(&mut self, device: Arc<Device>, memory_pool: XallocMemoryPool) {
        self.index_buffer = Some(CpuAccessibleBufferXalloc::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), self.indices.iter().cloned()).expect("failed to create index buffer"));
    }
}