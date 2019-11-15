//! Memory management types.
//!
//! [CpuAccessibleBufferXalloc]: crate::buffer::CpuAccessibleBufferXalloc
//! [XallocMemoryPool]: self::xalloc::XallocMemoryPool
//! [XallocChunkAllocator]: self::xalloc::XallocChunkAllocator
//! [XallocMemoryPoolChunk]: self::xalloc::XallocMemoryPoolChunk
//! [StdHostVisibleMemoryTypePool]: https://docs.rs/vulkano/0.16.0/vulkano/memory/pool/struct.StdHostVisibleMemoryTypePool.html
//! [SysTlsf]: https://docs.rs/xalloc/0.2.6/xalloc/tlsf/type.SysTlsf.html
//!
//! # Overview
//!
//! tl;dr - [XallocMemoryPool] is a managed memory pool that implements `MemoryPool` so you can use
//! it for vulkano types.
//!
//! The memory management hierarchy is as follows:
//!
//! ## Device memory
//!
//! The lowest level of memory abstraction is a memory pool on the GPU hardware. Each pool has
//! its own designated layout, mapping requirements, and memory type (host-visible or not).
//!
//! [XallocMemoryPool] is a vulkano-compatible memory pool, usable by vulkano types. (See
//! [CpuAccessibleBufferXalloc] for an example.) [XallocMemoryPool] manages device memory pools
//! (`vulkano`'s [StdHostVisibleMemoryTypePool]) and creates a [XallocChunkAllocator] to manage each
//! pool. The total number of pools is generally quite low for most applications, since you only
//! need one pool per layout + mapping + type.
//!
//! ## Chunks
//!
//! A chunk represents a region of allocated device memory, i.e. part of a memory pool. Chunk
//! allocation involves querying the GPU for available memory in a pool, and is rather slow (10ms or
//! so) so this should only happen infrequently. That's why we use blocks to split up larger chunks
//! so we can perform fewer slow GPU allocations.
//!
//! [XallocMemoryPoolChunk] represents a chunk of allocated
//! device memory. The **block allocator** allocates small individual regions of memory from a given
//! chunk, on the cpu (much faster!)
//!
//! ## Blocks
//!
//! A block is the highest level of abstraction, representing a region of a chunk, allocated by the
//! block allocator. When you need to initialize a GPU buffer, you'll use the memory defined in one
//! block. (Note: you shouldn't use blocks directly outside of this module; usually everything
//! should be managed by [XallocMemoryPool].) The block allocator used here is `xalloc`'s [SysTlsf].

pub mod xalloc;
pub use self::xalloc::XallocMemoryPool;
