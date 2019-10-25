extern crate std;
extern crate num;
//extern crate serde;

use std::iter::{Iterator, IntoIterator};

use self::num::{Integer, Signed, Unsigned};
 
use std::marker::Copy;
use std::fmt;

use std::ops::Add;
use std::ops::Sub;

use cgmath::{Vector3, Point3, BaseNum};
use std::f64;

use std::convert::From;
use std::convert::Into;

use std::default::Default;

use std::cmp;

use std::result::Result;
use std::error::Error;

use serde::{Serialize, Deserialize};

pub trait USizeAble {
    fn as_usize(&self) -> usize;
    fn from_usize(val : usize) -> Self;
}

impl USizeAble for u8 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val : usize) -> Self {
        val as u8
    }    
}
impl USizeAble for u16 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val : usize) -> Self {
        val as u16
    }    
}
impl USizeAble for u32 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val : usize) -> Self {
        val as u32
    }    
}
impl USizeAble for u64 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val : usize) -> Self {
        val as u64
    }    
}

impl USizeAble for usize {
    #[inline]
    fn as_usize(&self) -> usize { *self }
    #[inline]
    fn from_usize(val : usize) -> Self { val }    
}

/// This should panic if you use it on a negative number.
pub trait ToUnsigned<U> {
    fn as_unsigned(&self) -> U;
    fn from_unsigned(val : U) -> Self;
}

impl ToUnsigned<u8> for i8 {
    #[inline]
    fn as_unsigned(&self) -> u8 {
        assert!(self >= &0);
        (*self) as u8
    }
    #[inline]
    fn from_unsigned(val : u8) -> Self {
        val as i8
    }    
}

impl ToUnsigned<u16> for i16 {
    #[inline]
    fn as_unsigned(&self) -> u16 {
        assert!(self >= &0);
        (*self) as u16
    }
    #[inline]
    fn from_unsigned(val : u16) -> Self {
        val as i16
    }
}

impl ToUnsigned<u32> for i32 {
    #[inline]
    fn as_unsigned(&self) -> u32 {
        assert!(self >= &0);
        (*self) as u32
    }
    #[inline]
    fn from_unsigned(val : u32) -> Self {
        val as i32
    }
}
impl ToUnsigned<u64> for i64 {
    #[inline]
    fn as_unsigned(&self) -> u64 {
        assert!(self >= &0);
        (*self) as u64
    }
    #[inline]
    fn from_unsigned(val : u64) -> Self {
        val as i64
    }
}
pub trait ToSigned<S> {
    fn as_signed(&self) -> S;
    fn from_signed(val : S) -> Self;
}

impl <S, U> ToSigned<S> for U where S : ToUnsigned<U>, U : Clone {
    #[inline]
    fn as_signed(&self) -> S { 
        S::from_unsigned(self.clone())
    }
    #[inline]
    fn from_signed(val : S) -> Self {
        val.as_unsigned()
    }
}

pub trait VoxelCoord : 'static + Copy + Integer + fmt::Display + fmt::Debug + num::PrimInt + Default {}
impl<T> VoxelCoord for T where T : 'static + Copy + Integer + fmt::Display + fmt::Debug + num::PrimInt + Default {}

/// A point in Voxel space. (A cell.)
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoxelPos<T : VoxelCoord> {
	pub x: T, pub y: T, pub z: T,
}

impl <T:VoxelCoord + Default> Default for VoxelPos<T> {
    /// Make a new VoxelArray wherein every value is set to T::Default
    #[inline]
    fn default() -> Self { VoxelPos{x : Default::default(),y : Default::default(),z : Default::default(),} }
}


impl <T> Add for VoxelPos<T> where T : VoxelCoord + Add<Output=T> {
    type Output = VoxelPos<T>;
    #[inline]
    fn add(self, other: VoxelPos<T>) -> VoxelPos<T> {
        VoxelPos { x: self.x + other.x, y: self.y + other.y, z : self.z + other.z }
    }
}

impl <T> Sub for VoxelPos<T> where T : VoxelCoord + Sub<Output=T> {
    type Output = VoxelPos<T>;
    #[inline]
    fn sub(self, other: VoxelPos<T>) -> VoxelPos<T> {
        VoxelPos { x: self.x - other.x, y: self.y - other.y, z : self.z - other.z }
    }
}


impl <T> fmt::Display for VoxelPos<T> where T : VoxelCoord + fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl <T> From<(T, T, T)> for VoxelPos<T> where T : VoxelCoord { 
    fn from(tuple: (T, T, T)) -> VoxelPos<T> { VoxelPos { x : tuple.0, y : tuple.1, z : tuple.2} }
}
impl <T> Into<(T, T, T)> for VoxelPos<T> where T : VoxelCoord { 
    fn into(self) -> (T, T, T) { (self.x, self.y, self.z) }
}

impl <T> From<Point3<T>> for VoxelPos<T> where T : VoxelCoord + BaseNum { 
    fn from(point: Point3<T>) -> VoxelPos<T> { VoxelPos { x : point.x, y : point.y, z : point.z} }
}
impl <T> Into<Point3<T>> for VoxelPos<T> where T : VoxelCoord + BaseNum { 
    fn into(self) -> Point3<T> { Point3::new(self.x, self.y, self.z) }
}

macro_rules! vpos {
    ($x:expr, $y:expr, $z:expr) => { VoxelPos { x: $x, y: $y, z : $z } }
}


impl <S, U> ToUnsigned<VoxelPos<U>> for VoxelPos<S> 
    where S : ToUnsigned<U> + VoxelCoord, U : ToSigned<S> + VoxelCoord {
    fn as_unsigned(&self) -> VoxelPos<U> {
        vpos!(self.x.as_unsigned(), self.y.as_unsigned(), self.z.as_unsigned())
    }
    fn from_unsigned(val : VoxelPos<U>) -> Self {
        vpos!(S::from_unsigned(val.x), S::from_unsigned(val.y), S::from_unsigned(val.z))
    }
}

/// Describes the dimensions in voxel units of any arbitrary thing.
/// Functionally identical to VoxelPos, but it's useful to keep track of which is which.
pub type VoxelSize<T> = VoxelPos<T>;

/// Represents any rectangular cuboid in voxel space.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VoxelRange<T : VoxelCoord> {
	pub lower : VoxelPos<T>, pub upper : VoxelPos<T>, 
}

impl <T> VoxelRange<T> where T : VoxelCoord {
    #[inline]
    pub fn get_validated_lower(&self) -> VoxelPos<T> {
        vpos!(cmp::min(self.upper.x, self.lower.x), cmp::min(self.upper.y, self.lower.y), cmp::min(self.upper.z, self.lower.z))
    }
    #[inline]
    pub fn get_validated_upper(&self) -> VoxelPos<T> {
        vpos!(cmp::max(self.upper.x, self.lower.x), cmp::max(self.upper.y, self.lower.y), cmp::max(self.upper.z, self.lower.z))
    }
    /// Make sure that the coordinates in upper are higher numbers than the coordinates in lower and vice-versa
    #[inline]
    pub fn get_validated(&self) -> VoxelRange<T> {
        VoxelRange{lower: self.get_validated_lower(), upper: self.get_validated_upper()}
    }
    #[inline]
    pub fn validate(&mut self) {
        let validated = self.get_validated().clone();
        self.lower = validated.lower;
        self.upper = validated.upper;
    }
    #[inline]
    pub fn new(low : VoxelPos<T>, high : VoxelPos<T>) -> Self {
        let mut range = VoxelRange{lower: low, upper: high};
        range.validate();
        range
    }
    /// Construct a voxel range from origin + size.
    #[inline]
    pub fn new_origin_size(origin : VoxelPos<T>, size : VoxelSize<T>) -> Self {
        let mut range = VoxelRange{lower: origin, upper: origin+size};
        range.validate();
        range
    }
    /// Shift / move our position by offset
    pub fn get_shifted(&self, offset : VoxelPos<T>) -> VoxelRange<T> { VoxelRange{ lower: self.lower + offset, upper: self.upper + offset } }
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
    #[inline]
    pub fn contains(&self, point : VoxelPos<T>) -> bool { 
         ( point.x >= self.lower.x ) && ( point.x < self.upper.x ) &&
         ( point.y >= self.lower.y ) && ( point.y < self.upper.y ) &&
         ( point.z >= self.lower.z ) && ( point.z < self.upper.z )
    }
    /// Take a position in "world" space and return an offset from self.lower, telling you how far the point is from our origin.
    /// Returns None if this is not a local point.
    #[inline]
    pub fn get_local(&self, point : VoxelPos<T>) -> Option<VoxelPos<T>> {
        if ! self.contains(point) { return None; }
        let validated_lower = self.get_validated_lower();
        Some(point - validated_lower)
    }
    /// Gives you the furthest position inside this VoxelRange along the direction you provide.
    #[inline]
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

    /// Returns the size_x, size_y, and size_z of this range.
    #[inline]
    pub fn get_size(&self) -> VoxelSize<T> {
        let new_upper = vpos!(cmp::max(self.upper.x, self.lower.x), cmp::max(self.upper.y, self.lower.y), cmp::max(self.upper.z, self.lower.z));
        let new_lower = vpos!(cmp::min(self.upper.x, self.lower.x), cmp::min(self.upper.y, self.lower.y), cmp::min(self.upper.z, self.lower.z));
        
        new_upper - new_lower
    }

    /// Does the voxel you gave lie along the selected side of this rectangle?
    #[inline]
    pub fn is_on_side(&self, point : VoxelPos<T>, side : VoxelAxis) -> bool { 
        let mut edge = self.get_bound(side);
        //Don't trip over off-by-one errors - the positive bounds are one past the valid coordinates. 
        if side.get_sign() == VoxelAxisSign::POSI {
            edge = edge - T::one();
        }
        return point.coord_for_axis(side.into()) == edge;
    }
}

pub trait VoxelRangeUnsigner<S : ToUnsigned<U> + VoxelCoord, U : ToSigned<S> + VoxelCoord> {
    type MARKERHACK;
    /// Take a position in "world" space and return an offset from self.lower, telling you how far the point is from our origin.
    /// Returns None if this is not a local point.
    /// Converts to unsigned, sicne we can guarantee the offset is positive here - 
    /// it returns the offset from self.lower, and guarantees this point is within our range
    /// which means that point is always higher than self.lower 
    fn get_local_unsigned(&self, point : VoxelPos<S>) -> Option<VoxelPos<U>>;
    /// Size is a scalar, it can only be positive - it is the amount that self.upper is further from self.lower.
    fn get_size_unsigned(&self) -> VoxelSize<U>;
}

impl <S, U> VoxelRangeUnsigner<S, U> for VoxelRange<S> 
    where S : ToUnsigned<U> + VoxelCoord, U : ToSigned<S> + VoxelCoord {
    type MARKERHACK = ();
    /// Take a position in "world" space and return an offset from self.lower, telling you how far the point is from our origin.
    /// Returns None if this is not a local point.
    /// Converts to unsigned, sicne we can guarantee the offset is positive here - 
    /// it returns the offset from self.lower, and guarantees this point is within our range
    /// which means that point is always higher than self.lower 
    #[inline]
    fn get_local_unsigned(&self, point : VoxelPos<S>) -> Option<VoxelPos<U>> {
        let validated_lower = self.get_validated_lower();
        if ! self.contains(point) { return None; }
        let point_after_offset = point - validated_lower;
        Some(point_after_offset.as_unsigned())
    }
    /// Size is a scalar, it can only be positive - it is the amount that self.upper is further from self.lower.
    #[inline]
    fn get_size_unsigned(&self) -> VoxelSize<U> {
        let new_upper = vpos!(cmp::max(self.upper.x, self.lower.x), cmp::max(self.upper.y, self.lower.y), cmp::max(self.upper.z, self.lower.z));
        let new_lower = vpos!(cmp::min(self.upper.x, self.lower.x), cmp::min(self.upper.y, self.lower.y), cmp::min(self.upper.z, self.lower.z));
        
        (new_upper - new_lower).as_unsigned()
    }
}

impl <T> fmt::Display for VoxelRange<T> where T : VoxelCoord + fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({} to {})", self.lower, self.upper)
    }
}

impl <T> IntoIterator for VoxelRange<T> where T : VoxelCoord { 
    type Item = VoxelPos<T>;
    type IntoIter = VoxelRangeIter<T>;
    fn into_iter(self) -> VoxelRangeIter<T> {
        self.get_iterator()
    }
}

pub struct VoxelRangeIter<T : VoxelCoord> {
    range : VoxelRange<T>,
    pos : Option<VoxelPos<T>>,
}

impl <T> Iterator for VoxelRangeIter<T> where T : VoxelCoord { 
    type Item = VoxelPos<T>;
    #[inline]
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

pub struct VoxelSideIter<T : VoxelCoord> {
    range : VoxelRange<T>,
    //origin : VoxelPos<T>,
    direction1 : VoxelAxis,
    direction2 : VoxelAxis,
    pos : Option<VoxelPos<T>>,
}

impl <T> Iterator for VoxelSideIter<T> where T : VoxelCoord + Signed { 
    type Item = VoxelPos<T>;
    fn next(&mut self) -> Option<VoxelPos<T>> { 
        if self.pos.is_none() { 
            return None;
        }
        let mut pos = self.pos.unwrap(); //Cannot panic if is_none() is valid

        let mut over = false;
        let ret = pos; // Our "self.pos" as well as the "pos" variable are both for the next loop, really. "Ret" can capture the first element.

        pos = pos.get_neighbor(self.direction1);
        if pos.coord_for_axis(self.direction1.into()) == self.range.get_bound(self.direction1) { //Iterate over our first direction until we hit our first bound
            pos.set_coord_for_axis(self.direction1.opposite().into(), self.range.get_bound(self.direction1.opposite())); //Return to start of our first direction.
            pos = pos.get_neighbor(self.direction2); //Move forward through our second direction.
            if pos.coord_for_axis(self.direction2.into()) == self.range.get_bound(self.direction2) { //Are we at the end of our second direction? Loop finished. 
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
// Just proxy to debug
impl fmt::Display for VoxelAxis {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Describes an unsigned cartesian axis in 3D space
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum VoxelAxisUnsigned {
	X,
	Y,
	Z,
}

// Just proxy to debug
impl fmt::Display for VoxelAxisUnsigned {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Make sure we can "downcast" this enum so it's just the axis and not a direction
impl From<VoxelAxis> for VoxelAxisUnsigned {
    #[inline]
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
pub enum VoxelAxisSign {
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
    #[inline]
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


macro_rules! voxel_sides_unroll {
    ($side:ident, $b:block)=> { 
        let $side = VoxelAxis::PosiX;
        $b
        let $side = VoxelAxis::NegaX;
        $b
        let $side = VoxelAxis::PosiY;
        $b
        let $side = VoxelAxis::NegaY;
        $b
        let $side = VoxelAxis::PosiZ;
        $b
        let $side = VoxelAxis::NegaZ;
        $b
    };
}

impl VoxelAxis {
    /// Gives you an iterator over each of the 6 cardinal directions in voxel space.
    pub fn iter_all() -> VoxelAxisIter { VoxelAxisIter::new() }
    #[inline]
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
    pub fn get_sign(&self) -> VoxelAxisSign { 
        match *self {
            VoxelAxis::PosiX => return VoxelAxisSign::POSI,
            VoxelAxis::NegaX => return VoxelAxisSign::NEGA,
            VoxelAxis::PosiY => return VoxelAxisSign::POSI,
            VoxelAxis::NegaY => return VoxelAxisSign::NEGA,
            VoxelAxis::PosiZ => return VoxelAxisSign::POSI,
            VoxelAxis::NegaZ => return VoxelAxisSign::NEGA,
        }
    }
    pub fn split(&self) -> (VoxelAxisSign, VoxelAxisUnsigned) { (self.get_sign(), self.clone().into())}
    pub fn from_parts(sign : VoxelAxisSign, axis : VoxelAxisUnsigned) -> Self {
        match axis {
            VoxelAxisUnsigned::X => { 
                match sign { 
                    VoxelAxisSign::POSI => return VoxelAxis::PosiX,
                    VoxelAxisSign::NEGA => return VoxelAxis::NegaX,
                }
            },
            VoxelAxisUnsigned::Y => { 
                match sign { 
                    VoxelAxisSign::POSI => return VoxelAxis::PosiY,
                    VoxelAxisSign::NEGA => return VoxelAxis::NegaY,
                }
            },
            VoxelAxisUnsigned::Z => { 
                match sign { 
                    VoxelAxisSign::POSI => return VoxelAxis::PosiZ,
                    VoxelAxisSign::NEGA => return VoxelAxis::NegaZ,
                }
            },
        }
    }
}

impl <T> VoxelPos<T> where T : VoxelCoord {
    /// Along the provided axis, what is our coordinate?
    #[inline]
    pub fn coord_for_axis(&self, direction : VoxelAxisUnsigned) -> T {
        match direction {
            VoxelAxisUnsigned::X => return self.x,
            VoxelAxisUnsigned::Y => return self.y,
            VoxelAxisUnsigned::Z => return self.z,
        }
    }
    /// Set our coordinate along the axis you pass.
    #[inline]
    pub fn set_coord_for_axis(&mut self, direction : VoxelAxisUnsigned, value: T) {
        match direction {
            VoxelAxisUnsigned::X => self.x = value,
            VoxelAxisUnsigned::Y => self.y = value,
            VoxelAxisUnsigned::Z => self.z = value,
        }
    }
}


/// Signed, we can subtract.
impl <T> VoxelPos<T> where T : VoxelCoord {
    /// Returns the cell adjacent to this one in the direction passed
    #[inline]
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
    /// Sets this cell to be the cell adjacent to this one in the direction passed
    #[inline]
    pub fn go_neighbor(&mut self, direction : VoxelAxis) {
        match direction {
            VoxelAxis::PosiX => self.x = self.x + T::one(),
            VoxelAxis::NegaX => self.x = self.x - T::one(),
            VoxelAxis::PosiY => self.y = self.y + T::one(),
            VoxelAxis::NegaY => self.y = self.y - T::one(),
            VoxelAxis::PosiZ => self.z = self.z + T::one(),
            VoxelAxis::NegaZ => self.z = self.z - T::one(),
        }
    }
}
#[derive(Debug)]
pub struct UnsignedUnderflowError {
    direction : VoxelAxis,
}

impl fmt::Display for UnsignedUnderflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Tried to subtract from unsigned integer which is 0, moving VoxelPos in direction {}", self.direction)
    }
}

impl Error for UnsignedUnderflowError {
    fn description(&self) -> &str {
        "Tried to subtract from unsigned integer which is 0."
    }

    fn cause(&self) -> Option<&dyn Error> { None }
}


/// Unsigned 
impl <T> VoxelPos<T> where T : VoxelCoord + Unsigned {
    /// Returns the cell adjacent to this one in the direction passed
    #[inline]
    pub fn get_neighbor_unsigned(&self, direction : VoxelAxis) -> Result<VoxelPos<T>, UnsignedUnderflowError> {
        match direction {
            VoxelAxis::PosiX => return Ok(VoxelPos{x : self.x + T::one(), y : self.y, z : self.z }),
            VoxelAxis::NegaX => {
                if self.x == T::zero() {
                    return Err(UnsignedUnderflowError{direction : direction});
                }
                return Ok(VoxelPos{x : self.x - T::one(), y : self.y, z : self.z });
            },
            VoxelAxis::PosiY => return Ok(VoxelPos{x : self.x, y : self.y + T::one(), z : self.z }),
            VoxelAxis::NegaY => {
                if self.y == T::zero() {
                    return Err(UnsignedUnderflowError{direction : direction});
                }
                return Ok( VoxelPos{x : self.x, y : self.y - T::one(), z : self.z });
            },
            VoxelAxis::PosiZ => return Ok(VoxelPos{x : self.x, y : self.y, z : self.z + T::one() }),
            VoxelAxis::NegaZ => {
                if self.z == T::zero() {
                    return Err(UnsignedUnderflowError{direction : direction});
                }
                return Ok(VoxelPos{x : self.x, y : self.y, z : self.z - T::one() });
            },
        }
    }
}


#[derive(Clone, Debug)]
pub struct VoxelRaycast {
	pub pos : VoxelPos<i32>,
    t_max : Vector3<f64>, //Where does the ray cross the first voxel boundary? (in all directions)
    t_delta : Vector3<f64>, //How far along do we need to move for the length of that movement to equal the width of a voxel?
    step_dir : VoxelPos<i32>, //Values are only 1 or -1, to determine the sign of the direction the ray is traveling.
    last_direction : VoxelAxisUnsigned,
}

/*
Many thanks to John Amanatides and Andrew Woo for this algorithm, described in "A Fast Voxel Traversal Algorithm for Ray Tracing" (2011)
*/
impl VoxelRaycast {
    #[inline]
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
    #[inline]
    pub fn get_last_direction(&self) -> VoxelAxis {
        match self.last_direction {
            VoxelAxisUnsigned::X => {
                if self.step_dir.x < 0 {
                    return VoxelAxis::NegaX;
                }
                else {
                    return VoxelAxis::PosiX;
                }
            },
            VoxelAxisUnsigned::Y => {
                if self.step_dir.y < 0 {
                    return VoxelAxis::NegaY;
                }
                else {
                    return VoxelAxis::PosiY;
                }
            },
            VoxelAxisUnsigned::Z => {
                if self.step_dir.z < 0 {
                    return VoxelAxis::NegaZ;
                }
                else {
                    return VoxelAxis::PosiZ;
                }
            },
        }
    }
    pub fn new(origin : Point3<f64>, direction : Vector3<f64>) -> VoxelRaycast {
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
        let mut t_max : Vector3<f64> = Vector3::new(0.0, 0.0, 0.0);
        if direction.x != 0.0 {
            t_max.x = (next_voxel_boundary.x as f64 - origin.x)/direction.x;
        }
        else {
            t_max.x = f64::MAX; //Undefined in this direction
        }
        if direction.y != 0.0 {
            t_max.y = (next_voxel_boundary.y as f64 - origin.y)/direction.y;
        }
        else {
            t_max.y = f64::MAX; //Undefined in this direction
        }
        if direction.z != 0.0 {
            t_max.z = (next_voxel_boundary.z as f64 - origin.z)/direction.z;
        }
        else {
            t_max.z = f64::MAX; //Undefined in this direction
        }

        //Set up our t_delta - movement per iteration.
        //Again, voxel is assumed to be 1x1x1 in this situation.
        let mut t_delta : Vector3<f64> = Vector3::new(0.0, 0.0, 0.0);
        if direction.x != 0.0 {
            t_delta.x = 1.0/(direction.x*step_dir.x as f64);
        }
        else {
            t_delta.x = f64::MAX; //Undefined in this direction
        }
        if direction.y != 0.0 {
            t_delta.y = 1.0/(direction.y*step_dir.y as f64);
        }
        else {
            t_delta.y = f64::MAX; //Undefined in this direction
        }
        if direction.z != 0.0 {
            t_delta.z = 1.0/(direction.z*step_dir.z as f64);
        }
        else {
            t_delta.z = f64::MAX; //Undefined in this direction
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
fn test_axis_iteration_unrolled() {
    let mut list : Vec<VoxelAxis> = Vec::new();
    voxel_sides_unroll!(dir, {
        list.push(dir);
    });
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
