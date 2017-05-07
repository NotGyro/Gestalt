extern crate std;
extern crate num;

use std::string::String;
use std::vec::Vec;
use std::marker::Copy;
use util::voxelutil::VoxelPos;
use util::voxelutil::VoxelRange;
use std::io;
use std::io::prelude::*;

//Previously, we used these for voxel position types:
//use std::ops::{Add, Sub, Mul, Div};
//use std::cmp::{Ord, Eq};

use num::Integer;
use num::traits::identities::One;
use num::traits::identities::Zero;


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

pub trait VoxelStorage<T: Clone, P = u16> where P : Copy + Integer {
    fn get(&self, x: P, y: P, z: P) -> Option<T>;
    fn getv(&self, coord: VoxelPos<P>) -> Option<T> {
        self.get(coord.x, coord.y, coord.z)
    }

    fn set(&mut self, x: P, y: P, z: P, value: T);
    fn setv(&mut self, coord: VoxelPos<P>, value: T) {
        self.set(coord.x, coord.y, coord.z, value);
    }
}

pub trait VoxelStorageIOAble<T : Clone, P = u16> : VoxelStorage<T, P> where P : Copy + Integer {
    fn load<R: Read + Sized>(&mut self, reader: &mut R);
    fn save<W: Write + Sized>(&self, writer: &mut W) -> Result<usize, std::io::Error>;
}


/// Any VoxelStorage which has defined, finite bounds.
/// Must provide a valid voxel for any position within
/// the range provided by get_bounds().
/// Usually, this implies that the voxel storage is not paged.

pub trait VoxelStorageBounded<T: Clone, P = u16> : VoxelStorage<T, P> where P : Copy + Integer {
    fn get_bounds(&self) -> VoxelRange<P>;
}

/* // TODO: 
struct RunLengthVoxel<T: Copy> {
    pub data: Vec<T>,
    pub length: usize,
}
*/
