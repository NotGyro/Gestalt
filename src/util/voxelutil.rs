extern crate std;
extern crate num;

use std::iter::{Iterator, IntoIterator};

use num::Integer;
use num::traits::identities::One;
use num::traits::identities::Zero;

use std::marker::Copy;
use std::fmt;

use std::ops::Add;
use std::ops::Sub;

/*Previously, we used these for voxel position types: 
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};
use std::num::{One};
And then: T : Copy + Integer
This was kind of a mess, so I'm refactoring it to use num::Integer.
*/

/// Type alias for trait for voxel position types, in case we ever need to change that.
//trait VoxelCoordTrait : Copy + Integer + ... {}
//impl <T> VoxelCoordTrait for T where T: Copy + Integer + ... {}
//Should I refactor it to this? Unsure.

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoxelPos<T : Copy + Integer> {
	pub x: T, pub y: T, pub z: T,
}

impl <T> Add for VoxelPos<T> where T : Copy + Integer + Add<Output=T> {
    type Output = VoxelPos<T>;

    fn add(self, other: VoxelPos<T>) -> VoxelPos<T> {
        VoxelPos { x: self.x + other.x, y: self.y + other.y, z : self.z + other.z }
    }
}

impl <T> Sub for VoxelPos<T> where T : Copy + Integer + Sub<Output=T> {
    type Output = VoxelPos<T>;

    fn sub(self, other: VoxelPos<T>) -> VoxelPos<T> {
        VoxelPos { x: self.x + other.x, y: self.y + other.y, z : self.z + other.z }
    }
}


impl <T> fmt::Display for VoxelPos<T> where T : Copy + Integer + fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoxelRange<T : Copy + Integer> {
	pub upper : VoxelPos<T>, pub lower : VoxelPos<T>,
}

impl <T> VoxelRange<T> where T : Copy + Integer { 
    pub fn get_iterator(&self) -> VoxelRangeIter<T> { 
        VoxelRangeIter { range : *self, pos : Some(self.lower) }
    }

    pub fn contains(&self, point : VoxelPos<T>) -> bool { 
         ( point.x >= self.lower.x ) && ( point.x < self.upper.x ) &&
         ( point.y >= self.lower.y ) && ( point.y < self.upper.y ) &&
         ( point.x >= self.lower.z ) && ( point.z < self.upper.z )
    }
}

impl <T> fmt::Display for VoxelRange<T> where T : Copy + Integer + fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({} to {})", self.lower, self.upper)
    }
}

impl <T> IntoIterator for VoxelRange<T> where T : Copy + Integer { 
    type Item = VoxelPos<T>;
    type IntoIter = VoxelRangeIter<T>;
    fn into_iter(self) -> VoxelRangeIter<T> {
        self.get_iterator()
    }
}

pub struct VoxelRangeIter<T : Copy + Integer> {
    range : VoxelRange<T>,
    pos : Option<VoxelPos<T>>,
}

impl <T> Iterator for VoxelRangeIter<T> where T : Copy + Integer { 
    type Item = VoxelPos<T>;
    fn next(&mut self) -> Option<VoxelPos<T>> { 
        if(self.pos.is_none()) { 
            return None;
        }
        let pos = self.pos.unwrap(); //Cannot panic if is_none() is valid
        let ret = pos;
        let mut x = pos.x;
        let mut y = pos.y;
        let mut z = pos.z;
        
        let mut over = false;
        
        z = z + T::one();
        if(z >= self.range.upper.z) {
            z = self.range.lower.z;
            y = y + T::one();
            if(y >= self.range.upper.y) {
                y = self.range.lower.y;
                x = x + T::one();
                if(x >= self.range.upper.x) { 
                    over = true;
                }
            }
        }
        if(over) { 
            self.pos = None;
        }
        else {
            self.pos = Some(VoxelPos::<T> {x : x, y : y, z : z });
        }
        return Some(ret);
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum VoxelAxis {
	PosiX,
	NegaX,
	PosiY,
	NegaY,
	PosiZ,
	NegaZ,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoxelAxisIter {
    axis : Option<VoxelAxis>,
}
impl VoxelAxisIter { 
    fn new() -> Self { VoxelAxisIter { axis: None } }
}
impl Iterator for VoxelAxisIter { 
    type Item = VoxelAxis;
    fn next(&mut self) -> Option<VoxelAxis> { 
        let mut result = Some(VoxelAxis::PosiX);
        match self.axis {
            None => (), //result = Some(VoxelAxis::PosiX,
            Some(VoxelAxis::PosiX) => result = Some(VoxelAxis::NegaX),
            Some(VoxelAxis::NegaX) => result = Some(VoxelAxis::PosiY),
            Some(VoxelAxis::PosiY) => result = Some(VoxelAxis::NegaY),
            Some(VoxelAxis::NegaY) => result = Some(VoxelAxis::PosiZ),
            Some(VoxelAxis::PosiZ) => result = Some(VoxelAxis::NegaZ),
            Some(VoxelAxis::NegaZ) => result = None,
        }
        self.axis = result;
        return result;
    }
}

impl VoxelAxis {
    fn iter_all() -> VoxelAxisIter { VoxelAxisIter::new() }
    fn opposite(&self) -> Self {
        match *self {
            VoxelAxis::PosiX => return VoxelAxis::NegaX,
            VoxelAxis::NegaX => return VoxelAxis::PosiX,
            VoxelAxis::PosiY => return VoxelAxis::NegaY,
            VoxelAxis::NegaY => return VoxelAxis::PosiY,
            VoxelAxis::PosiZ => return VoxelAxis::NegaZ,
            VoxelAxis::NegaZ => return VoxelAxis::PosiZ,
        }
    }
}

impl <T> VoxelPos<T> where T : Copy + Integer {
   fn  get_neighbor(&self, direction : VoxelAxis) -> VoxelPos<T> {
        match direction {
            VoxelAxis::PosiX => return VoxelPos{x : self.x + T::one(), y : self.y, z : self.z },
            VoxelAxis::NegaX => return VoxelPos{x : self.x - T::one(), y : self.y, z : self.z },
            VoxelAxis::PosiY => return VoxelPos{x : self.x, y : self.y + T::one(), z : self.z },
            VoxelAxis::NegaY => return VoxelPos{x : self.x, y : self.y - T::one(), z : self.z },
            VoxelAxis::PosiZ => return VoxelPos{x : self.x, y : self.y, z : self.z + T::one() },
            VoxelAxis::NegaZ => return VoxelPos{x : self.x, y : self.y, z : self.z - T::one() },
        }
    }
}

#[test]
fn test_voxel_range_iteration() {
    let side1 = 50;
    let side2 = 10;
    let side3 = 25;
    let sz = side1 * side2 * side3;

    let low : VoxelPos<i32> = VoxelPos{x: 0, y: 0, z: 0};
    let high : VoxelPos<i32> = VoxelPos{x: side1 as i32, y: side2 as i32, z: side3 as i32};
    let ran : VoxelRange<i32> = VoxelRange{lower: low, upper: high};

    let mut counter = 0;
    for i in ran {
        assert!( ! (i.x >= side1) );
        assert!( ! (i.y >= side2) );
        assert!( ! (i.z >= side3) );
        counter = counter + 1;
    }
    assert!( counter == sz );
}

#[test]
fn test_axis_iteration() {
    let mut list : Vec<VoxelAxis> = Vec::new();
    for dir in VoxelAxis::iter_all() {
        list.push(dir);
    }
    assert!( list.len() == 6 );
    assert!(list.contains(&VoxelAxis::PosiX));
    assert!(list.contains(&VoxelAxis::NegaX));
    assert!(list.contains(&VoxelAxis::PosiY));
    assert!(list.contains(&VoxelAxis::NegaY));
    assert!(list.contains(&VoxelAxis::PosiZ));
    assert!(list.contains(&VoxelAxis::NegaZ));
}

#[test]
fn test_get_neighbor() {
    let initial : VoxelPos<i32> = VoxelPos{x : 1, y : 4, z : 1};
    let neighbor = initial.get_neighbor(VoxelAxis::PosiZ);
    assert!( neighbor.z == 2 );
}