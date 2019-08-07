extern crate std;
extern crate num;
extern crate serde;

use std::iter::{Iterator, IntoIterator};

use num::Integer;

use std::marker::Copy;
use std::fmt;

use std::ops::Add;
use std::ops::Sub;

use cgmath::{Vector3, Point3};
use std::f32;

use std::convert::From;
use std::convert::Into;


pub trait USizeAble {
    fn as_usize(&self) -> usize;
    fn from_usize(val : usize) -> Self;
}

impl USizeAble for u8 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u8
    }    
}
impl USizeAble for u16 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u16
    }    
}
impl USizeAble for u32 {
    fn as_usize(&self) -> usize {
     From<T>
    }From<T>
    fn from_usize(val : usize) -> Self {
        val as u32
    }    
}
impl USizeAble for u64 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u64
    }    
}

impl<T> From<T> for usize where T : USizeAble {
    fn from(val: T) { val.as_usize() }
}

/// A point in Voxel space. (A cell.)
#[derive(Copy, Serialize, Clone, Debug, Hash, PartialEq, Eq)]
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

macro_rules! vpos {
    ($x:expr, $y:expr, $z:expr) => { VoxelPos { x: $x, y: $y, z : $z } };
}

/// Represents any rectangular cuboid in voxel space.
#[derive(Copy, Serialize, Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoxelRange<T : Copy + Integer> {
	pub upper : VoxelPos<T>, pub lower : VoxelPos<T>,
}

impl <T> VoxelRange<T> where T : Copy + Integer {
    /// Shift / move our position by offset
    pub fn get_shifted(&self, offset : VoxelPos<T>) -> VoxelRange<T> { VoxelRange{ upper: self.upper + offset, lower: self.lower + offset } }
    /// Shift / move our position by offset
    pub fn shift(&mut self, offset: VoxelPos<T>) {
        let shifted = self.get_shifted(offset);
        self.lower = shifted.lower;
        self.upper = shifted.upper;
    }
    /// Get an iterator which will visit each element of this range exactly once.
    pub fn get_iterator(&self) -> VoxelRangeIter<T> { 
        VoxelRangeIter { range : *self, pos : Some(self.lower) }
    }
    /// Get an iterator which will visit every voxel laying along the selected side of your cuboid.
    /// For example, VoxelAxis::NegaZ will visit all of the voxels in this range where z = self.lower.z
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
    /// Does the provided point fall within this VoxelRange?
    pub fn contains(&self, point : VoxelPos<T>) -> bool { 
         ( point.x >= self.lower.x ) && ( point.x < self.upper.x ) &&
         ( point.y >= self.lower.y ) && ( point.y < self.upper.y ) &&
         ( point.z >= self.lower.z ) && ( point.z < self.upper.z )
    }
    /// Gives you the furthest position inside this VoxelRange along the direction you provide.
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

    /// Does the voxel you gave lie along the selected side of this rectangle?
    pub fn is_on_side(&self, point : VoxelPos<T>, side : VoxelAxis) -> bool { 
        let mut edge = self.get_bound(side);
        //Don't trip over off-by-one errors - the positive bounds are one past the valid coordinates. 
        if side.get_sign() == VoxelAxisSign::POSI {
            edge = edge - T::one();
        }
        return point.coord_for_axis(side) == edge;
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
        if self.pos.is_none() { 
            return None;
        }
        let pos = self.pos.unwrap(); //Cannot panic if is_none() is valid
        let ret = pos;
        let mut x = pos.x;
        let mut y = pos.y;
        let mut z = pos.z;
        
        let mut over = false;
        
        z = z + T::one();
        if z >= self.range.upper.z {
            z = self.range.lower.z;
            y = y + T::one();
            if y >= self.range.upper.y {
                y = self.range.lower.y;
                x = x + T::one();
                if x >= self.range.upper.x { 
                    over = true;
                }
            }
        }
        if over { 
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
        if self.pos.is_none() { 
            return None;
        }
        let mut pos = self.pos.unwrap(); //Cannot panic if is_none() is valid

        let mut over = false;
        let ret = pos; // Our "self.pos" as well as the "pos" variable are both for the next loop, really. "Ret" can capture the first element.

        pos = pos.get_neighbor(self.direction1);
        if pos.coord_for_axis(self.direction1) == self.range.get_bound(self.direction1) { //Iterate over our first direction until we hit our first bound
            pos.set_coord_for_axis(self.direction1.opposite(), self.range.get_bound(self.direction1.opposite())); //Return to start of our first direction.
            pos = pos.get_neighbor(self.direction2); //Move forward through our second direction.
            if pos.coord_for_axis(self.direction2) == self.range.get_bound(self.direction2) { //Are we at the end of our second direction? Loop finished. 
                over = true;
            }
        }
        if over { 
            self.pos = None;
        }
        else {
            self.pos = Some(pos);
        }
        return Some(ret);
    }
}

/// A signed direction in voxel space. 
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum VoxelAxis {
	PosiX,
	NegaX,
	PosiY,
	NegaY,
	PosiZ,
	NegaZ,
}

/// Describes an unsigned cartesian axis in 3D space
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum VoxelAxisUnsigned {
	X,
	Y,
	Z,
}

/// Make sure we can "downcast" this enum so it's just the axis and not a direction
impl From<VoxelAxis> for VoxelAxisUnsigned {
    fn from(axis: VoxelAxis) -> VoxelAxisUnsigned {
        match axis {
            VoxelAxis::PosiX => VoxelAxisUnsigned::X,
            VoxelAxis::NegaX => VoxelAxisUnsigned::X,
            VoxelAxis::PosiY => VoxelAxisUnsigned::Y,
            VoxelAxis::NegaY => VoxelAxisUnsigned::Y,
            VoxelAxis::PosiZ => VoxelAxisUnsigned::Z,
            VoxelAxis::NegaZ => VoxelAxisUnsigned::Z,
        }
    }
}

/// Represents the sign of a VoxelAxis.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
enum VoxelAxisSign {
    POSI,
    NEGA,
}

/// An iterator over each of the 6 cardinal directions in voxel space.
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
    /// Gives you an iterator over each of the 6 cardinal directions in voxel space.
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
    fn split(&self) -> (VoxelAxisSign, VoxelAxisUnsigned) { (self.clone().into(), self.get_sign())}
    fn from_parts(sign : VoxelAxisSign, axis : VoxelAxisUnsigned) -> Self {
        match axis {
            VoxelAxisUnsigned::X => { 
                match sign { 
                    POSI => return VoxelAxis::PosiX,
                    NEGA => return VoxelAxis::NegaX,
                }
            },
            VoxelAxisUnsigned::Y => { 
                match sign { 
                    POSI => return VoxelAxis::PosiY,
                    NEGA => return VoxelAxis::NegaY,
                }
            },
            VoxelAxisUnsigned::Z => { 
                match sign { 
                    POSI => return VoxelAxis::PosiZ,
                    NEGA => return VoxelAxis::NegaZ,
                }
            },
        }
    }
}

impl <T> VoxelPos<T> where T : Copy + Integer {
    /// Returns the cell adjacent to this one in the direction passed
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
    /// Along the provided axis, what is our coordinate?
    pub fn coord_for_axis(&self, direction : VoxelAxisUnsigned) -> T {
        match direction {
            VoxelAxisUnsigned::X => return self.x,
            VoxelAxisUnsigned::Y => return self.y,
            VoxelAxisUnsigned::Z => return self.z,
        }
    }
    /// Set our coordinate along the axis you pass.
    pub fn set_coord_for_axis(&mut self, direction : VoxelAxisUnsigned, value: T) {
        match direction {
            VoxelAxisUnsigned::X => self.x = value,
            VoxelAxisUnsigned::Y => self.y = value,
            VoxelAxisUnsigned::Z => self.z = value,
        }
    }
}

#[derive(Clone, Debug)]
pub struct VoxelRaycast {
	pub pos : VoxelPos<i32>,
    t_max : Vector3<f32>, //Where does the ray cross the first voxel boundary? (in all directions)
    t_delta : Vector3<f32>, //How far along do we need to move for the length of that movement to equal the width of a voxel?
    step_dir : VoxelPos<i32>, //Values are only 1 or -1, to determine the sign of the direction the ray is traveling.
    last_direction : VoxelAxisUnsigned,
}

/*
Many thanks to John Amanatides and Andrew Woo for this algorithm, described in "A Fast Voxel Traversal Algorithm for Ray Tracing" (2011)
*/
impl VoxelRaycast {
    pub fn step(&mut self) {
        if self.t_max.x < self.t_max.y {
            if self.t_max.x < self.t_max.z {
                self.pos.x = self.pos.x + self.step_dir.x;
                self.t_max.x= self.t_max.x + self.t_delta.x;
                self.last_direction = VoxelAxisUnsigned::X; //We will correct the sign on this in a get function, rather than in the loop.
            } else  {
                self.pos.z = self.pos.z + self.step_dir.z;
                self.t_max.z= self.t_max.z + self.t_delta.z;
                self.last_direction = VoxelAxisUnsigned::Z; //We will correct the sign on this in a get function, rather than in the loop.
            }
        } else  {
            if self.t_max.y < self.t_max.z {
                self.pos.y = self.pos.y + self.step_dir.y;
                self.t_max.y = self.t_max.y + self.t_delta.y;
                self.last_direction = VoxelAxisUnsigned::Y; //We will correct the sign on this in a get function, rather than in the loop.
            } else  {
                self.pos.z = self.pos.z + self.step_dir.z;
                self.t_max.z= self.t_max.z + self.t_delta.z;
                self.last_direction = VoxelAxisUnsigned::Z; //We will correct the sign on this in a get function, rather than in the loop.
            }
        }
    }
    pub fn get_last_direction(&self) -> VoxelAxis {
        match self.last_direction {
            VoxelAxisUnsigned::X => {
                if self.step_dir.x < 0 {
                    //The reason these are all the opposite of what they seem like they should be is we're getting the side the raycast hit.
                    //The last direction we traveled will be the opposite of the normal of the side we struck.
                    return VoxelAxis::PosiX;
                }
                else {
                    return VoxelAxis::NegaX;
                }
            },
            VoxelAxisUnsigned::Y => {
                if self.step_dir.y < 0 {
                    return VoxelAxis::PosiY;
                }
                else {
                    return VoxelAxis::NegaY;
                }
            },
            VoxelAxisUnsigned::Z => {
                if self.step_dir.z < 0 {
                    return VoxelAxis::PosiZ;
                }
                else {
                    return VoxelAxis::NegaZ;
                }
            },
        }
    }
    pub fn new(origin : Point3<f32>, direction : Vector3<f32>) -> VoxelRaycast {
        //Voxel is assumed to be 1x1x1 in this situation.
        //Set up our step sign variable.
        let mut step_dir : VoxelPos<i32> = VoxelPos{x: 0, y: 0, z : 0};
        if direction.x >= 0.0 {
            step_dir.x = 1;
        }
        else {
            step_dir.x = -1;
        }
        if direction.y >= 0.0 {
            step_dir.y = 1;
        }
        else {
            step_dir.y = -1;
        }
        if direction.z >= 0.0 {
            step_dir.z = 1;
        }
        else {
            step_dir.z = -1;
        }

        let mut voxel_origin : VoxelPos<i32> = VoxelPos{x: origin.x.floor() as i32, y: origin.y.floor() as i32, z: origin.z.floor() as i32};
        //Distance along the ray to the next voxel from our origin
        let next_voxel_boundary = voxel_origin + step_dir;

        //Set up our t_max - distances to next cell
        let mut t_max : Vector3<f32> = Vector3::new(0.0, 0.0, 0.0);
        if direction.x != 0.0 {
            t_max.x = (next_voxel_boundary.x as f32 - origin.x)/direction.x;
        }
        else {
            t_max.x = f32::MAX; //Undefined in this direction
        }
        if direction.y != 0.0 {
            t_max.y = (next_voxel_boundary.y as f32 - origin.y)/direction.y;
        }
        else {
            t_max.y = f32::MAX; //Undefined in this direction
        }
        if direction.z != 0.0 {
            t_max.z = (next_voxel_boundary.z as f32 - origin.z)/direction.z;
        }
        else {
            t_max.z = f32::MAX; //Undefined in this direction
        }

        //Set up our t_delta - movement per iteration.
        //Again, voxel is assumed to be 1x1x1 in this situation.
        let mut t_delta : Vector3<f32> = Vector3::new(0.0, 0.0, 0.0);
        if direction.x != 0.0 {
            t_delta.x = 1.0/(direction.x*step_dir.x as f32);
        }
        else {
            t_delta.x = f32::MAX; //Undefined in this direction
        }
        if direction.y != 0.0 {
            t_delta.y = 1.0/(direction.y*step_dir.y as f32);
        }
        else {
            t_delta.y = f32::MAX; //Undefined in this direction
        }
        if direction.z != 0.0 {
            t_delta.z = 1.0/(direction.z*step_dir.z as f32);
        }
        else {
            t_delta.z = f32::MAX; //Undefined in this direction
        }

        //Resolve some weird sign bugs.
        let mut negative : bool =false;
        let mut step_negative : VoxelPos<i32> = VoxelPos{x: 0, y: 0, z : 0};
        if direction.x<0.0 {
            step_negative.x = -1; negative=true;
        }
        if direction.y<0.0 {
            step_negative.y = -1; negative=true;
        }
        if direction.z<0.0 {
            step_negative.z = -1; negative=true;
        }
        if negative {
            voxel_origin = voxel_origin + step_negative;
        }
        VoxelRaycast { pos : voxel_origin,
            t_max : t_max,
            t_delta : t_delta,
            step_dir : step_dir,
            last_direction : VoxelAxisUnsigned::Z,
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
