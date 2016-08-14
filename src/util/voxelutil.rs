extern crate std;

use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};
use std::iter::{Iterator, IntoIterator};
use std::num::{One};

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoxelPos<T : Copy + Eq + Ord + Add<T, Output=T> + Sub + Mul + Div> {
	pub x: T, pub y: T, pub z: T,
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct VoxelRange<T : Copy + Eq + Ord + Add<T, Output=T> + Sub + Mul + Div> {
	pub upper : VoxelPos<T>, pub lower : VoxelPos<T>,
}
impl <T> VoxelRange<T> where T : One + Copy + Eq + Ord + Add<T, Output=T> + Sub + Mul + Div { 
    pub fn get_iterator(&self) -> VoxelRangeIter<T> { 
        VoxelRangeIter { range : *self, pos : Some(self.lower) }
    }
}

impl <T> IntoIterator for VoxelRange<T> where T : One + Copy + Eq + Ord + Add<T, Output=T> + Sub + Mul + Div { 
    type Item = VoxelPos<T>;
    type IntoIter = VoxelRangeIter<T>;
    fn into_iter(self) -> VoxelRangeIter<T> {
        self.get_iterator()
    }
}

pub struct VoxelRangeIter<T : Copy + Eq + Ord + Add<T, Output=T> + Sub + Mul + Div> {
    range : VoxelRange<T>,
    pos : Option<VoxelPos<T>>,
}

impl <T> Iterator for VoxelRangeIter<T> where T : One + Copy + Eq + Ord + Add<T, Output=T> + Sub + Mul + Div { 
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

pub enum VoxelAxis {
	PosiX,
	NegaX,
	PosiY,
	NegaY,
	PosiZ,
	NegaZ,
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