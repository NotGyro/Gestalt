// This file is the oldest surviving element of the Gestalt engine. It's practically a dinosaur!

use std::iter::{IntoIterator, Iterator};

use num::{Integer, Signed, Unsigned};

use std::fmt;
use std::marker::Copy;

use std::ops::Add;
use std::ops::Sub;

use std::convert::From;
use std::convert::Into;

use std::default::Default;

use std::cmp;

use std::error::Error;
use std::result::Result;

use serde::{Deserialize, Serialize};

pub trait USizeAble {
    fn as_usize(&self) -> usize;
    fn from_usize(val: usize) -> Self;
}

impl USizeAble for u8 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val: usize) -> Self {
        val as u8
    }
}
impl USizeAble for u16 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val: usize) -> Self {
        val as u16
    }
}
impl USizeAble for u32 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val: usize) -> Self {
        val as u32
    }
}
impl USizeAble for u64 {
    #[inline]
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    #[inline]
    fn from_usize(val: usize) -> Self {
        val as u64
    }
}

impl USizeAble for usize {
    #[inline]
    fn as_usize(&self) -> usize {
        *self
    }
    #[inline]
    fn from_usize(val: usize) -> Self {
        val
    }
}

/// This should panic if you use it on a negative number.
pub trait ToUnsigned<U> {
    fn as_unsigned(&self) -> U;
    fn from_unsigned(val: U) -> Self;
}

impl ToUnsigned<u8> for i8 {
    #[inline]
    fn as_unsigned(&self) -> u8 {
        assert!(self >= &0);
        (*self) as u8
    }
    #[inline]
    fn from_unsigned(val: u8) -> Self {
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
    fn from_unsigned(val: u16) -> Self {
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
    fn from_unsigned(val: u32) -> Self {
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
    fn from_unsigned(val: u64) -> Self {
        val as i64
    }
}
pub trait ToSigned<S> {
    fn as_signed(&self) -> S;
    fn from_signed(val: S) -> Self;
}

impl<S, U> ToSigned<S> for U
where
    S: ToUnsigned<U>,
    U: Clone,
{
    #[inline]
    fn as_signed(&self) -> S {
        S::from_unsigned(self.clone())
    }
    #[inline]
    fn from_signed(val: S) -> Self {
        val.as_unsigned()
    }
}

pub trait VoxelCoord:
    'static + Copy + Integer + fmt::Display + fmt::Debug + num::PrimInt + Default
{
}
impl<T> VoxelCoord for T where
    T: 'static + Copy + Integer + fmt::Display + fmt::Debug + num::PrimInt + Default
{
}

/// A point in Voxel space. (A cell.)
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoxelPos<T: VoxelCoord> {
    pub x: T,
    pub y: T,
    pub z: T,
}

impl<T: VoxelCoord + Default> Default for VoxelPos<T> {
    /// Make a new VoxelArray wherein every value is set to T::Default
    #[inline]
    fn default() -> Self {
        VoxelPos {
            x: Default::default(),
            y: Default::default(),
            z: Default::default(),
        }
    }
}

impl<T> Add for VoxelPos<T>
where
    T: VoxelCoord + Add<Output = T>,
{
    type Output = VoxelPos<T>;
    #[inline]
    fn add(self, other: VoxelPos<T>) -> VoxelPos<T> {
        VoxelPos {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl<T> Sub for VoxelPos<T>
where
    T: VoxelCoord + Sub<Output = T>,
{
    type Output = VoxelPos<T>;
    #[inline]
    fn sub(self, other: VoxelPos<T>) -> VoxelPos<T> {
        VoxelPos {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl<T> fmt::Display for VoxelPos<T>
where
    T: VoxelCoord + fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl<T> From<(T, T, T)> for VoxelPos<T>
where
    T: VoxelCoord,
{
    fn from(tuple: (T, T, T)) -> VoxelPos<T> {
        VoxelPos {
            x: tuple.0,
            y: tuple.1,
            z: tuple.2,
        }
    }
}
impl<T> From<VoxelPos<T>> for (T, T, T)
where
    T: VoxelCoord,
{
    fn from(pos: VoxelPos<T>) -> Self {
        (pos.x, pos.y, pos.z)
    }
}
/*
impl <T> From<Point3<T>> for VoxelPos<T> where T : VoxelCoord + BaseNum {
    fn from(point: Point3<T>) -> VoxelPos<T> { VoxelPos { x : point.x, y : point.y, z : point.z} }
}
impl <T> Into<Point3<T>> for VoxelPos<T> where T : VoxelCoord + BaseNum {
    fn into(self) -> Point3<T> { Point3::new(self.x, self.y, self.z) }
}
*/
macro_rules! vpos {
    ($x:expr, $y:expr, $z:expr) => {
        VoxelPos {
            x: $x,
            y: $y,
            z: $z,
        }
    };
}

impl<S, U> ToUnsigned<VoxelPos<U>> for VoxelPos<S>
where
    S: ToUnsigned<U> + VoxelCoord,
    U: ToSigned<S> + VoxelCoord,
{
    fn as_unsigned(&self) -> VoxelPos<U> {
        vpos!(
            self.x.as_unsigned(),
            self.y.as_unsigned(),
            self.z.as_unsigned()
        )
    }
    fn from_unsigned(val: VoxelPos<U>) -> Self {
        vpos!(
            S::from_unsigned(val.x),
            S::from_unsigned(val.y),
            S::from_unsigned(val.z)
        )
    }
}

/// Describes the dimensions in voxel units of any arbitrary thing.
/// Functionally identical to VoxelPos, but it's useful to keep track of which is which.
pub type VoxelSize<T> = VoxelPos<T>;

/// Represents any rectangular cuboid in voxel space.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VoxelRange<T: VoxelCoord> {
    pub lower: VoxelPos<T>,
    pub upper: VoxelPos<T>,
}

impl<T> VoxelRange<T>
where
    T: VoxelCoord,
{
    #[inline]
    pub fn get_validated_lower(&self) -> VoxelPos<T> {
        vpos!(
            cmp::min(self.upper.x, self.lower.x),
            cmp::min(self.upper.y, self.lower.y),
            cmp::min(self.upper.z, self.lower.z)
        )
    }
    #[inline]
    pub fn get_validated_upper(&self) -> VoxelPos<T> {
        vpos!(
            cmp::max(self.upper.x, self.lower.x),
            cmp::max(self.upper.y, self.lower.y),
            cmp::max(self.upper.z, self.lower.z)
        )
    }
    /// Make sure that the coordinates in upper are higher numbers than the coordinates in lower and vice-versa
    #[inline]
    pub fn get_validated(&self) -> VoxelRange<T> {
        VoxelRange {
            lower: self.get_validated_lower(),
            upper: self.get_validated_upper(),
        }
    }
    #[inline]
    pub fn validate(&mut self) {
        let validated = self.get_validated();
        self.lower = validated.lower;
        self.upper = validated.upper;
    }
    #[inline]
    pub fn new(low: VoxelPos<T>, high: VoxelPos<T>) -> Self {
        let mut range = VoxelRange {
            lower: low,
            upper: high,
        };
        range.validate();
        range
    }
    /// Construct a voxel range from origin + size.
    #[inline]
    #[allow(dead_code)]
    pub fn new_origin_size(origin: VoxelPos<T>, size: VoxelSize<T>) -> Self {
        let mut range = VoxelRange {
            lower: origin,
            upper: origin + size,
        };
        range.validate();
        range
    }
    /// Shift / move our position by offset
    #[allow(dead_code)]
    pub fn get_shifted(&self, offset: VoxelPos<T>) -> VoxelRange<T> {
        VoxelRange {
            lower: self.lower + offset,
            upper: self.upper + offset,
        }
    }
    /// Shift / move our position by offset
    #[allow(dead_code)]
    pub fn shift(&mut self, offset: VoxelPos<T>) {
        let shifted = self.get_shifted(offset);
        self.lower = shifted.lower;
        self.upper = shifted.upper;
    }
    /// Get an iterator which will visit each element of this range exactly once.
    pub fn get_iterator(&self) -> VoxelRangeIter<T> {
        VoxelRangeIter {
            range: *self,
            pos: Some(self.lower),
        }
    }
    /// Get an iterator which will visit every voxel laying along the selected side of your cuboid.
    /// For example, VoxelAxis::NegaZ will visit all of the voxels in this range where z = self.lower.z
    #[allow(dead_code)]
    pub fn get_side_iterator(&self, side: VoxelSide) -> VoxelSideIter<T> {
        match side {
            VoxelSide::PosiX => {
                VoxelSideIter {
                    range: *self,
                    direction1: VoxelSide::PosiY,
                    direction2: VoxelSide::PosiZ,
                    pos: Some(VoxelPos {
                        x: self.upper.x,
                        y: self.lower.y,
                        z: self.lower.z,
                    }), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            }
            VoxelSide::NegaX => {
                VoxelSideIter {
                    range: *self,
                    direction1: VoxelSide::PosiY,
                    direction2: VoxelSide::PosiZ,
                    pos: Some(VoxelPos {
                        x: self.lower.x,
                        y: self.lower.y,
                        z: self.lower.z,
                    }), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            }
            VoxelSide::PosiY => {
                VoxelSideIter {
                    range: *self,
                    direction1: VoxelSide::PosiX,
                    direction2: VoxelSide::PosiZ,
                    pos: Some(VoxelPos {
                        x: self.lower.x,
                        y: self.upper.y,
                        z: self.lower.z,
                    }), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            }
            VoxelSide::NegaY => {
                VoxelSideIter {
                    range: *self,
                    direction1: VoxelSide::PosiX,
                    direction2: VoxelSide::PosiZ,
                    pos: Some(VoxelPos {
                        x: self.lower.x,
                        y: self.lower.y,
                        z: self.lower.z,
                    }), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            }
            VoxelSide::PosiZ => {
                VoxelSideIter {
                    range: *self,
                    direction1: VoxelSide::PosiX,
                    direction2: VoxelSide::PosiY,
                    pos: Some(VoxelPos {
                        x: self.lower.x,
                        y: self.lower.y,
                        z: self.upper.z,
                    }), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            }
            VoxelSide::NegaZ => {
                VoxelSideIter {
                    range: *self,
                    direction1: VoxelSide::PosiX,
                    direction2: VoxelSide::PosiY,
                    pos: Some(VoxelPos {
                        x: self.lower.x,
                        y: self.lower.y,
                        z: self.lower.z,
                    }), //Direction 1 & 2 must travel away from the origin's z and y. This is very important.
                }
            }
        }
    }
    /// Does the provided point fall within this VoxelRange?
    #[inline]
    pub fn contains(&self, point: VoxelPos<T>) -> bool {
        (point.x >= self.lower.x)
            && (point.x < self.upper.x)
            && (point.y >= self.lower.y)
            && (point.y < self.upper.y)
            && (point.z >= self.lower.z)
            && (point.z < self.upper.z)
    }
    /// Take a position in "world" space and return an offset from self.lower, telling you how far the point is from our origin.
    /// Returns None if this is not a local point.
    #[inline]
    #[allow(dead_code)]
    pub fn get_local(&self, point: VoxelPos<T>) -> Option<VoxelPos<T>> {
        if !self.contains(point) {
            return None;
        }
        let validated_lower = self.get_validated_lower();
        Some(point - validated_lower)
    }
    /// Gives you the furthest position inside this VoxelRange along the direction you provide.
    #[inline]
    pub fn get_bound(&self, direction: VoxelSide) -> T {
        match direction {
            VoxelSide::PosiX => self.upper.x,
            VoxelSide::PosiY => self.upper.y,
            VoxelSide::PosiZ => self.upper.z,
            VoxelSide::NegaX => self.lower.x,
            VoxelSide::NegaY => self.lower.y,
            VoxelSide::NegaZ => self.lower.z,
        }
    }

    /// Returns the size_x, size_y, and size_z of this range.
    #[inline]
    #[allow(dead_code)]
    pub fn get_size(&self) -> VoxelSize<T> {
        let new_upper = vpos!(
            cmp::max(self.upper.x, self.lower.x),
            cmp::max(self.upper.y, self.lower.y),
            cmp::max(self.upper.z, self.lower.z)
        );
        let new_lower = vpos!(
            cmp::min(self.upper.x, self.lower.x),
            cmp::min(self.upper.y, self.lower.y),
            cmp::min(self.upper.z, self.lower.z)
        );

        new_upper - new_lower
    }

    /// Does the voxel you gave lie along the selected side of this rectangle?
    #[inline]
    #[allow(dead_code)]
    pub fn is_on_side(&self, point: VoxelPos<T>, side: VoxelSide) -> bool {
        let mut edge = self.get_bound(side);
        //Don't trip over off-by-one errors - the positive bounds are one past the valid coordinates.
        if side.get_sign() == VoxelAxisSign::POSI {
            edge = edge - T::one();
        }
        point.coord_for_axis(side.into()) == edge
    }
}

pub trait VoxelRangeUnsigner<S: ToUnsigned<U> + VoxelCoord, U: ToSigned<S> + VoxelCoord> {
    type MARKERHACK;
    /// Take a position in "world" space and return an offset from self.lower, telling you how far the point is from our origin.
    /// Returns None if this is not a local point.
    /// Converts to unsigned, sicne we can guarantee the offset is positive here -
    /// it returns the offset from self.lower, and guarantees this point is within our range
    /// which means that point is always higher than self.lower
    fn get_local_unsigned(&self, point: VoxelPos<S>) -> Option<VoxelPos<U>>;
    /// Size is a scalar, it can only be positive - it is the amount that self.upper is further from self.lower.
    fn get_size_unsigned(&self) -> VoxelSize<U>;
}

impl<S, U> VoxelRangeUnsigner<S, U> for VoxelRange<S>
where
    S: ToUnsigned<U> + VoxelCoord,
    U: ToSigned<S> + VoxelCoord,
{
    type MARKERHACK = ();
    /// Take a position in "world" space and return an offset from self.lower, telling you how far the point is from our origin.
    /// Returns None if this is not a local point.
    /// Converts to unsigned, sicne we can guarantee the offset is positive here -
    /// it returns the offset from self.lower, and guarantees this point is within our range
    /// which means that point is always higher than self.lower
    #[inline]
    fn get_local_unsigned(&self, point: VoxelPos<S>) -> Option<VoxelPos<U>> {
        let validated_lower = self.get_validated_lower();
        if !self.contains(point) {
            return None;
        }
        let point_after_offset = point - validated_lower;
        Some(point_after_offset.as_unsigned())
    }
    /// Size is a scalar, it can only be positive - it is the amount that self.upper is further from self.lower.
    #[inline]
    fn get_size_unsigned(&self) -> VoxelSize<U> {
        let new_upper = vpos!(
            cmp::max(self.upper.x, self.lower.x),
            cmp::max(self.upper.y, self.lower.y),
            cmp::max(self.upper.z, self.lower.z)
        );
        let new_lower = vpos!(
            cmp::min(self.upper.x, self.lower.x),
            cmp::min(self.upper.y, self.lower.y),
            cmp::min(self.upper.z, self.lower.z)
        );

        (new_upper - new_lower).as_unsigned()
    }
}

impl<T> fmt::Display for VoxelRange<T>
where
    T: VoxelCoord + fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({} to {})", self.lower, self.upper)
    }
}

impl<T> IntoIterator for VoxelRange<T>
where
    T: VoxelCoord,
{
    type Item = VoxelPos<T>;
    type IntoIter = VoxelRangeIter<T>;
    fn into_iter(self) -> VoxelRangeIter<T> {
        self.get_iterator()
    }
}

pub struct VoxelRangeIter<T: VoxelCoord> {
    range: VoxelRange<T>,
    pos: Option<VoxelPos<T>>,
}

impl<T> Iterator for VoxelRangeIter<T>
where
    T: VoxelCoord,
{
    type Item = VoxelPos<T>;
    #[inline]
    fn next(&mut self) -> Option<VoxelPos<T>> {
        self.pos?;
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
        } else {
            self.pos = Some(VoxelPos::<T> { x, y, z });
        }
        Some(ret)
    }
}

pub struct VoxelSideIter<T: VoxelCoord> {
    range: VoxelRange<T>,
    //origin : VoxelPos<T>,
    direction1: VoxelSide,
    direction2: VoxelSide,
    pos: Option<VoxelPos<T>>,
}

impl<T> Iterator for VoxelSideIter<T>
where
    T: VoxelCoord + Signed,
{
    type Item = VoxelPos<T>;
    fn next(&mut self) -> Option<VoxelPos<T>> {
        self.pos?;
        let mut pos = self.pos.unwrap(); //Cannot panic if is_none() is valid

        let mut over = false;
        let ret = pos; // Our "self.pos" as well as the "pos" variable are both for the next loop, really. "Ret" can capture the first element.

        pos = pos.get_neighbor(self.direction1);
        if pos.coord_for_axis(self.direction1.into()) == self.range.get_bound(self.direction1) {
            //Iterate over our first direction until we hit our first bound
            pos.set_coord_for_axis(
                self.direction1.opposite().into(),
                self.range.get_bound(self.direction1.opposite()),
            ); //Return to start of our first direction.
            pos = pos.get_neighbor(self.direction2); //Move forward through our second direction.
            if pos.coord_for_axis(self.direction2.into()) == self.range.get_bound(self.direction2) {
                //Are we at the end of our second direction? Loop finished.
                over = true;
            }
        }
        if over {
            self.pos = None;
        } else {
            self.pos = Some(pos);
        }
        Some(ret)
    }
}

// (Index + 3) % 6 flips the sign.
macro_rules! posi_x_index {
    () => {
        0
    };
}
macro_rules! posi_y_index {
    () => {
        1
    };
}
macro_rules! posi_z_index {
    () => {
        2
    };
}
macro_rules! nega_x_index {
    () => {
        3
    };
}
macro_rules! nega_y_index {
    () => {
        4
    };
}
macro_rules! nega_z_index {
    () => {
        5
    };
}

pub mod axis {
    use std::fmt;

    //pub mod VoxelAxis {}
    /// A signed direction in voxel space.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    #[repr(u8)]
    pub enum VoxelSide {
        PosiX = posi_x_index!(),
        NegaX = nega_x_index!(),
        PosiY = posi_y_index!(),
        NegaY = nega_y_index!(),
        PosiZ = posi_z_index!(),
        NegaZ = nega_z_index!(),
    }

    impl VoxelSide {
        #[inline(always)]
        pub const fn opposite(&self) -> VoxelSide {
            match &self {
                VoxelSide::PosiX => VoxelSide::NegaX,
                VoxelSide::NegaX => VoxelSide::PosiX,
                VoxelSide::PosiY => VoxelSide::NegaY,
                VoxelSide::NegaY => VoxelSide::PosiY,
                VoxelSide::PosiZ => VoxelSide::NegaZ,
                VoxelSide::NegaZ => VoxelSide::PosiZ,
            }
        }

        /// Gives you an iterator over each of the 6 cardinal directions in voxel space.
        #[allow(dead_code)]
        pub fn iter_all() -> VoxelAxisIter {
            VoxelAxisIter::new()
        }

        #[allow(dead_code)]
        #[inline(always)]
        pub const fn get_sign(&self) -> VoxelAxisSign {
            match *self {
                VoxelSide::PosiX => VoxelAxisSign::POSI,
                VoxelSide::NegaX => VoxelAxisSign::NEGA,
                VoxelSide::PosiY => VoxelAxisSign::POSI,
                VoxelSide::NegaY => VoxelAxisSign::NEGA,
                VoxelSide::PosiZ => VoxelAxisSign::POSI,
                VoxelSide::NegaZ => VoxelAxisSign::NEGA,
            }
        }
        #[allow(dead_code)]
        #[inline(always)]
        pub const fn get_axis(&self) -> VoxelAxis {
            match *self {
                VoxelSide::PosiX => VoxelAxis::X,
                VoxelSide::NegaX => VoxelAxis::X,
                VoxelSide::PosiY => VoxelAxis::Y,
                VoxelSide::NegaY => VoxelAxis::Y,
                VoxelSide::PosiZ => VoxelAxis::Z,
                VoxelSide::NegaZ => VoxelAxis::Z,
            }
        }

        #[allow(dead_code)]
        #[inline(always)]
        pub const fn split(&self) -> (VoxelAxisSign, VoxelAxis) {
            (self.get_sign(), self.get_axis())
        }

        #[allow(dead_code)]
        #[inline(always)]
        pub const fn from_parts(sign: VoxelAxisSign, axis: VoxelAxis) -> Self {
            match axis {
                VoxelAxis::X => match sign {
                    VoxelAxisSign::POSI => VoxelSide::PosiX,
                    VoxelAxisSign::NEGA => VoxelSide::NegaX,
                },
                VoxelAxis::Y => match sign {
                    VoxelAxisSign::POSI => VoxelSide::PosiY,
                    VoxelAxisSign::NEGA => VoxelSide::NegaY,
                },
                VoxelAxis::Z => match sign {
                    VoxelAxisSign::POSI => VoxelSide::PosiZ,
                    VoxelAxisSign::NEGA => VoxelSide::NegaZ,
                },
            }
        }

        #[inline(always)]
        #[allow(dead_code)]
        pub const fn to_id(&self) -> usize {
            match self {
                VoxelSide::PosiX => posi_x_index!(),
                VoxelSide::PosiY => posi_y_index!(),
                VoxelSide::PosiZ => posi_z_index!(),
                VoxelSide::NegaX => nega_x_index!(),
                VoxelSide::NegaY => nega_y_index!(),
                VoxelSide::NegaZ => nega_z_index!(),
            }
        }

        #[inline(always)]
        #[allow(dead_code)]
        pub const fn from_id(val: usize) -> Self {
            match val {
                posi_x_index!() => VoxelSide::PosiX,
                posi_z_index!() => VoxelSide::PosiY,
                posi_y_index!() => VoxelSide::PosiZ,
                nega_x_index!() => VoxelSide::NegaX,
                nega_y_index!() => VoxelSide::NegaY,
                nega_z_index!() => VoxelSide::NegaZ,
                _ => VoxelSide::PosiX, //panic!("It should not be possible to get a side-ID greater than or equal to 6! You passed in {}", num),
            }
        }

        /// If you are looking straight at this side (assuming no roll), what direction is its' local 2D positive-X direction?
        #[allow(dead_code)]
        pub fn get_2d_x(&self) -> VoxelSide {
            match self {
                VoxelSide::PosiX => VoxelSide::PosiZ,
                VoxelSide::NegaX => VoxelSide::PosiZ,
                VoxelSide::PosiY => VoxelSide::PosiX,
                VoxelSide::NegaY => VoxelSide::PosiX,
                VoxelSide::PosiZ => VoxelSide::PosiX,
                VoxelSide::NegaZ => VoxelSide::PosiX,
            }
        }

        /// If you are looking straight at this side (assuming no roll), what direction is its' local 2D positive-Y direction?
        #[allow(dead_code)]
        pub fn get_2d_y(&self) -> VoxelSide {
            match self {
                VoxelSide::PosiX => VoxelSide::PosiY,
                VoxelSide::NegaX => VoxelSide::PosiY,
                VoxelSide::PosiY => VoxelSide::PosiZ,
                VoxelSide::NegaY => VoxelSide::PosiZ,
                VoxelSide::PosiZ => VoxelSide::PosiY,
                VoxelSide::NegaZ => VoxelSide::PosiY,
            }
        }
    }

    /// Describes an unsigned cartesian axis in 3D space
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    #[repr(u8)]
    pub enum VoxelAxis {
        X = 0,
        Y = 1,
        Z = 2,
    }
    // Just proxy to debug
    impl fmt::Display for VoxelSide {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{:?}", self)
        }
    }

    // Just proxy to debug
    impl fmt::Display for VoxelAxis {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "{:?}", self)
        }
    }

    /// Make sure we can "downcast" this enum so it's just the axis and not a direction
    impl From<VoxelSide> for VoxelAxis {
        #[inline(always)]
        fn from(axis: VoxelSide) -> VoxelAxis {
            match axis {
                VoxelSide::PosiX => VoxelAxis::X,
                VoxelSide::NegaX => VoxelAxis::X,
                VoxelSide::PosiY => VoxelAxis::Y,
                VoxelSide::NegaY => VoxelAxis::Y,
                VoxelSide::PosiZ => VoxelAxis::Z,
                VoxelSide::NegaZ => VoxelAxis::Z,
            }
        }
    }

    /// Represents the sign of a VoxelAxis.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    #[allow(dead_code)]
    pub enum VoxelAxisSign {
        POSI,
        NEGA,
    }

    /// An iterator over each of the 6 cardinal directions in voxel space.
    #[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
    pub struct VoxelAxisIter {
        axis: Option<VoxelSide>,
    }
    impl VoxelAxisIter {
        #[allow(dead_code)]
        pub fn new() -> Self {
            VoxelAxisIter { axis: None }
        }
    }
    impl Default for VoxelAxisIter {
        fn default() -> Self {
            Self::new()
        }
    }
    impl Iterator for VoxelAxisIter {
        type Item = VoxelSide;
        #[inline(always)]
        fn next(&mut self) -> Option<VoxelSide> {
            let mut result = Some(VoxelSide::PosiX);
            match self.axis {
                None => (), //result = Some(VoxelAxis::PosiX,
                Some(VoxelSide::PosiX) => result = Some(VoxelSide::NegaX),
                Some(VoxelSide::NegaX) => result = Some(VoxelSide::PosiY),
                Some(VoxelSide::PosiY) => result = Some(VoxelSide::NegaY),
                Some(VoxelSide::NegaY) => result = Some(VoxelSide::PosiZ),
                Some(VoxelSide::PosiZ) => result = Some(VoxelSide::NegaZ),
                Some(VoxelSide::NegaZ) => result = None,
            }
            self.axis = result;
            result
        }
    }
}

pub use axis::VoxelAxis;
pub use axis::VoxelAxisIter;
pub use axis::VoxelAxisSign;
pub use axis::VoxelSide;

//VoxelAxis::PosiX => 0,
//VoxelAxis::PosiY => 1,
//VoxelAxis::PosiZ => 2,
//VoxelAxis::NegaX => 3,
//VoxelAxis::NegaY => 4,
//VoxelAxis::NegaZ => 5,

#[allow(unused_macros)]
macro_rules! voxel_sides_unroll {
    ($side:ident, $b:block) => {{
        const $side: VoxelSide = VoxelSide::PosiX;
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaX;
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::PosiY;
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaY;
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::PosiZ;
        $b
    }
    {
        const $side: VoxelSide = VoxelSide::NegaZ;
        $b
    }};
}

#[allow(unused_macros)]
macro_rules! voxel_side_indicies_unroll {
    ($idx:ident, $b:block) => {{
        const $idx: usize = posi_x_index!();
        $b
    }
    {
        const $idx: usize = nega_x_index!();
        $b
    }
    {
        const $idx: usize = posi_y_index!();
        $b
    }
    {
        const $idx: usize = nega_y_index!();
        $b
    }
    {
        const $idx: usize = posi_z_index!();
        $b
    }
    {
        const $idx: usize = nega_z_index!();
        $b
    }};
}

#[allow(unused_macros)]
macro_rules! enumerated_voxel_side_unroll {
    ($idx:ident, $side:ident, $b:block) => {{
        const $side: VoxelAxis = VoxelAxis::PosiX;
        const $idx: usize = posi_x_index!();
        $b
    }
    {
        const $side: VoxelAxis = VoxelAxis::NegaX;
        const $idx: usize = nega_x_index!();
        $b
    }
    {
        const $side: VoxelAxis = VoxelAxis::PosiY;
        const $idx: usize = posi_y_index!();
        $b
    }
    {
        const $side: VoxelAxis = VoxelAxis::NegaY;
        const $idx: usize = nega_y_index!();
        $b
    }
    {
        const $side: VoxelAxis = VoxelAxis::PosiZ;
        const $idx: usize = posi_z_index!();
        $b
    }
    {
        const $side: VoxelAxis = VoxelAxis::NegaZ;
        const $idx: usize = nega_z_index!();
        $b
    }};
}

impl<T> VoxelPos<T>
where
    T: VoxelCoord,
{
    /// Along the provided axis, what is our coordinate?
    #[inline]
    pub fn coord_for_axis(&self, direction: VoxelAxis) -> T {
        match direction {
            VoxelAxis::X => self.x,
            VoxelAxis::Y => self.y,
            VoxelAxis::Z => self.z,
        }
    }
    /// Set our coordinate along the axis you pass.
    #[inline]
    pub fn set_coord_for_axis(&mut self, direction: VoxelAxis, value: T) {
        match direction {
            VoxelAxis::X => self.x = value,
            VoxelAxis::Y => self.y = value,
            VoxelAxis::Z => self.z = value,
        }
    }
}

/// Signed, we can subtract.
impl<T> VoxelPos<T>
where
    T: VoxelCoord,
{
    /// Returns the cell adjacent to this one in the direction passed
    #[inline(always)]
    pub fn get_neighbor(&self, direction: VoxelSide) -> VoxelPos<T> {
        match direction {
            VoxelSide::PosiX => VoxelPos {
                x: self.x + T::one(),
                y: self.y,
                z: self.z,
            },
            VoxelSide::NegaX => VoxelPos {
                x: self.x - T::one(),
                y: self.y,
                z: self.z,
            },
            VoxelSide::PosiY => VoxelPos {
                x: self.x,
                y: self.y + T::one(),
                z: self.z,
            },
            VoxelSide::NegaY => VoxelPos {
                x: self.x,
                y: self.y - T::one(),
                z: self.z,
            },
            VoxelSide::PosiZ => VoxelPos {
                x: self.x,
                y: self.y,
                z: self.z + T::one(),
            },
            VoxelSide::NegaZ => VoxelPos {
                x: self.x,
                y: self.y,
                z: self.z - T::one(),
            },
        }
    }
    /// Sets this cell to be the cell adjacent to this one in the direction passed
    #[inline(always)]
    #[allow(dead_code)]
    pub fn go_neighbor(&mut self, direction: VoxelSide) {
        match direction {
            VoxelSide::PosiX => self.x = self.x + T::one(),
            VoxelSide::NegaX => self.x = self.x - T::one(),
            VoxelSide::PosiY => self.y = self.y + T::one(),
            VoxelSide::NegaY => self.y = self.y - T::one(),
            VoxelSide::PosiZ => self.z = self.z + T::one(),
            VoxelSide::NegaZ => self.z = self.z - T::one(),
        }
    }
}
#[derive(Debug)]
pub struct UnsignedUnderflowError {
    direction: VoxelSide,
}

impl fmt::Display for UnsignedUnderflowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Tried to subtract from unsigned integer which is 0, moving VoxelPos in direction {}",
            self.direction
        )
    }
}

impl Error for UnsignedUnderflowError {
    fn description(&self) -> &str {
        "Tried to subtract from unsigned integer which is 0."
    }

    fn cause(&self) -> Option<&dyn Error> {
        None
    }
}

/// Unsigned
impl<T> VoxelPos<T>
where
    T: VoxelCoord + Unsigned,
{
    /// Returns the cell adjacent to this one in the direction passed
    #[inline]
    #[allow(dead_code)]
    pub fn get_neighbor_unsigned(
        &self,
        direction: VoxelSide,
    ) -> Result<VoxelPos<T>, UnsignedUnderflowError> {
        match direction {
            VoxelSide::PosiX => Ok(VoxelPos {
                x: self.x + T::one(),
                y: self.y,
                z: self.z,
            }),
            VoxelSide::NegaX => {
                if self.x == T::zero() {
                    return Err(UnsignedUnderflowError { direction });
                }
                Ok(VoxelPos {
                    x: self.x - T::one(),
                    y: self.y,
                    z: self.z,
                })
            }
            VoxelSide::PosiY => Ok(VoxelPos {
                x: self.x,
                y: self.y + T::one(),
                z: self.z,
            }),
            VoxelSide::NegaY => {
                if self.y == T::zero() {
                    return Err(UnsignedUnderflowError { direction });
                }
                Ok(VoxelPos {
                    x: self.x,
                    y: self.y - T::one(),
                    z: self.z,
                })
            }
            VoxelSide::PosiZ => Ok(VoxelPos {
                x: self.x,
                y: self.y,
                z: self.z + T::one(),
            }),
            VoxelSide::NegaZ => {
                if self.z == T::zero() {
                    return Err(UnsignedUnderflowError { direction });
                }
                Ok(VoxelPos {
                    x: self.x,
                    y: self.y,
                    z: self.z - T::one(),
                })
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct VoxelRaycast {
    pub pos: VoxelPos<i32>,
    t_max: glam::Vec3, //Where does the ray cross the first voxel boundary? (in all directions)
    t_delta: glam::Vec3, //How far along do we need to move for the length of that movement to equal the width of a voxel?
    step_dir: VoxelPos<i32>, //Values are only 1 or -1, to determine the sign of the direction the ray is traveling.
    last_direction: VoxelAxis,
}

/*
Many thanks to John Amanatides and Andrew Woo for this algorithm, described in "A Fast Voxel Traversal Algorithm for Ray Tracing" (2011)
*/
impl VoxelRaycast {
    #[inline]
    #[allow(dead_code)]
    fn step_x(&mut self) {
        self.pos.x += self.step_dir.x;
        self.t_max.x += self.t_delta.x;
        self.last_direction = VoxelAxis::X; //We will correct the sign on this in a get function, rather than in the loop.
    }
    #[inline]
    #[allow(dead_code)]
    fn step_y(&mut self) {
        self.pos.y += self.step_dir.y;
        self.t_max.y += self.t_delta.y;
        self.last_direction = VoxelAxis::Y; //We will correct the sign on this in a get function, rather than in the loop.
    }
    #[inline]
    #[allow(dead_code)]
    fn step_z(&mut self) {
        self.pos.z += self.step_dir.z;
        self.t_max.z += self.t_delta.z;
        self.last_direction = VoxelAxis::Z; //We will correct the sign on this in a get function, rather than in the loop.
    }
    #[inline]
    #[allow(dead_code)]
    pub fn step(&mut self) {
        if (self.t_max.x < self.t_max.y) && (self.t_max.x < self.t_max.z) {
            self.step_x();
        } else if (self.t_max.y < self.t_max.x) && (self.t_max.y < self.t_max.z) {
            self.step_y();
        } else if (self.t_max.z < self.t_max.x) && (self.t_max.z < self.t_max.y) {
            self.step_z();
        }
    }
    #[inline]
    #[allow(dead_code)]
    pub fn get_last_direction(&self) -> VoxelSide {
        match self.last_direction {
            VoxelAxis::X => {
                if self.step_dir.x < 0 {
                    VoxelSide::NegaX
                } else {
                    VoxelSide::PosiX
                }
            }
            VoxelAxis::Y => {
                if self.step_dir.y < 0 {
                    VoxelSide::NegaY
                } else {
                    VoxelSide::PosiY
                }
            }
            VoxelAxis::Z => {
                if self.step_dir.z < 0 {
                    VoxelSide::NegaZ
                } else {
                    VoxelSide::PosiZ
                }
            }
        }
    }
    #[allow(dead_code)]
    pub fn new(origin: glam::Vec3, direction: glam::Vec3) -> VoxelRaycast {
        //Voxel is assumed to be 1x1x1 in this situation.
        //Set up our step sign variable.
        let mut step_dir: VoxelPos<i32> = VoxelPos { x: 0, y: 0, z: 0 };
        if direction.x >= 0.0 {
            step_dir.x = 1;
        } else {
            step_dir.x = -1;
        }
        if direction.y >= 0.0 {
            step_dir.y = 1;
        } else {
            step_dir.y = -1;
        }
        if direction.z >= 0.0 {
            step_dir.z = 1;
        } else {
            step_dir.z = -1;
        }

        let voxel_origin: VoxelPos<i32> = VoxelPos {
            x: origin.x.floor() as i32,
            y: origin.y.floor() as i32,
            z: origin.z.floor() as i32,
        };

        //Resolve some weird sign bugs.
        /*let mut negative : bool =false;
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
        }*/

        //Distance along the ray to the next voxel from our origin
        let next_voxel_boundary = voxel_origin + step_dir;

        //Set up our t_max - distances to next cell
        let mut t_max: glam::Vec3 = glam::Vec3::new(0.0, 0.0, 0.0);
        if direction.x != 0.0 {
            t_max.x = (next_voxel_boundary.x as f32 - origin.x) / direction.x;
        } else {
            t_max.x = f32::MAX; //Undefined in this direction
        }
        if direction.y != 0.0 {
            t_max.y = (next_voxel_boundary.y as f32 - origin.y) / direction.y;
        } else {
            t_max.y = f32::MAX; //Undefined in this direction
        }
        if direction.z != 0.0 {
            t_max.z = (next_voxel_boundary.z as f32 - origin.z) / direction.z;
        } else {
            t_max.z = f32::MAX; //Undefined in this direction
        }

        //Set up our t_delta - movement per iteration.
        //Again, voxel is assumed to be 1x1x1 in this situation.
        let mut t_delta: glam::Vec3 = glam::Vec3::new(0.0, 0.0, 0.0);
        if direction.x != 0.0 {
            t_delta.x = 1.0 / (direction.x * step_dir.x as f32);
        } else {
            t_delta.x = f32::MAX; //Undefined in this direction
        }
        if direction.y != 0.0 {
            t_delta.y = 1.0 / (direction.y * step_dir.y as f32);
        } else {
            t_delta.y = f32::MAX; //Undefined in this direction
        }
        if direction.z != 0.0 {
            t_delta.z = 1.0 / (direction.z * step_dir.z as f32);
        } else {
            t_delta.z = f32::MAX; //Undefined in this direction
        }

        VoxelRaycast {
            pos: voxel_origin,
            t_max,
            t_delta,
            step_dir,
            last_direction: VoxelAxis::Z,
        }
    }
    pub fn hit_side(&self) -> VoxelSide {
        match self.last_direction {
            VoxelAxis::X => {
                if self.step_dir.x >= 0 { 
                    VoxelSide::PosiX
                }
                else { 
                    VoxelSide::NegaX
                }
            },
            VoxelAxis::Y => {
                if self.step_dir.y >= 0 { 
                    VoxelSide::PosiY
                }
                else { 
                    VoxelSide::NegaY
                }
            },
            VoxelAxis::Z => {
                if self.step_dir.z >= 0 { 
                    VoxelSide::PosiZ
                }
                else { 
                    VoxelSide::NegaZ
                }
            },
        }.opposite()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SidesArray<T>
where
    T: Clone + std::fmt::Debug,
{
    pub data: [T; 6],
}

impl<T> SidesArray<T>
where
    T: Clone + std::fmt::Debug,
{
    pub fn new_uniform(value: &T) -> Self {
        SidesArray {
            data: [
                value.clone(),
                value.clone(),
                value.clone(),
                value.clone(),
                value.clone(),
                value.clone(),
            ],
        }
    }
    pub fn new(posi_x: T, posi_y: T, posi_z: T, nega_x: T, nega_y: T, nega_z: T) -> Self {
        SidesArray {
            data: [posi_x, posi_y, posi_z, nega_x, nega_y, nega_z],
        }
    }
    pub const fn get(&self, dir: VoxelSide) -> &T {
        &self.data[dir.to_id()]
    }
    pub const fn get_i(&self, i: usize) -> &T {
        &self.data[i]
    }
    pub fn set(&mut self, value: T, dir: VoxelSide) {
        (*self.data.get_mut(dir.to_id()).unwrap()) = value;
    }
    pub fn set_i(&mut self, value: T, i: usize) {
        (*self.data.get_mut(i).unwrap()) = value;
    }

    pub fn iter<'a>(&'a self) -> SidesArrayIterator<'a, T> {
        SidesArrayIterator {
            next_index: 0,
            data: &self,
        }
    }
}

impl<T> Default for SidesArray<T>
where
    T: Clone + std::fmt::Debug + Default,
{
    fn default() -> Self {
        Self {
            data: Default::default(),
        }
    }
}

impl<T> Copy for SidesArray<T> where T: Clone + std::fmt::Debug + Copy {}

pub struct SidesArrayIterator<'a, T>
where
    T: Clone + std::fmt::Debug,
{
    next_index: usize,
    data: &'a SidesArray<T>,
}

impl<'a, T> Iterator for SidesArrayIterator<'a, T>
where
    T: Clone + std::fmt::Debug,
{
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        if self.next_index < 6 {
            let output = self.data.get_i(self.next_index);
            self.next_index += 1;

            Some(output)
        } else {
            // Past the end.
            None
        }
    }
}

#[test]
fn test_voxel_range_iteration() {
    let side1 = 50;
    let side2 = 10;
    let side3 = 25;
    let sz = side1 * side2 * side3;

    let low: VoxelPos<i32> = VoxelPos { x: 0, y: 0, z: 0 };
    let high: VoxelPos<i32> = VoxelPos {
        x: side1 as i32,
        y: side2 as i32,
        z: side3 as i32,
    };
    let ran: VoxelRange<i32> = VoxelRange {
        lower: low,
        upper: high,
    };

    let mut counter = 0;
    for i in ran {
        assert!(!(i.x >= side1));
        assert!(!(i.y >= side2));
        assert!(!(i.z >= side3));
        counter += 1;
    }
    assert!(counter == sz);
}

#[test]
fn test_side_iteration() {
    let side_x = 50;
    let side_y = 10;
    let side_z = 25;

    let low: VoxelPos<i32> = VoxelPos { x: 0, y: 0, z: 0 };
    let high: VoxelPos<i32> = VoxelPos {
        x: side_x as i32,
        y: side_y as i32,
        z: side_z as i32,
    };
    let ran: VoxelRange<i32> = VoxelRange {
        lower: low,
        upper: high,
    };

    let mut counter = 0;
    for i in ran.get_side_iterator(VoxelSide::PosiY) {
        assert!(!(i.x >= side_x));
        assert!(!(i.z >= side_z));
        assert!(i.y == ran.upper.y);
        counter += 1;
    }
    assert!(counter == (side_x * side_z));

    counter = 0;
    for i in ran.get_side_iterator(VoxelSide::NegaX) {
        assert!(!(i.y >= side_y));
        assert!(!(i.z >= side_z));
        assert!(i.x == ran.lower.x);
        counter += 1;
    }
    assert!(counter == (side_y * side_z));
}

#[test]
fn test_axis_iteration() {
    let mut list: Vec<VoxelSide> = Vec::new();
    for dir in VoxelSide::iter_all() {
        list.push(dir);
    }
    assert!(list.len() == 6);
    assert!(list.contains(&VoxelSide::PosiX));
    assert!(list.contains(&VoxelSide::NegaX));
    assert!(list.contains(&VoxelSide::PosiY));
    assert!(list.contains(&VoxelSide::NegaY));
    assert!(list.contains(&VoxelSide::PosiZ));
    assert!(list.contains(&VoxelSide::NegaZ));
}
#[test]
fn test_axis_iteration_unrolled() {
    let mut list: Vec<VoxelSide> = Vec::new();
    voxel_sides_unroll!(DIR, {
        list.push(DIR);
    });
    assert!(list.len() == 6);
    assert!(list.contains(&VoxelSide::PosiX));
    assert!(list.contains(&VoxelSide::NegaX));
    assert!(list.contains(&VoxelSide::PosiY));
    assert!(list.contains(&VoxelSide::NegaY));
    assert!(list.contains(&VoxelSide::PosiZ));
    assert!(list.contains(&VoxelSide::NegaZ));
}

#[test]
fn test_get_neighbor() {
    let initial: VoxelPos<i32> = VoxelPos { x: 1, y: 4, z: 1 };
    let neighbor = initial.get_neighbor(VoxelSide::PosiZ);
    assert!(neighbor.z == 2);
}

#[test]
fn test_contains() {
    let low: VoxelPos<i32> = VoxelPos {
        x: -40,
        y: -40,
        z: -40,
    };
    let high: VoxelPos<i32> = VoxelPos {
        x: -10,
        y: -10,
        z: -10,
    };
    let ran: VoxelRange<i32> = VoxelRange {
        lower: low,
        upper: high,
    };

    for i in ran {
        assert!(ran.contains(i));
    }
}
