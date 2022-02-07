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
    /// Any error encountered while trying to load a chunk
    LoadingIssue,
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
    type Error: VoxelError + Sized;
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

pub mod voxel_bulk_ops { 
    use super::*;

    /// Like Into but it may panic, or TryInto with a .unwrap on it.
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

    #[derive(Debug)]
    /// Two error types in a generic fused. 
    pub enum FusedError<AE, BE> 
        where AE: std::error::Error + Sized, BE: std::error::Error + Sized {
            ErrorA(AE), 
            ErrorB(BE),
    }
    impl<AE, BE> std::fmt::Display for FusedError<AE, BE> 
            where AE: std::error::Error + Sized, BE: std::error::Error + Sized {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                FusedError::ErrorA(a) => std::fmt::Display::fmt(a, f),
                FusedError::ErrorB(b) => std::fmt::Display::fmt(b, f),
            }
        }
    }
    impl<AE, BE> std::error::Error for FusedError<AE, BE> 
        where AE: std::error::Error + Sized, BE: std::error::Error + Sized {}
    
    pub trait VoxelMappable<T: Voxel, P: VoxelCoord>: VoxelStorageBounded<T, P> {
        /// Iterate over each voxel in this area.
        fn each_voxel<F: FnMut(T)>(&self, func: F);
        /// Iterate over each voxel and its (local to this chunk!) coordinate in this area.
        fn each_cell<F: FnMut(T, VoxelPos<P>)>(&self, func: F);
        /// Iterate over each voxel, yielding a new voxel to replace it in this chunk.
        fn map_voxels<F: FnOnce(T) -> T>(&mut self, func: F);
        /// Iterate over each voxel and its (local to this chunk!) coordinate, yielding a new voxel to replace it in this chunk.
        fn map_cells<F: FnOnce(T, VoxelPos<P>) -> T>(&mut self, func: F);
    }
    
    /// A voxel and adjacent voxels in 6 cardinal directions. Type arguments: T (The type we store) and V (the voxel type we map to / from).
    pub trait VoxelNeighborhood: Clone + Sized {
        type SourceVoxel: Voxel;
        type OurVoxel: UnwrapInto<Self::SourceVoxel>;
        fn new(
            center: Self::SourceVoxel,
            sides: SidesArray<Self::SourceVoxel>
        ) -> Self;
        fn get_center(&self) -> &Self::OurVoxel;
        fn get(&self, neighbor: VoxelSide) -> &Self::OurVoxel;
    }
    
    pub trait VoxelNeighborhoodOps<T: Voxel, P: VoxelCoord>: VoxelStorageBounded<T, P> {
        type Neighborhood: VoxelNeighborhood<SourceVoxel = T>;
        /// Iterate over each voxel-neighborhood in this area.
        fn each_neighborhood<F: FnMut(Self::Neighborhood)>(&self, func: F);
        /// Iterate over each voxel-neighborhood and the position of its center, in this area.
        fn each_neighborhood_enumerate<F: FnMut(Self::Neighborhood, VoxelPos<P>)>(&self, func: F);
        /// Iterate over each neighborhood. The value returned from `func` will replace the center cell.
        fn map_neighborhood<F: FnOnce(Self::Neighborhood) -> T>(&mut self, func: F);
        /// Iterate over each neighborhood. The value returned from `func` will replace the center cell which is at the given position.
        fn map_neighborhood_enumerate<F: FnOnce(Self::Neighborhood, VoxelPos<P>) -> T>(&mut self, func: F);
    }

    /// Operations to set entire Y-axis-aligned columns in a VoxelStorage. Intended for worldgen.
    pub trait VoxelColumns<T: Voxel, P: VoxelCoord>: VoxelStorage<T,P> {
        /// Sets `column_height` voxels along the Y axis at our given (X,Z) position to `value`
        fn set_vertical_column_down(&mut self, top: &VoxelPos<P>, column_height: P, value: &T) -> Result<(), Self::Error>;
        /// Sets `column_height` voxels along the Y axis at our given (X,Z) position to `value`
        fn set_vertical_column_up(&mut self, bottom: &VoxelPos<P>, column_height: P, value: &T) -> Result<(), Self::Error>;
    }

    /// Operations to set entire Y-axis-aligned columns in a VoxelStorage. Intended for worldgen.
    pub trait BoundedVoxelColumns<T: Voxel, P: VoxelCoord>: VoxelStorageBounded<T,P> {
        /// Sets all voxels, until we hit the boundary, along the Y axis at our given (X,Z) position from the top down to `value`
        fn set_whole_vertical_column_down(&mut self, top: &VoxelPos<P>, value: &T) -> Result<(), Self::Error>;
        /// Sets all voxels, until we hit the boundary, along the Y axis at our given (X,Z) position from the bottom up to `value`
        fn set_whole_vertical_column_up(&mut self, bottom: &VoxelPos<P>, value: &T) -> Result<(), Self::Error>;
    }

    /// Operations to set entire rectangular cuboid regions to a value in a voxel storage.
    pub trait VoxelRangeSet<T: Voxel, P: VoxelCoord>: VoxelStorageBounded<T, P> { 
        fn set_range(&mut self, range: VoxelRange<P>, value: &T) -> Result<(), Self::Error>;
    }
    
    /// Copy voxels from one storage to another. Naive implementation, just uses Set and Get. Not optimized.
    #[allow(dead_code)]
    pub fn naive_voxel_blit<T: Voxel, P: VoxelCoord, VA: VoxelStorage<T, P>, VB: VoxelStorage<T, P>>(
        source_range: VoxelRange<P>,
        source: &VA,
        dest_origin: VoxelPos<P>,
        dest: &mut VB,
    ) -> Result<(), FusedError<VA::Error, VB::Error>> {
        for pos in source_range {
            let voxel = source.get(pos).map_err(|a| FusedError::ErrorA(a))?;
            let offset_pos = (pos - source_range.lower) + dest_origin;
            dest.set(offset_pos, voxel.clone()).map_err(|b| FusedError::ErrorB(b))?;
        }
        Ok(())
    }
}