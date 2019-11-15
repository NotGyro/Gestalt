//! Buffer whose content is accessible to the CPU. Managed by AutoMemoryPool.
//!
//! The `CpuAccessibleBufferAutoPool` is a basic general-purpose buffer. It's memory is managed by
//! a `AutoMemoryPool`. It can be used in any situation but may not perform as well as other buffer
//! types.
//!
//! Each access from the CPU or from the GPU locks the whole buffer for either reading or writing.
//! You can read the buffer multiple times simultaneously. Trying to read and write simultaneously,
//! or write and write simultaneously will block.

use smallvec::SmallVec;
use std::error;
use std::fmt;
use std::iter;
use std::marker::PhantomData;
use std::mem;
use std::ops::Deref;
use std::ops::DerefMut;
use std::ptr;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use vulkano::buffer::BufferUsage;
use vulkano::buffer::sys::{BufferCreationError, SparseLevel, UnsafeBuffer};
use vulkano::buffer::{BufferAccess, BufferInner};
use vulkano::buffer::TypedBufferAccess;
use vulkano::device::{Device, DeviceOwned, Queue};
use vulkano::image::ImageAccess;
use vulkano::instance::QueueFamily;
use vulkano::memory::{Content, DeviceMemoryAllocError};
use vulkano::memory::CpuAccess as MemCpuAccess;
use vulkano::memory::pool::{AllocLayout, MappingRequirement, MemoryPool, MemoryPoolAlloc};
use vulkano::sync::{AccessError, Sharing};

use crate::memory::xalloc::{XallocMemoryPoolBlock, XallocMemoryPool};

/// Buffer whose content is accessible by the CPU. Managed by
/// [XallocMemoryPool](crate::memory::xalloc::XallocMemoryPool).
#[derive(Debug)]
pub struct CpuAccessibleBufferXalloc<T: ?Sized, A = XallocMemoryPoolBlock> {
    // Inner content.
    inner: UnsafeBuffer,

    // The memory held by the buffer.
    memory: A,

    memory_pool: XallocMemoryPool,

    // Access pattern of the buffer.
    // Every time the user tries to read or write the buffer from the CPU, this `RwLock` is kept
    // locked and its content is checked to verify that we are allowed access. Every time the user
    // tries to submit this buffer for the GPU, this `RwLock` is briefly locked and modified.
    access: RwLock<CurrentGpuAccess>,

    // Queue families allowed to access this buffer.
    queue_families: SmallVec<[u32; 4]>,

    // Necessary to make it compile.
    marker: PhantomData<Box<T>>,
}

#[derive(Debug)]
enum CurrentGpuAccess {
    NonExclusive {
        // Number of non-exclusive GPU accesses. Can be 0.
        num: AtomicUsize,
    },
    Exclusive {
        // Number of exclusive locks. Cannot be 0. If 0 is reached, we must jump to `NonExclusive`.
        num: usize,
    },
}

#[allow(dead_code)]
impl<T> CpuAccessibleBufferXalloc<T> {
    /// Builds a new buffer with some data in it. Only allowed for sized data.
    pub fn from_data(device: Arc<Device>, pool: XallocMemoryPool, usage: BufferUsage, data: T)
                     -> Result<Arc<CpuAccessibleBufferXalloc<T>>, DeviceMemoryAllocError>
        where T: Content + 'static
    {
        unsafe {
            let uninitialized =
                CpuAccessibleBufferXalloc::raw(device, mem::size_of::<T>(), pool.clone(), usage, iter::empty())?;

            // Note that we are in panic-unsafety land here. However a panic should never ever
            // happen here, so in theory we are safe.
            // TODO: check whether that's true ^

            {
                let mut mapping = uninitialized.write().unwrap();
                ptr::write::<T>(&mut *mapping, data)
            }

            Ok(uninitialized)
        }
    }

    /// Builds a new uninitialized buffer. Only allowed for sized data.
    #[inline]
    pub unsafe fn uninitialized(device: Arc<Device>, pool: XallocMemoryPool, usage: BufferUsage)
                                -> Result<Arc<CpuAccessibleBufferXalloc<T>>, DeviceMemoryAllocError> {
        CpuAccessibleBufferXalloc::raw(device, mem::size_of::<T>(), pool.clone(), usage, iter::empty())
    }
}

impl<T> CpuAccessibleBufferXalloc<[T]> {
    /// Builds a new buffer that contains an array `T`. The initial data comes from an iterator
    /// that produces that list of Ts.
    pub fn from_iter<I>(device: Arc<Device>, pool: XallocMemoryPool, usage: BufferUsage, data: I)
                        -> Result<Arc<CpuAccessibleBufferXalloc<[T]>>, DeviceMemoryAllocError>
        where I: ExactSizeIterator<Item = T>,
              T: Content + 'static
    {
        unsafe {
            let uninitialized =
                CpuAccessibleBufferXalloc::uninitialized_array(device, data.len(), pool.clone(), usage)?;

            // Note that we are in panic-unsafety land here. However a panic should never ever
            // happen here, so in theory we are safe.
            // TODO: check whether that's true ^

            {
                let mut mapping = uninitialized.write().unwrap();

                for (i, o) in data.zip(mapping.iter_mut()) {
                    ptr::write(o, i);
                }
            }

            Ok(uninitialized)
        }
    }

    /// Builds a new buffer. Can be used for arrays.
    #[inline]
    pub unsafe fn uninitialized_array(
        device: Arc<Device>, len: usize, pool: XallocMemoryPool, usage: BufferUsage)
        -> Result<Arc<CpuAccessibleBufferXalloc<[T]>>, DeviceMemoryAllocError> {
        CpuAccessibleBufferXalloc::raw(device, len * mem::size_of::<T>(), pool.clone(), usage, iter::empty())
    }
}

impl<T: ?Sized> CpuAccessibleBufferXalloc<T> {
    /// Builds a new buffer without checking the size.
    ///
    /// # Safety
    ///
    /// You must ensure that the size that you pass is correct for `T`.
    ///
    pub unsafe fn raw<'a, I>(device: Arc<Device>, size: usize, pool: XallocMemoryPool, usage: BufferUsage, queue_families: I)
                             -> Result<Arc<CpuAccessibleBufferXalloc<T>>, DeviceMemoryAllocError>
        where I: IntoIterator<Item = QueueFamily<'a>>
    {
        let queue_families = queue_families
            .into_iter()
            .map(|f| f.id())
            .collect::<SmallVec<[u32; 4]>>();

        let (buffer, mem_reqs) = {
            let sharing = if queue_families.len() >= 2 {
                Sharing::Concurrent(queue_families.iter().cloned())
            } else {
                Sharing::Exclusive
            };

            match UnsafeBuffer::new(device.clone(), size, usage, sharing, SparseLevel::none()) {
                Ok(b) => b,
                Err(BufferCreationError::AllocError(err)) => return Err(err),
                Err(_) => unreachable!(),        // We don't use sparse binding, therefore the other
                // errors can't happen
            }
        };

        let mem_ty = device.physical_device().memory_types().filter(|t| t.is_host_visible()).next().unwrap();
        let mem = pool.alloc_generic(mem_ty, size, mem_reqs.alignment, AllocLayout::Linear, MappingRequirement::Map)?;
        debug_assert!((mem.offset() % mem_reqs.alignment) == 0);
        debug_assert!(mem.mapped_memory().is_some());
        buffer.bind_memory(mem.memory(), mem.offset())?;

        Ok(Arc::new(CpuAccessibleBufferXalloc {
            inner: buffer,
            memory: mem,
            memory_pool: pool.clone(),
            access: RwLock::new(CurrentGpuAccess::NonExclusive {
                num: AtomicUsize::new(0),
            }),
            queue_families,
            marker: PhantomData,
        }))
    }
}

#[allow(dead_code)]
impl<T: ?Sized, A> CpuAccessibleBufferXalloc<T, A> {
    /// Returns the queue families this buffer can be used on.
    // TODO: use a custom iterator
    #[inline]
    pub fn queue_families(&self) -> Vec<QueueFamily> {
        self.queue_families
            .iter()
            .map(|&num| {
                self.device()
                    .physical_device()
                    .queue_family_by_id(num)
                    .unwrap()
            })
            .collect()
    }
}

#[allow(dead_code)]
impl<T: ?Sized, A> CpuAccessibleBufferXalloc<T, A>
    where T: Content + 'static,
          A: MemoryPoolAlloc
{
    /// Locks the buffer in order to read its content from the CPU.
    ///
    /// If the buffer is currently used in exclusive mode by the GPU, this function will return
    /// an error. Similarly if you called `write()` on the buffer and haven't dropped the lock,
    /// this function will return an error as well.
    ///
    /// After this function successfully locks the buffer, any attempt to submit a command buffer
    /// that uses it in exclusive mode will fail. You can still submit this buffer for non-exlusive
    /// accesses (ie. reads).
    #[inline]
    pub fn read(&self) -> Result<ReadLock<T>, ReadLockError> {
        let lock = match self.access.try_read() {
            Ok(l) => l,
            // TODO: if a user simultaneously calls .write(), and write() is currently finding out
            //       that the buffer is in fact GPU locked, then we will return a CpuWriteLocked
            //       error instead of a GpuWriteLocked ; is this a problem? how do we fix this?
            Err(_) => return Err(ReadLockError::CpuWriteLocked),
        };

        if let CurrentGpuAccess::Exclusive { .. } = *lock {
            return Err(ReadLockError::GpuWriteLocked);
        }

        let offset = self.memory.offset();
        let range = offset .. offset + self.inner.size();

        Ok(ReadLock {
            inner: unsafe { self.memory.mapped_memory().unwrap().read_write(range) },
            lock: lock,
        })
    }

    /// Locks the buffer in order to write its content from the CPU.
    ///
    /// If the buffer is currently in use by the GPU, this function will return an error. Similarly
    /// if you called `read()` on the buffer and haven't dropped the lock, this function will
    /// return an error as well.
    ///
    /// After this function successfully locks the buffer, any attempt to submit a command buffer
    /// that uses it and any attempt to call `read()` will return an error.
    #[inline]
    pub fn write(&self) -> Result<WriteLock<T>, WriteLockError> {
        let lock = match self.access.try_write() {
            Ok(l) => l,
            // TODO: if a user simultaneously calls .read() or .write(), and the function is
            //       currently finding out that the buffer is in fact GPU locked, then we will
            //       return a CpuLocked error instead of a GpuLocked ; is this a problem?
            //       how do we fix this?
            Err(_) => return Err(WriteLockError::CpuLocked),
        };

        match *lock {
            CurrentGpuAccess::NonExclusive { ref num } if num.load(Ordering::SeqCst) == 0 => (),
            _ => return Err(WriteLockError::GpuLocked),
        }

        let offset = self.memory.offset();
        let range = offset .. offset + self.inner.size();

        Ok(WriteLock {
            inner: unsafe { self.memory.mapped_memory().unwrap().read_write(range) },
            lock: lock,
        })
    }
}

unsafe impl<T: ?Sized, A> BufferAccess for CpuAccessibleBufferXalloc<T, A>
    where T: 'static + Send + Sync
{
    #[inline]
    fn inner(&self) -> BufferInner {
        BufferInner {
            buffer: &self.inner,
            offset: 0,
        }
    }

    #[inline]
    fn size(&self) -> usize {
        self.inner.size()
    }

    #[inline]
    fn conflicts_buffer(&self, other: &dyn BufferAccess) -> bool {
        self.conflict_key() == other.conflict_key() // TODO:
    }

    #[inline]
    fn conflicts_image(&self, _other: &dyn ImageAccess) -> bool {
        false
    }

    #[inline]
    fn conflict_key(&self) -> (u64, usize) {
        (self.inner.key(), 0)
    }

    #[inline]
    fn try_gpu_lock(&self, exclusive_access: bool, _: &Queue) -> Result<(), AccessError> {
        if exclusive_access {
            let mut lock = match self.access.try_write() {
                Ok(lock) => lock,
                Err(_) => return Err(AccessError::AlreadyInUse),
            };

            match *lock {
                CurrentGpuAccess::NonExclusive { ref num } if num.load(Ordering::SeqCst) == 0 => (),
                _ => return Err(AccessError::AlreadyInUse),
            };

            *lock = CurrentGpuAccess::Exclusive { num: 1 };
            Ok(())

        } else {
            let lock = match self.access.try_read() {
                Ok(lock) => lock,
                Err(_) => return Err(AccessError::AlreadyInUse),
            };

            match *lock {
                CurrentGpuAccess::Exclusive { .. } => return Err(AccessError::AlreadyInUse),
                CurrentGpuAccess::NonExclusive { ref num } => {
                    num.fetch_add(1, Ordering::SeqCst)
                },
            };

            Ok(())
        }
    }

    #[inline]
    unsafe fn increase_gpu_lock(&self) {
        // First, handle if we have a non-exclusive access.
        {
            // Since the buffer is in use by the GPU, it is invalid to hold a write-lock to
            // the buffer. The buffer can still be briefly in a write-locked state for the duration
            // of the check though.
            let read_lock = self.access.read().unwrap();
            if let CurrentGpuAccess::NonExclusive { ref num } = *read_lock {
                let prev = num.fetch_add(1, Ordering::SeqCst);
                debug_assert!(prev >= 1);
                return;
            }
        }

        // If we reach here, this means that `access` contains `CurrentGpuAccess::Exclusive`.
        {
            // Same remark as above, but for writing.
            let mut write_lock = self.access.write().unwrap();
            if let CurrentGpuAccess::Exclusive { ref mut num } = *write_lock {
                *num += 1;
            } else {
                unreachable!()
            }
        }
    }

    #[inline]
    unsafe fn unlock(&self) {
        // First, handle if we had a non-exclusive access.
        {
            // Since the buffer is in use by the GPU, it is invalid to hold a write-lock to
            // the buffer. The buffer can still be briefly in a write-locked state for the duration
            // of the check though.
            let read_lock = self.access.read().unwrap();
            if let CurrentGpuAccess::NonExclusive { ref num } = *read_lock {
                let prev = num.fetch_sub(1, Ordering::SeqCst);
                debug_assert!(prev >= 1);
                return;
            }
        }

        // If we reach here, this means that `access` contains `CurrentGpuAccess::Exclusive`.
        {
            // Same remark as above, but for writing.
            let mut write_lock = self.access.write().unwrap();
            if let CurrentGpuAccess::Exclusive { ref mut num } = *write_lock {
                if *num != 1 {
                    *num -= 1;
                    return;
                }
            } else {
                // Can happen if we lock in exclusive mode N times, and unlock N+1 times with the
                // last two unlocks happen simultaneously.
                panic!()
            }

            *write_lock = CurrentGpuAccess::NonExclusive { num: AtomicUsize::new(0) };
        }
    }
}

unsafe impl<T: ?Sized, A> TypedBufferAccess for CpuAccessibleBufferXalloc<T, A>
    where T: 'static + Send + Sync
{
    type Content = T;
}

unsafe impl<T: ?Sized, A> DeviceOwned for CpuAccessibleBufferXalloc<T, A> {
    #[inline]
    fn device(&self) -> &Arc<Device> {
        self.inner.device()
    }
}

/// Object that can be used to read or write the content of a `CpuAccessibleBufferAutoPool`.
///
/// Note that this object holds a rwlock read guard on the chunk. If another thread tries to access
/// this buffer's content or tries to submit a GPU command that uses this buffer, it will block.
pub struct ReadLock<'a, T: ?Sized + 'a> {
    inner: MemCpuAccess<'a, T>,
    lock: RwLockReadGuard<'a, CurrentGpuAccess>,
}

#[allow(dead_code)]
impl<'a, T: ?Sized + 'a> ReadLock<'a, T> {
    /// Makes a new `ReadLock` to access a sub-part of the current `ReadLock`.
    #[inline]
    pub fn map<U: ?Sized + 'a, F>(self, f: F) -> ReadLock<'a, U>
        where F: FnOnce(&mut T) -> &mut U
    {
        ReadLock {
            inner: self.inner.map(|ptr| unsafe { f(&mut *ptr) as *mut _ }),
            lock: self.lock,
        }
    }
}

impl<'a, T: ?Sized + 'a> Deref for ReadLock<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

/// Error when attempting to CPU-read a buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum ReadLockError {
    /// The buffer is already locked for write mode by the CPU.
    CpuWriteLocked,
    /// The buffer is already locked for write mode by the GPU.
    GpuWriteLocked,
}

impl error::Error for ReadLockError {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            ReadLockError::CpuWriteLocked => {
                "the buffer is already locked for write mode by the CPU"
            },
            ReadLockError::GpuWriteLocked => {
                "the buffer is already locked for write mode by the GPU"
            },
        }
    }
}

impl fmt::Display for ReadLockError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", error::Error::description(self))
    }
}

/// Object that can be used to read or write the content of a `CpuAccessibleBufferAutoPool`.
///
/// Note that this object holds a rwlock write guard on the chunk. If another thread tries to access
/// this buffer's content or tries to submit a GPU command that uses this buffer, it will block.
/// #[allow(dead_code)]
pub struct WriteLock<'a, T: ?Sized + 'a> {
    inner: MemCpuAccess<'a, T>,
    lock: RwLockWriteGuard<'a, CurrentGpuAccess>,
}

#[allow(dead_code)]
impl<'a, T: ?Sized + 'a> WriteLock<'a, T> {
    /// Makes a new `WriteLock` to access a sub-part of the current `WriteLock`.
    #[inline]
    pub fn map<U: ?Sized + 'a, F>(self, f: F) -> WriteLock<'a, U>
        where F: FnOnce(&mut T) -> &mut U
    {
        WriteLock {
            inner: self.inner.map(|ptr| unsafe { f(&mut *ptr) as *mut _ }),
            lock: self.lock,
        }
    }
}

impl<'a, T: ?Sized + 'a> Deref for WriteLock<'a, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

impl<'a, T: ?Sized + 'a> DerefMut for WriteLock<'a, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        self.inner.deref_mut()
    }
}

/// Error when attempting to CPU-write a buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteLockError {
    /// The buffer is already locked by the CPU.
    CpuLocked,
    /// The buffer is already locked by the GPU.
    GpuLocked,
}

impl error::Error for WriteLockError {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            WriteLockError::CpuLocked => {
                "the buffer is already locked by the CPU"
            },
            WriteLockError::GpuLocked => {
                "the buffer is already locked by the GPU"
            },
        }
    }
}

impl fmt::Display for WriteLockError {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(fmt, "{}", error::Error::description(self))
    }
}
