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
        VoxelPos { x: self.x - other.x, y: self.y - other.y, z : self.z - other.z }
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

    pub fn get_side_iterator(&self, side : VoxelAxis) -> VoxelSideIter<T> {
        match side {
            VoxelAxis::PosiX => { 
                return VoxelSideIter { range : *self, 
                direction1 : VoxelAxis::PosiY,
                direction2 : VoxelAxis::PosiZ,
                pos : Some(VoxelPos { x : self.upper.x, y : self.lower.y, z : self.lower.z} ), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                } 
            },
            VoxelAxis::NegaX => { 
                return VoxelSideIter { range : *self, 
                direction1 : VoxelAxis::PosiY,
                direction2 : VoxelAxis::PosiZ,
                pos : Some(VoxelPos { x : self.lower.x, y : self.lower.y, z : self.lower.z} ), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            },
            VoxelAxis::PosiY => { 
                return VoxelSideIter { range : *self, 
                direction1 : VoxelAxis::PosiX,
                direction2 : VoxelAxis::PosiZ,
                pos : Some(VoxelPos { x : self.lower.x, y : self.upper.y, z : self.lower.z} ), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            },
            VoxelAxis::NegaY => { 
                return VoxelSideIter { range : *self, 
                direction1 : VoxelAxis::PosiX,
                direction2 : VoxelAxis::PosiZ,
                pos : Some(VoxelPos { x : self.lower.x, y : self.lower.y, z : self.lower.z} ), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            },
            VoxelAxis::PosiZ => { 
                return VoxelSideIter { range : *self, 
                direction1 : VoxelAxis::PosiX,
                direction2 : VoxelAxis::PosiY,
                pos : Some(VoxelPos { x: self.lower.x, y : self.lower.y, z : self.upper.z} ), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            },
            VoxelAxis::NegaZ => { 
                return VoxelSideIter { range : *self, 
                direction1 : VoxelAxis::PosiX,
                direction2 : VoxelAxis::PosiY,
                pos : Some(VoxelPos { x : self.lower.x, y : self.lower.y, z : self.lower.z} ), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            },
        }
    }

    pub fn contains(&self, point : VoxelPos<T>) -> bool { 
         ( point.x >= self.lower.x ) && ( point.x < self.upper.x ) &&
         ( point.y >= self.lower.y ) && ( point.y < self.upper.y ) &&
         ( point.z >= self.lower.z ) && ( point.z < self.upper.z )
    }

    pub fn get_bound(&self, direction : VoxelAxis) -> T {
        match direction {
            VoxelAxis::PosiX => return self.upper.x,
            VoxelAxis::PosiY => return self.upper.y,
            VoxelAxis::PosiZ => return self.upper.z,
            VoxelAxis::NegaX => return self.lower.x,
            VoxelAxis::NegaY => return self.lower.y,
            VoxelAxis::NegaZ => return self.lower.z,
        }
    }
    pub fn is_on_side(&self, point : VoxelPos<T>, side : VoxelAxis) -> bool { 
        let mut edge = self.get_bound(side);
        //Don't trip over off-by-one errors - the positive bounds are one past the valid coordinates. 
        if(side.get_sign() == VoxelAxisSign::POSI) {
            edge = edge - T::one();
        }
        return (point.coord_for_axis(side) == edge);
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

pub struct VoxelSideIter<T : Copy + Integer> {
    range : VoxelRange<T>,
    //origin : VoxelPos<T>,
    direction1 : VoxelAxis,
    direction2 : VoxelAxis,
    pos : Option<VoxelPos<T>>,
}

impl <T> Iterator for VoxelSideIter<T> where T : Copy + Integer { 
    type Item = VoxelPos<T>;
    fn next(&mut self) -> Option<VoxelPos<T>> { 
        if(self.pos.is_none()) { 
            return None;
        }
        let mut pos = self.pos.unwrap(); //Cannot panic if is_none() is valid

        let mut over = false;
        let ret = pos; // Our "self.pos" as well as the "pos" variable are both for the next loop, really. "Ret" can capture the first element.

        pos = pos.get_neighbor(self.direction1);
        if(pos.coord_for_axis(self.direction1) == self.range.get_bound(self.direction1)) { //Iterate over our first direction until we hit our first bound
            pos.set_coord_for_axis(self.direction1.opposite(), self.range.get_bound(self.direction1.opposite())); //Return to start of our first direction.
            pos = pos.get_neighbor(self.direction2); //Move forward through our second direction.
            if(pos.coord_for_axis(self.direction2) == self.range.get_bound(self.direction2)) { //Are we at the end of our second direction? Loop finished. 
                over = true;
            }
        }
        if(over) { 
            self.pos = None;
        }
        else {
            self.pos = Some(pos);
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
pub enum VoxelAxisUnsigned {
	X,
	Y,
	Z,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
enum VoxelAxisSign {
    POSI,
    NEGA,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoxelAxisIter {
    axis : Option<VoxelAxis>,
}
impl VoxelAxisIter { 
    pub fn new() -> Self { VoxelAxisIter { axis: None } }
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
    pub fn iter_all() -> VoxelAxisIter { VoxelAxisIter::new() }
    pub fn opposite(&self) -> Self {
        match *self {
            VoxelAxis::PosiX => return VoxelAxis::NegaX,
            VoxelAxis::NegaX => return VoxelAxis::PosiX,
            VoxelAxis::PosiY => return VoxelAxis::NegaY,
            VoxelAxis::NegaY => return VoxelAxis::PosiY,
            VoxelAxis::PosiZ => return VoxelAxis::NegaZ,
            VoxelAxis::NegaZ => return VoxelAxis::PosiZ,
        }
    }
    fn get_sign(&self) -> VoxelAxisSign { 
        match *self {
            VoxelAxis::PosiX => return VoxelAxisSign::POSI,
            VoxelAxis::NegaX => return VoxelAxisSign::NEGA,
            VoxelAxis::PosiY => return VoxelAxisSign::POSI,
            VoxelAxis::NegaY => return VoxelAxisSign::NEGA,
            VoxelAxis::PosiZ => return VoxelAxisSign::POSI,
            VoxelAxis::NegaZ => return VoxelAxisSign::NEGA,
        }
    }
}

impl <T> VoxelPos<T> where T : Copy + Integer {
   pub fn get_neighbor(&self, direction : VoxelAxis) -> VoxelPos<T> {
        match direction {
            VoxelAxis::PosiX => return VoxelPos{x : self.x + T::one(), y : self.y, z : self.z },
            VoxelAxis::NegaX => return VoxelPos{x : self.x - T::one(), y : self.y, z : self.z },
            VoxelAxis::PosiY => return VoxelPos{x : self.x, y : self.y + T::one(), z : self.z },
            VoxelAxis::NegaY => return VoxelPos{x : self.x, y : self.y - T::one(), z : self.z },
            VoxelAxis::PosiZ => return VoxelPos{x : self.x, y : self.y, z : self.z + T::one() },
            VoxelAxis::NegaZ => return VoxelPos{x : self.x, y : self.y, z : self.z - T::one() },
        }
    }
   pub fn coord_for_axis(&self, direction : VoxelAxis) -> T {
        match direction {
            VoxelAxis::PosiX => return self.x,
            VoxelAxis::NegaX => return self.x,
            VoxelAxis::PosiY => return self.y,
            VoxelAxis::NegaY => return self.y,
            VoxelAxis::PosiZ => return self.z,
            VoxelAxis::NegaZ => return self.z,
        }
    }
   pub fn set_coord_for_axis(&mut self, direction : VoxelAxis, value: T) {
        match direction {
            VoxelAxis::PosiX => self.x = value,
            VoxelAxis::NegaX => self.x = value,
            VoxelAxis::PosiY => self.y = value,
            VoxelAxis::NegaY => self.y = value,
            VoxelAxis::PosiZ => self.z = value,
            VoxelAxis::NegaZ => self.z = value,
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
fn test_side_iteration() {
    let side_x = 50;
    let side_y = 10;
    let side_z = 25;

    let low : VoxelPos<i32> = VoxelPos{x: 0, y: 0, z: 0};
    let high : VoxelPos<i32> = VoxelPos{x: side_x as i32, y: side_y as i32, z: side_z as i32};
    let ran : VoxelRange<i32> = VoxelRange{lower: low, upper: high};

    let mut counter = 0;
    for i in ran.get_side_iterator(VoxelAxis::PosiY) {
        assert!( ! (i.x >= side_x) );
        assert!( ! (i.z >= side_z) );
        assert!( i.y == ran.upper.y );
        counter = counter + 1;
    }
    assert!( counter == (side_x * side_z) );

    
    counter = 0;
    for i in ran.get_side_iterator(VoxelAxis::NegaX) {
        assert!( ! (i.y >= side_y) );
        assert!( ! (i.z >= side_z) );
        assert!( i.x == ran.lower.x );
        counter = counter + 1;
    }
    assert!( counter == (side_y * side_z) );
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

#[test]
fn test_contains() {

    let low : VoxelPos<i32> = VoxelPos{x: -40, y: -40, z: -40};
    let high : VoxelPos<i32> = VoxelPos{x: -10 , y: -10, z: -10};
    let ran : VoxelRange<i32> = VoxelRange{lower: low, upper: high};

    for i in ran {
        assert!( ran.contains(i) );
    }
}