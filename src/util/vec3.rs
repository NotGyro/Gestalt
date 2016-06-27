extern crate std;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};

#[derive(Clone, Debug)]
pub struct Vec3<T : Eq + Ord + Add + Sub + Mul + Div> {
	pub x: T, pub y: T, pub z: T,
}