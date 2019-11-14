//! Memory pool types.
//!
//! [AutoMemoryPool] is a memory managed pool that only allocates new chunks of device memory when
//! needed, yielding blocks from existing chunks when possible.


use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, RwLock};
use std::sync::Mutex;

use vulkano::device::Device;
use vulkano::device::DeviceOwned;
use vulkano::instance::MemoryType;
use vulkano::memory::DeviceMemory;
use vulkano::memory::DeviceMemoryAllocError;
use vulkano::memory::MappedDeviceMemory;
use vulkano::memory::pool::AllocLayout;
use vulkano::memory::pool::MappingRequirement;
use vulkano::memory::pool::MemoryPool;
use vulkano::memory::pool::MemoryPoolAlloc;
use vulkano::memory::pool::StdHostVisibleMemoryTypePool;
use vulkano::memory::pool::StdHostVisibleMemoryTypePoolAlloc;
use fnv::FnvHasher;

use super::allocator::{PoolAllocator, BlockAllocator, BlockId};


/// Chunk size for [AutoMemoryPool] in bytes
pub const AUTO_POOL_CHUNK_SIZE: usize = 1024 * 1024 * 128;


/// Inner type for [AutoMemoryPool]. Necessary to implement vulkano's `MemoryPool` on an `Arc<T>`.
#[derive(Debug)]
pub struct AutoMemoryPoolInner {
    device: Arc<Device>,

    /// For each memory type index, stores the associated `PoolAllocator` which manages that pool.
    pools: Arc<Mutex<HashMap<(u32, AllocLayout, MappingRequirement), PoolAllocator, BuildHasherDefault<FnvHasher>>>>,
}


/// Memory managed pool that only allocates new chunks of device memory when needed, yielding blocks
/// from existing chunks when possible.
///
/// Alloc methods are in [impl MemoryPool for AutoMemoryPool](struct.AutoMemoryPool.html#impl-MemoryPool).
#[derive(Debug)]
pub struct AutoMemoryPool(pub Arc<AutoMemoryPoolInner>);


impl Clone for AutoMemoryPool {
    /// `AutoMemoryPool` is just a newtype around `Arc<AutoMemoryPoolInner>`, so it can be easily
    /// cloned.
    fn clone(&self) -> Self {
        AutoMemoryPool(self.0.clone())
    }
}


impl AutoMemoryPool {
    /// Creates a new `AutoMemoryPool`.
    #[inline]
    pub fn new(device: Arc<Device>) -> AutoMemoryPool {
        let cap = device.physical_device().memory_types().len();
        let hasher = BuildHasherDefault::<FnvHasher>::default();

        AutoMemoryPool(Arc::new(AutoMemoryPoolInner {
            device: device.clone(),
            pools: Arc::new(Mutex::new(HashMap::with_capacity_and_hasher(cap, hasher))),
        }))
    }
}

unsafe impl MemoryPool for AutoMemoryPool {
    type Alloc = AutoMemoryPoolBlock;


    /// Provides a block of memory to use, allocating new chunks when all existing chunks are full.
    fn alloc_generic(&self, memory_type: MemoryType, size: usize, alignment: usize,
                     layout: AllocLayout, map: MappingRequirement)
                     -> Result<AutoMemoryPoolBlock, DeviceMemoryAllocError> {
        let mut pools = self.0.pools.lock().unwrap();

        if !memory_type.is_host_visible() {
            panic!("AutoMemoryPool only works with host-visible memory!");
        }

        match pools.entry((memory_type.id(), layout, map)) {
            // existing pool and allocator
            Entry::Occupied(mut entry) => {
                let pool_allocator = entry.get_mut();
                let res = pool_allocator.alloc(size, alignment, &self.0);
                Ok(res)
            },
            // create new pool and allocator
            Entry::Vacant(entry) => {
                let pool = StdHostVisibleMemoryTypePool::new(self.0.device.clone(), memory_type);
                let mut pool_allocator = PoolAllocator::new(pool.clone());
                let block = pool_allocator.alloc(size, alignment, &self.0);
                entry.insert(pool_allocator);
                Ok(block)
            },
        }
    }
}


unsafe impl DeviceOwned for AutoMemoryPool {
    #[inline]
    fn device(&self) -> &Arc<Device> {
        &self.0.device
    }
}


/// Stores information about an allocated chunk of device memory. Blocks are allocated as regions
/// of one of these chunks.
#[derive(Debug)]
pub struct AutoMemoryPoolChunk {
    pub alloc: StdHostVisibleMemoryTypePoolAlloc,
    pub pool: Arc<AutoMemoryPoolInner>,
    pub id: usize
}
impl PartialEq for AutoMemoryPoolChunk {
    fn eq(&self, other: &AutoMemoryPoolChunk) -> bool {
        self.id == other.id
    }
}
impl Eq for AutoMemoryPoolChunk {}
impl ::std::hash::Hash for AutoMemoryPoolChunk {
    fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.id);
    }
}


/// Holds information about a single block of allocated memory.
///
/// Block is automatically freed from its chunk when `drop` is called.
#[derive(Debug)]
pub struct AutoMemoryPoolBlock {
    pub chunk: Arc<AutoMemoryPoolChunk>,
    pub allocator: Arc<RwLock<BlockAllocator>>,
    pub size: usize,
    pub offset: usize,
    pub block_id: BlockId
}
#[allow(dead_code)]
impl AutoMemoryPoolBlock {
    #[inline]
    pub fn size(&self) -> usize { self.size }
}
unsafe impl MemoryPoolAlloc for AutoMemoryPoolBlock {
    #[inline]
    fn mapped_memory(&self) -> Option<&MappedDeviceMemory> { Some(self.chunk.alloc.memory()) }
    #[inline]
    fn memory(&self) -> &DeviceMemory { self.chunk.alloc.memory().as_ref() }
    #[inline]
    fn offset(&self) -> usize { self.chunk.alloc.offset() + self.offset }
}
impl Drop for AutoMemoryPoolBlock {
    fn drop(&mut self) {
        let mut a = self.allocator.write().unwrap();
        a.free(&self.block_id);
    }
}
