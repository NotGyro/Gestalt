extern crate std;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};

#[derive(Copy, Clone, Debug)]
pub struct VoxelPos<T : Copy + Eq + Ord + Add + Sub + Mul + Div> {
	pub x: T, pub y: T, pub z: T,
}

pub enum VoxelAxis {
	PosiX,
	NegaX,
	PosiY,
	NegaY,
	PosiZ,
	NegaZ,
}

