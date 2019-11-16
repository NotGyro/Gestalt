extern crate std;
extern crate num;
use std::fmt::{Display, Debug};
use std::default::Default;
use std::fmt;
use std::error;
use std::error::Error;
use std::result::Result;

use crate::voxel::voxelmath::{VoxelCoord, VoxelPos, VoxelRange,VoxelAxis};

pub trait Voxel : Clone + Debug + Eq {}
impl<T> Voxel for T where T : Clone + Debug + Eq {}

#[allow(dead_code)]
pub enum VoxelErrorKind {
    OutOfBounds,
    NotYetLoaded,
    SetInvalidValue,
    InvalidValueAt,
    Other,
}
/// An error reported upon trying to get or set a voxel outside of our range. 
#[derive(Debug)]
#[allow(dead_code)]
pub enum VoxelError {
    OutOfBounds(String, String),
    NotYetLoaded(String),
    SetInvalidValue(String),
    InvalidValueAt(String),
    Other(Box<dyn error::Error + 'static>),
}

impl VoxelError {
    #[allow(dead_code)]
    fn kind(&self) -> VoxelErrorKind {
        match self { 
            VoxelError::OutOfBounds(_,_) => VoxelErrorKind::OutOfBounds,
            VoxelError::NotYetLoaded(_) => VoxelErrorKind::NotYetLoaded,
            VoxelError::SetInvalidValue(_) => VoxelErrorKind::SetInvalidValue,
            VoxelError::InvalidValueAt(_) => VoxelErrorKind::InvalidValueAt,
            VoxelError::Other(_) => VoxelErrorKind::Other,
        }
    }
}

impl Display for VoxelError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self { 
            VoxelError::OutOfBounds(pos,sz) => write!(f, "Attempted to access a voxel at position {} on a storage with bounds {}", pos, sz),
            VoxelError::NotYetLoaded(pos) => write!(f, "Attempted to access a voxel position {}, which is not yet loaded.", pos),
            VoxelError::SetInvalidValue(pos) => write!(f, "Attempted to set voxel at {} to an invalid value.", pos),
            VoxelError::InvalidValueAt(pos) => write!(f, "Voxel at {} contains an invalid value, most likely corrupt.", pos),
            VoxelError::Other(err) => write!(f, "Other voxel error: {}", err),
        }
    }
}
impl Error for VoxelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None //I would love to have it to handle Other correctly but nope, the sized variablre requirement isn't having it.
    }
}

/*impl<T> From<Box<dyn error::Error + 'static>> for VoxelError<T> where T : 'static + VoxelCoord{
    fn from(error: Box<dyn error::Error + 'static>) -> Self {
        VoxelError::Other(error)
    }
}*/ /*
impl From<Box<dyn error::Error + 'static>> for VoxelError {
    fn from(error: Box<dyn error::Error + 'static>) -> Self {
        VoxelError::Other(error)
    }
}*/

#[derive(Clone, Debug)]
pub struct VoxelNeighborhoodFull<T: Voxel> {
    pub center : T,
    pub posi_x : T,
    pub nega_x : T,
    pub posi_y : T,
    pub nega_y : T,
    pub posi_z : T,
    pub nega_z : T,
}
impl<T> VoxelNeighborhoodFull<T> where T : Voxel{
    #[inline]
    #[allow(dead_code)]
    pub fn get_neighbor(&self, index: VoxelAxis) -> T {
        match index {
            VoxelAxis::PosiX => self.posi_x.clone(),
            VoxelAxis::NegaX => self.nega_x.clone(),
            VoxelAxis::PosiY => self.posi_y.clone(),
            VoxelAxis::NegaY => self.nega_y.clone(),
            VoxelAxis::PosiZ => self.posi_z.clone(),
            VoxelAxis::NegaZ => self.nega_z.clone(),
        }
    }
    #[inline]
    #[allow(dead_code)]
    pub fn set_neighbor(&mut self, index: VoxelAxis, value: T) {
        match index {
            VoxelAxis::PosiX => self.posi_x = value,
            VoxelAxis::NegaX => self.nega_x = value,
            VoxelAxis::PosiY => self.posi_y = value,
            VoxelAxis::NegaY => self.nega_y = value,
            VoxelAxis::PosiZ => self.posi_z = value,
            VoxelAxis::NegaZ => self.nega_z = value,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VoxelNeighborhood<T: Voxel> {
    pub center : T,
    pub posi_x : Option<T>,
    pub nega_x : Option<T>,
    pub posi_y : Option<T>,
    pub nega_y : Option<T>,
    pub posi_z : Option<T>,
    pub nega_z : Option<T>,
}

impl<T> VoxelNeighborhood<T> where T : Voxel{
    #[inline]
    #[allow(dead_code)]
    pub fn get_neighbor(&self, index: VoxelAxis) -> Option<T> {
        match index {
            VoxelAxis::PosiX => self.posi_x.clone(),
            VoxelAxis::NegaX => self.nega_x.clone(),
            VoxelAxis::PosiY => self.posi_y.clone(),
            VoxelAxis::NegaY => self.nega_y.clone(),
            VoxelAxis::PosiZ => self.posi_z.clone(),
            VoxelAxis::NegaZ => self.nega_z.clone(),
        }
    }
    #[inline]
    #[allow(dead_code)]
    pub fn set_neighbor(&mut self, index: VoxelAxis, value: Option<T>) {
        match index {
            VoxelAxis::PosiX => self.posi_x = value,
            VoxelAxis::NegaX => self.nega_x = value,
            VoxelAxis::PosiY => self.posi_y = value,
            VoxelAxis::NegaY => self.nega_y = value,
            VoxelAxis::PosiZ => self.posi_z = value,
            VoxelAxis::NegaZ => self.nega_z = value,
        }
    }
    ///Count how many cells are filled in this neighborhood.
    #[inline]
    #[allow(dead_code)]
    pub fn count(&self) -> usize { 
        let mut num = 1; //Start with 1 because center has to be a valid voxel.
        if self.posi_x.is_some() {num += 1};
        if self.nega_x.is_some() {num += 1};
        if self.posi_y.is_some() {num += 1};
        if self.nega_y.is_some() {num += 1};
        if self.posi_z.is_some() {num += 1};
        if self.nega_z.is_some() {num += 1};
        num
    }
}


impl<T> From<VoxelNeighborhoodFull<T>> for VoxelNeighborhood<T> where T: Voxel {
    fn from(other: VoxelNeighborhoodFull<T>) -> Self {
        VoxelNeighborhood {
            center : other.center,
            posi_x : Some(other.posi_x),
            nega_x : Some(other.nega_x),
            posi_y : Some(other.posi_y),
            nega_y : Some(other.nega_y),
            posi_z : Some(other.posi_z),
            nega_z : Some(other.nega_z),
        }
    }
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
    // Get and Set are all you need to implement a Voxel Storage.
    fn get(&self, coord: VoxelPos<P>) -> Result<T, VoxelError>;
    fn set(&mut self, coord: VoxelPos<P>, value: T) -> Result<(), VoxelError>;

    /*
    fn apply_event(&mut self, e : VoxelEvent<T, P>) -> Result<(), VoxelError> where Self: std::marker::Sized {
        e.apply_blind(self)?;
        Ok(())
    }*/

    // ----------------------------------------------------------------------------------------------------------------
    // ---- Batch operations and etc start here. Everything below this point should have a default implementation. ----
    // ----------------------------------------------------------------------------------------------------------------

    /// Returns the whole neighborhood of voxels including a center voxel and all adjacent voxels.
    /// Does not include diagnonally-adjacent voxels, just neighbors in the six cardinal directions.
    /// This version of get_neighborhood will return an error if any one of its component parts cannot be retrieved.
    /// It is guaranteed to either get you all 7 of the voxels you are requesting, or none at all.
    fn get_neighborhood_full(&self, coord: VoxelPos<P>) -> Result<VoxelNeighborhoodFull<T>, VoxelError> {
        Ok(VoxelNeighborhoodFull {
            center : self.get(coord)?,
            posi_x : self.get(coord.get_neighbor(VoxelAxis::PosiX))?,
            nega_x : self.get(coord.get_neighbor(VoxelAxis::NegaX))?,
            posi_y : self.get(coord.get_neighbor(VoxelAxis::PosiY))?,
            nega_y : self.get(coord.get_neighbor(VoxelAxis::NegaY))?,
            posi_z : self.get(coord.get_neighbor(VoxelAxis::PosiZ))?,
            nega_z : self.get(coord.get_neighbor(VoxelAxis::NegaZ))?,
        })
    }
    /// Returns the whole neighborhood of voxels including a center voxel and all adjacent voxels.
    /// Does not include diagnonally-adjacent voxels, just neighbors in the six cardinal directions.
    /// The center voxel must be valid / loaded.
    fn get_neighborhood(&self, coord: VoxelPos<P>) -> Result<VoxelNeighborhood<T>, VoxelError> {
        // Get an Ok(None) if our position is out of bounds, Ok(Some(T)) if we have a voxel, Err(error) if we have a non-bounds / non-loading error.
        #[inline(always)]
        #[allow(dead_code)]
        fn filter_error<V>(r: Result<V, VoxelError>) -> Result<Option<V>, VoxelError> {
            match r { 
                Ok(vxl) => Ok(Some(vxl)),
                Err(VoxelError::OutOfBounds(_,_)) => Ok(None),
                Err(VoxelError::NotYetLoaded(_)) => Ok(None),
                Err(error) => Err(error),
            }
        }
        Ok(VoxelNeighborhood {
            center : self.get(coord)?,
            posi_x : filter_error(self.get(coord.get_neighbor(VoxelAxis::PosiX)))?,
            nega_x : filter_error(self.get(coord.get_neighbor(VoxelAxis::NegaX)))?,
            posi_y : filter_error(self.get(coord.get_neighbor(VoxelAxis::PosiY)))?,
            nega_y : filter_error(self.get(coord.get_neighbor(VoxelAxis::NegaY)))?,
            posi_z : filter_error(self.get(coord.get_neighbor(VoxelAxis::PosiZ)))?,
            nega_z : filter_error(self.get(coord.get_neighbor(VoxelAxis::NegaZ)))?,
        })
    }
}

/// Any VoxelStorage which has defined, finite bounds.
/// Must provide a valid voxel for any position within
/// the range provided by get_bounds().
/// Usually, this implies that the voxel storage is not paged.
pub trait VoxelStorageBounded<T: Voxel, P: VoxelCoord> : VoxelStorage<T, P> { 
    fn get_bounds(&self) -> VoxelRange<P>;
}

/// Copy voxels from one storage to another.
#[allow(dead_code)]
pub fn voxel_blit<T: Voxel, P: VoxelCoord>(source_range : VoxelRange<P>, source: &dyn VoxelStorage<T, P>,
                                                dest_origin: VoxelPos<P>, dest: &mut dyn VoxelStorage<T,P>)  -> Result<(), VoxelError> {
    for pos in source_range {
        let voxel = source.get(pos)?;
        let offset_pos = (pos - source_range.lower) + dest_origin;
        dest.set(offset_pos, voxel)?;
    }
    return Ok(());
}