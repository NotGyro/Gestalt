extern crate std;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};

pub struct Coord3<T : Eq + Ord + Add + Sub + Mul + Div> {
	pub x: T, pub y: T, pub z: T,
}