extern crate std;
extern crate num;

use std::marker::Copy;

//use std::ops::{Add, Sub, Mul, Div};
//use std::cmp::{Ord, Eq};
use std::mem::size_of;
use voxel::voxelstorage::*;
use util::numbers::USizeAble;
use util::voxelutil::*;
//use voxel::voxelstorage::ContiguousVS;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::borrow::Cow;

use num::Integer;
use num::traits::identities::One;
use num::traits::identities::Zero;
use num::Unsigned;

/// A 3D packed array of voxels - it's a single flat buffer in memory,
/// which is indexed by voxel positions with some math done on them. 
/// Should have a fixed, constant size after creation.
#[derive(Clone, Debug)]
pub struct VoxelArray<T: Clone> {
    size_x: u16, size_y: u16, size_z: u16,
    data: Vec<T>,
    bounds : VoxelRange<u16>,
}

impl <T:Clone> VoxelArray<T> {

	pub fn load_new(szx: u16, szy: u16, szz: u16, dat: Vec<T>) -> Box<VoxelArray<T>> {
		let bnd = VoxelRange::<u16> { lower : VoxelPos::<u16>{x : 0, y : 0, z : 0},
              upper : VoxelPos{x : szx, y : szy, z : szy}};
        return Box::new(VoxelArray{size_x: szx, size_y: szy, size_z: szz, 
            data: dat, bounds : bnd});
	}
}

impl <T: Clone> VoxelStorage<T, u16> for VoxelArray<T> {
    fn get(&self, x: u16, y: u16, z: u16) -> Option<T> {
    	//Bounds-check.
    	if (x >= self.size_x) ||
    		(y >= self.size_y) ||
    		(z >= self.size_z)
    	{
    		return None;
    	}
    	//Packed array access
    	let result : Option<&T> = self.data.get((
    		(z * (self.size_x * self.size_y)) +
    		(y * (self.size_x))
    		+ x) as usize);
    	if result.is_none() {
    		return None;
    	}
    	else {
    		return Some(result.unwrap().clone());
    	}
    }

    fn set(&mut self, x: u16, y: u16, z: u16, value: T) {
    	if (x >= self.size_x) ||
    		(y >= self.size_y) ||
    		(z >= self.size_z)
    	{
    		return;
    	}
    	//u16acked array access
    	(*self.data.get_mut((
    		(z * (self.size_x * self.size_y)) +
    		(y * (self.size_x))
    		+ x) as usize).unwrap()) = value;
    }

    //Intializes a voxel storage, with each cell set to default value.
    //fn init_new(&mut self, size_x: u16, size_y: u16, size_z: u16, default: T);
    //Uninitialized version of the above. Still allocates, probably.
    //fn init_new_uninitialized(&mut self, size_x: u16, size_y: u16, size_z: P);

    //Gets how many bytes this structure takes up in memory.
    /*fn get_footprint(&self) -> usize {
    	return ((size_of::<T>() as u16) * (self.size_x * self.size_y * self.size_z)) as usize;
    }*/
}
impl <T: Clone> VoxelStorageIOAble<T> for VoxelArray<T> { 
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load<R: Read + Sized>(&mut self, reader: &mut R) { 
		let array: &mut [u8] = unsafe { mem::transmute(&*self.data) };
    	reader.read(array);
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save<W: Write + Sized>(&self, writer: &mut W) -> Result<usize, std::io::Error> {
		let array: &[u8] = unsafe { mem::transmute(&*self.data) };
    	writer.write(array)
    }
}

impl <T: Clone> VoxelStorageBounded<T> for VoxelArray<T> { 
    fn get_bounds(&self) -> VoxelRange<u16> { 
        return self.bounds;
    }
}

#[test]
fn test_array_raccess() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u16);
    }

    let mut test_va : Box<VoxelArray<u16>> = VoxelArray::load_new(16, 16, 16, test_chunk);

    assert!(test_va.get(14,14,14).unwrap() == 3822);
    test_va.set(14,14,14,9);
    assert!(test_va.get(14,14,14).unwrap() == 9);
}


#[test]
fn test_array_iterative() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for _i in 0 .. OURSIZE {
    	test_chunk.push(16);
    }

    let mut test_va : Box<VoxelArray<u16>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    let xsz : u16 = test_va.get_bounds().upper.x;
    let ysz : u16 = test_va.get_bounds().upper.y;
    let zsz : u16 = test_va.get_bounds().upper.z;
	for x in 0 .. xsz as u16 {
		for y in 0 .. ysz as u16 {
			for z in 0 .. zsz as u16 {
				assert!(test_va.get(x,y,z).unwrap() == 16);
				test_va.set(x,y,z, (x as u16 % 10));
			}
		}
	}
	assert!(test_va.get(10,0,0).unwrap() == 0);
	assert!(test_va.get(11,0,0).unwrap() == 1);
    //assert_eq!(test_va.get_data_size(), (OURSIZE * 2));
}
