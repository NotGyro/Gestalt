use std::fmt::{Display, Debug};
use std::fmt;
use std::error::Error;

use crate::common::voxelmath::*;
use crate::world::{TilePos, ChunkPos};

#[allow(dead_code)]
pub enum VoxelErrorKind {
    OutOfBounds,
    ChunkBoundIssue,
    NotYetLoaded,
    Other,
}
/// An error reported upon trying to get or set a voxel outside of our range.
#[derive(Debug)]
#[allow(dead_code)]
pub enum VoxelError {
    OutOfBounds(TilePos),
    ChunkBoundIssue(TilePos, ChunkPos),
    NotYetLoaded(TilePos),
    Other(Box<dyn Error + 'static>),
}

impl VoxelError {
    #[allow(dead_code)]
    fn kind(&self) -> VoxelErrorKind {
        match self {
            VoxelError::OutOfBounds(_) => VoxelErrorKind::OutOfBounds,
            VoxelError::ChunkBoundIssue(_,_) => VoxelErrorKind::ChunkBoundIssue,
            VoxelError::NotYetLoaded(_) => VoxelErrorKind::NotYetLoaded,
            VoxelError::Other(_) => VoxelErrorKind::Other,
        }
    }
}

impl Display for VoxelError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            VoxelError::OutOfBounds(pos) => write!(f, "Attempted to access a voxel at position {}, which is out of bounds on this space.", pos),
            VoxelError::ChunkBoundIssue(pos, chunkpos) => 
                write!(f, "Attempted to access a voxel at position {}, on chunk cell {}, which did not accept this as in-bounds.", pos, chunkpos),
            VoxelError::NotYetLoaded(pos) => write!(f, "Attempted to access a voxel position {}, which is not yet loaded.", pos),
            VoxelError::Other(err) => write!(f, "Other voxel error: {}", err),
        }
    }
}
impl Error for VoxelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None //I would love to have it to handle Other correctly but nope, the sized variablre requirement isn't having it.
    }
}


pub trait Voxel : Clone + Debug + PartialEq {}
impl<T> Voxel for T where T : Clone + Debug + PartialEq {}

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
    fn get(&self, coord: VoxelPos<P>) -> Result<T, VoxelError>;
    fn set(&mut self, coord: VoxelPos<P>, value: T) -> Result<(), VoxelError>;
}

/// Any VoxelStorage which has defined, finite bounds.
/// Must provide a valid voxel for any position within
/// the range provided by get_bounds().
/// Usually, this implies that the voxel storage is not paged.
pub trait VoxelStorageBounded<T: Voxel, P: VoxelCoord> : VoxelStorage<T, P> {
    fn get_bounds(&self) -> VoxelRange<P>;
}

pub trait VsBulkOps<T: Voxel, P: VoxelCoord> : VoxelStorageBounded<T, P> {
    /// Iterate over each voxel in this area.
    fn each_voxel<F: FnMut(T) -> ()>(&self, func: F);
    /// Iterate over each voxel and its (local to this chunk!) coordinate in this area.
    fn each_cell<F: FnMut(T, P) -> ()>(&self, func: F);
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
impl<T, E> UnwrapInto<T> for Result<T, E> where E: std::fmt::Debug {
    fn unwrap_into(self) -> T {
        self.unwrap()
    }
}

/// A voxel and adjacent voxels in 6 cardinal directions. Type arguments: T (The type we store) and V (the voxel type we map to / from).
pub trait VoxelNeighborhood : Clone {
    type SourceVoxel : Voxel;
    type OurVoxel : UnwrapInto<Self::SourceVoxel>;
    fn new(center: Self::SourceVoxel, posi_x: Self::SourceVoxel, posi_y: Self::SourceVoxel, posi_z: Self::SourceVoxel, nega_x: Self::SourceVoxel, nega_y: Self::SourceVoxel, nega_z: Self::SourceVoxel) -> Self;
    fn get_center(&self) -> &Self::OurVoxel;
    fn get(&self, neighbor: VoxelSide) -> &Self::OurVoxel;
}

pub trait VsNeighborhoodOps<T: Voxel, P: VoxelCoord> : VoxelStorageBounded<T, P> {
    type NeighborhoodType : VoxelNeighborhood<SourceVoxel=T>;
}

/// Copy voxels from one storage to another.
#[allow(dead_code)]
pub fn voxel_blit<T: Voxel, P: VoxelCoord, VA: VoxelStorage<T, P>, VB: VoxelStorage<T, P>>(source_range : VoxelRange<P>, source: &VA,
                                           dest_origin: VoxelPos<P>, dest: &mut VB)  -> Result<(), VoxelError> {
    for pos in source_range {
        let voxel = source.get(pos)?;
        let offset_pos = (pos - source_range.lower) + dest_origin;
        dest.set(offset_pos, voxel)?;
    }
    return Ok(());
}