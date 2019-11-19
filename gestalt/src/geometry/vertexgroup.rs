//! A vertex group type, which holds vertex and index buffers and a material id.
//!
//! Material id is a `u8` which corresponds to the index of a material in the owning [Mesh](super::Mesh).

use std::sync::Arc;

use vulkano::buffer::BufferUsage;
use vulkano::device::Device;

use crate::buffer::CpuAccessibleBufferXalloc;
use crate::memory::xalloc::XallocMemoryPool;


/// Vertex group object. Material id is a `u8` which corresponds to the index of a material in the owning [Mesh](super::Mesh).
///
/// See [module-level documentation](self).
#[derive(Debug)]
pub struct VertexGroup<V> {
    /// Vertex buffer. Cpu-accessible, managed by [AutoMemoryPool](::memory::pool::AutoMemoryPool).
    pub vertex_buffer: Arc<CpuAccessibleBufferXalloc<[V]>>,
    /// Index buffer. Cpu-accessible, managed by [AutoMemoryPool](::memory::pool::AutoMemoryPool).
    pub index_buffer: Arc<CpuAccessibleBufferXalloc<[u32]>>,
    pub material_id: u8,
}


impl<V> VertexGroup<V> {
    /// Constructs a new `VertexGroup` with the given parameters.
    pub fn new<Iv, Ii>(verts: Iv, idxs: Ii, material_id: u8, device: Arc<Device>, memory_pool: XallocMemoryPool) -> VertexGroup<V>
            where Iv: ExactSizeIterator<Item=V>, Ii: ExactSizeIterator<Item=u32>, V: 'static {
        VertexGroup {
            vertex_buffer: CpuAccessibleBufferXalloc::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), verts).expect("failed to create vertex buffer"),
            index_buffer: CpuAccessibleBufferXalloc::from_iter(device.clone(), memory_pool.clone(), BufferUsage::all(), idxs).expect("failed to create index buffer"),
            material_id
        }
    }
}