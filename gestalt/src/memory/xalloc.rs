//! Vulkano memory manager using xalloc

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::hash::BuildHasherDefault;
use std::sync::{Arc, RwLock};
use std::sync::Mutex;
use std::mem::MaybeUninit;

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
use xalloc::{TlsfRegion, SysTlsf};
use xalloc::arena::sys;

/// Chunk size for [XallocMemoryPool] in bytes
pub const XALLOC_POOL_CHUNK_SIZE: usize = 1024 * 1024 * 64;

/// Inner type for [XallocMemoryPool]. Necessary to implement `vulkano`'s `MemoryPool` on an `Arc<T>`.
#[derive(Debug)]
pub struct XallocMemoryPoolInner {
    device: Arc<Device>,

    /// For each memory type index, stores the associated `PoolAllocator` which manages that pool.
    pools: Arc<Mutex<HashMap<(u32, AllocLayout, MappingRequirement), XallocChunkAllocator, BuildHasherDefault<FnvHasher>>>>,
}

/// Memory managed pool that only allocates new chunks of device memory when needed, yielding blocks
/// from existing chunks when possible.
///
/// Alloc methods are in [impl MemoryPool for XallocMemoryPool](struct.XallocMemoryPool.html#impl-MemoryPool).
#[derive(Debug)]
pub struct XallocMemoryPool(pub Arc<XallocMemoryPoolInner>);

impl Clone for XallocMemoryPool {
    /// `XallocMemoryPool` is just a newtype around `Arc<XallocMemoryPoolInner>`, so it can be easily cloned.
    fn clone(&self) -> Self {
        XallocMemoryPool(self.0.clone())
    }
}

impl XallocMemoryPool {
    /// Creates a new `XallocMemoryPool`.
    #[inline]
    pub fn new(device: Arc<Device>) -> Self {
        let cap = device.physical_device().memory_types().len();
        let hasher = BuildHasherDefault::<FnvHasher>::default();

        XallocMemoryPool(Arc::new(XallocMemoryPoolInner {
            device: device.clone(),
            pools: Arc::new(Mutex::new(HashMap::with_capacity_and_hasher(cap, hasher))),
        }))
    }
}

unsafe impl MemoryPool for XallocMemoryPool {
    type Alloc = XallocMemoryPoolBlock;

    /// Provides a block of memory to use, allocating new chunks when all existing chunks are full.
    fn alloc_generic(&self, memory_type: MemoryType, size: usize, alignment: usize,
                     layout: AllocLayout, map: MappingRequirement)
                     -> Result<XallocMemoryPoolBlock, DeviceMemoryAllocError> {
        let mut pools = self.0.pools.lock().unwrap();

        if !memory_type.is_host_visible() {
            panic!("XallocMemoryPool only works with host-visible memory!");
        }

        match pools.entry((memory_type.id(), layout, map)) {
            // existing pool and allocator
            Entry::Occupied(mut entry) => {
                let chunk_allocator = entry.get_mut();
                let block = chunk_allocator.alloc(size, alignment, &self.0);
                Ok(block)
            },
            // create new pool and allocator
            Entry::Vacant(entry) => {
                let pool = StdHostVisibleMemoryTypePool::new(self.0.device.clone(), memory_type);
                let mut chunk_allocator = XallocChunkAllocator::new(pool.clone());
                let block = chunk_allocator.alloc(size, alignment, &self.0);
                entry.insert(chunk_allocator);
                Ok(block)
            },
        }
    }
}

unsafe impl DeviceOwned for XallocMemoryPool {
    #[inline]
    fn device(&self) -> &Arc<Device> {
        &self.0.device
    }
}

/// Chunk allocator using `xalloc`'s [SysTlsf](https://docs.rs/xalloc/0.2.6/xalloc/tlsf/type.SysTlsf.html)
/// for block allocation. See the module documentation for more info.
#[derive(Debug)]
pub struct XallocChunkAllocator {
    pub pool: Arc<StdHostVisibleMemoryTypePool>,
    pub chunks: HashMap<Arc<XallocMemoryPoolChunk>, Arc<RwLock<SysTlsf<usize>>>>,
}

impl XallocChunkAllocator {
    /// Creates a new ChunkAllocator to manage the given pool of device memory.
    pub fn new(pool: Arc<StdHostVisibleMemoryTypePool>) -> Self {
        Self { pool, chunks: HashMap::new() }
    }

    /// Allocates a new block. Uses [https://docs.rs/xalloc/0.2.6/xalloc/tlsf/type.SysTlsf.html](xalloc::SysTlsf)
    /// to manage block allocations for chunks, and allocates new chunks of device memory when needed.
    pub fn alloc(&mut self, size: usize, alignment: usize, pool: &Arc<XallocMemoryPoolInner>) -> XallocMemoryPoolBlock {
        for (chunk, block_allocator) in self.chunks.iter_mut() {
            let mut alloc_inner = block_allocator.write().unwrap();

            if size == 0 {
                return XallocMemoryPoolBlock {
                    chunk: chunk.clone(),
                    allocator: block_allocator.clone(),
                    region: None,
                    size,
                    offset: 0
                };
            }
            else {
                if let Some((region, offset)) = alloc_inner.alloc_aligned(size, alignment) {
                    return XallocMemoryPoolBlock {
                        chunk: chunk.clone(),
                        allocator: block_allocator.clone(),
                        region: Some(region),
                        size,
                        offset
                    };
                }
            }
            // no open spaces in that chunk, try next chunk
        }
        // no open spaces in any chunks, need to allocate new chunk
        let chunk_alloc = StdHostVisibleMemoryTypePool::alloc(&self.pool, XALLOC_POOL_CHUNK_SIZE, alignment).unwrap();
        let mut chunk_id = 1;
        while self.contains_chunk(chunk_id) {
            chunk_id += 1;
        }
        let chunk = Arc::new(XallocMemoryPoolChunk {
            alloc: chunk_alloc,
            pool: pool.clone(),
            id: chunk_id
        });
        let mut block_allocator = SysTlsf::new(XALLOC_POOL_CHUNK_SIZE);

        let block;
        if size == 0 {
            let allocator_arc = Arc::new(RwLock::new(block_allocator));
            self.chunks.insert(chunk.clone(), allocator_arc.clone());
            block = XallocMemoryPoolBlock {
                chunk: chunk.clone(),
                allocator: allocator_arc.clone(),
                region: None,
                size,
                offset: 0
            };
        }
        else {
            let (region, offset) = block_allocator.alloc_aligned(size, alignment).unwrap();
            // panic on this unwrap means you tried to allocate a block larger than the entire chunk.
            // XALLOC_POOL_CHUNK_SIZE needs to be increased.

            let allocator_arc = Arc::new(RwLock::new(block_allocator));
            self.chunks.insert(chunk.clone(), allocator_arc.clone());

            block = XallocMemoryPoolBlock {
                chunk: chunk.clone(),
                allocator: allocator_arc.clone(),
                region: Some(region),
                size,
                offset
            };
        }
        block
    }

    /// Gets whether a certain chunk id exists in this pool.
    pub fn contains_chunk(&self, chunk_id: usize) -> bool {
        for (chunk, _) in self.chunks.iter() {
            if chunk.id == chunk_id {
                return true;
            }
        }
        false
    }
}

/// Stores information about an allocated chunk of device memory. Blocks are allocated as regions
/// of one of these chunks.
#[derive(Debug)]
pub struct XallocMemoryPoolChunk {
    pub alloc: StdHostVisibleMemoryTypePoolAlloc,
    pub pool: Arc<XallocMemoryPoolInner>,
    pub id: usize
}
impl PartialEq for XallocMemoryPoolChunk {
    fn eq(&self, other: &XallocMemoryPoolChunk) -> bool {
        self.id == other.id
    }
}
impl Eq for XallocMemoryPoolChunk {}
impl ::std::hash::Hash for XallocMemoryPoolChunk {
    fn hash<H: ::std::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.id);
    }
}

/// Holds information about a single block of allocated memory.
///
/// Block is automatically freed from its chunk when `drop` is called.
#[derive(Debug)]
pub struct XallocMemoryPoolBlock {
    pub chunk: Arc<XallocMemoryPoolChunk>,
    pub allocator: Arc<RwLock<SysTlsf<usize>>>,
    /// this is None if size is zero, since no allocation is necessary
    pub region: Option<TlsfRegion<sys::Ptr>>,
    pub size: usize,
    pub offset: usize,
}
#[allow(dead_code)]
impl XallocMemoryPoolBlock {
    #[inline]
    pub fn size(&self) -> usize { self.size }
}
unsafe impl MemoryPoolAlloc for XallocMemoryPoolBlock {
    #[inline]
    fn mapped_memory(&self) -> Option<&MappedDeviceMemory> { Some(self.chunk.alloc.memory()) }
    #[inline]
    fn memory(&self) -> &DeviceMemory { self.chunk.alloc.memory().as_ref() }
    #[inline]
    fn offset(&self) -> usize { self.chunk.alloc.offset() + self.offset as usize }
}
impl Drop for XallocMemoryPoolBlock {
    fn drop(&mut self) {
        if let Some(region) = &self.region {
            // TODO: no idea if this is safe
            unsafe {
                let mut a = self.allocator.write().unwrap();
                let mut region_copy: TlsfRegion<sys::Ptr> = MaybeUninit::uninit().assume_init();
                std::ptr::copy(region, &mut region_copy, 1);
                a.dealloc(region_copy).unwrap();
            }
        }
    }
}
