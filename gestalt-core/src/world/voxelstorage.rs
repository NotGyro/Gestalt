use std::fmt::{Debug};
use std::hash::Hash;

use crate::common::voxelmath::*;

use super::{TileCoord, TilePos};

pub trait Voxel: Clone + Debug + PartialEq + Eq + Hash {}
impl<T> Voxel for T where T: Clone + Debug + PartialEq + Eq + Hash {}

/// Abstract categories of voxel errors allowing you to check 
/// ANY voxel error to see things like, say, is this recoverable? 
/// Can we ignore this? Do we need to perform some kind of data
/// corruption check? Etc.
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub enum VoxelErrorCategory { 
    OutOfBounds,
    NotYetLoaded,
    InvalidTileInput,
    PaletteIssue,
    Other,
}

pub trait VoxelError: std::error::Error + std::fmt::Debug { 
    fn kind(&self) -> VoxelErrorCategory; 
}

/// A basic trait for any 3d grid data structure.
/// Type arguments are type of element, type of position.
///
/// (Type of positon must be an integer, but I'm still using
/// genericism here because it should be possible to use
/// any bit length of integer, or even a bigint implementation
///
/// For this trait, a single level of detail is assumed.
///
/// For voxel data structures with a level of detail, we will
/// assume that the level of detail is a signed integer, and
/// calling these methods / treating them as "flat" voxel
/// structures implies acting on a level of detail of 0.
pub trait VoxelStorage<T: Voxel, P: VoxelCoord> {
    type Error: VoxelError;
    fn get(&self, coord: VoxelPos<P>) -> Result<&T, Self::Error>;
    fn set(&mut self, coord: VoxelPos<P>, value: T) -> Result<(), Self::Error>;
}

/// Any VoxelStorage which has defined, finite bounds.
/// Must provide a valid voxel for any position within
/// the range provided by get_bounds().
/// Usually, this implies that the voxel storage is not paged.
pub trait VoxelStorageBounded<T: Voxel, P: VoxelCoord>: VoxelStorage<T, P> {
    fn get_bounds(&self) -> VoxelRange<P>;
    /// A count of the total number of voxels in this storage.
    fn get_area(&self) -> P {
        (self.get_bounds().upper.x - self.get_bounds().lower.x)
            * (self.get_bounds().upper.y - self.get_bounds().lower.y)
            * (self.get_bounds().upper.z - self.get_bounds().lower.z)
    }
}

pub trait VsBulkOps<T: Voxel, P: VoxelCoord>: VoxelStorageBounded<T, P> {
    /// Iterate over each voxel in this area.
    fn each_voxel<F: FnMut(T)>(&self, func: F);
    /// Iterate over each voxel and its (local to this chunk!) coordinate in this area.
    fn each_cell<F: FnMut(T, P)>(&self, func: F);
    /// Iterate over each voxel, yielding a new voxel to replace it in this chunk.
    fn map_voxels<F: FnOnce(T) -> T>(&mut self, func: F);
    /// Iterate over each voxel and its (local to this chunk!) coordinate, yielding a new voxel to replace it in this chunk.
    fn map_cells<F: FnOnce(T, P) -> T>(&mut self, func: F);
}

/// Like Into but it may panic.
pub trait UnwrapInto<T> {
    fn unwrap_into(self) -> T;
}
impl<T> UnwrapInto<T> for Option<T> {
    fn unwrap_into(self) -> T {
        self.unwrap()
    }
}
impl<T> UnwrapInto<T> for T {
    fn unwrap_into(self) -> T {
        self
    }
}
impl<T, E> UnwrapInto<T> for Result<T, E>
where
    E: std::fmt::Debug,
{
    fn unwrap_into(self) -> T {
        self.unwrap()
    }
}

/// A voxel and adjacent voxels in 6 cardinal directions. Type arguments: T (The type we store) and V (the voxel type we map to / from).
pub trait VoxelNeighborhood: Clone {
    type SourceVoxel: Voxel;
    type OurVoxel: UnwrapInto<Self::SourceVoxel>;
    fn new(
        center: Self::SourceVoxel,
        posi_x: Self::SourceVoxel,
        posi_y: Self::SourceVoxel,
        posi_z: Self::SourceVoxel,
        nega_x: Self::SourceVoxel,
        nega_y: Self::SourceVoxel,
        nega_z: Self::SourceVoxel,
    ) -> Self;
    fn get_center(&self) -> &Self::OurVoxel;
    fn get(&self, neighbor: VoxelSide) -> &Self::OurVoxel;
}

pub trait VsNeighborhoodOps<T: Voxel, P: VoxelCoord>: VoxelStorageBounded<T, P> {
    type NeighborhoodType: VoxelNeighborhood<SourceVoxel = T>;
}

pub trait VoxelSpace<T: Voxel> : VoxelStorage<T, TileCoord> {
    /// Coordinate of a chunk
    type ChunkCoord : VoxelCoord;
    /// Coordinate of a voxel inside a chunk
    type WithinChunkCoord : VoxelCoord;
    type Chunk : VoxelStorageBounded<T, Self::WithinChunkCoord>;

    fn is_loaded(&self, voxel: TilePos) -> bool;

    /// Try to borrow a chunk immutably. If it isn't loaded yet, returns None.
    fn borrow_chunk(&self, chunk: &VoxelPos<Self::ChunkCoord>) -> Result<&Self::Chunk, Self::Error>;
    /// Try to borrow a chunk mutably. If it isn't loaded yet, returns None.
    fn borrow_chunk_mut(&mut self, chunk: &VoxelPos<Self::ChunkCoord>) -> Result<&mut Self::Chunk, Self::Error>;

    fn get_loaded_chunks(&self) -> Vec<&VoxelPos<Self::ChunkCoord>>;
}

/*
/// Copy voxels from one storage to another.
#[allow(dead_code)]
pub fn voxel_blit<T: Voxel, P: VoxelCoord, VA: VoxelStorage<T, P>, VB: VoxelStorage<T, P>>(
    source_range: VoxelRange<P>,
    source: &VA,
    dest_origin: VoxelPos<P>,
    dest: &mut VB,
) -> Result<(), VoxelError> {
    for pos in source_range {
        let voxel = source.get(pos)?;
        let offset_pos = (pos - source_range.lower) + dest_origin;
        dest.set(offset_pos, voxel.clone())?;
    }
    Ok(())
}*/