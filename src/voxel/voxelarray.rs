/* A basic trait for any 3d grid of data.
For this trait, a single level of detail is assumed.

For voxel data structures with a level of detail, we will
assume that the level of detail is a signed integer, and
calling these methods / treating them as "flat" voxel
structures implies acting on a level of detail of 0. */

extern crate std;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};
use std::mem::size_of;
use voxel::voxelstorage::VoxelStorage;
//use voxel::voxelstorage::ContiguousVS;

// Type arguments are type of element, type of position / index.
pub struct VoxelArray<T: Copy> {
    size_x: u32, size_y: u32, size_z: u32,
    data: Vec<T>,
}

impl <T:Copy> VoxelArray<T> {
	
	pub fn load_new(szx: usize, szy: usize, szz: usize, mut dat: Vec<T>) -> Box<VoxelArray<T>> {
		return Box::new(VoxelArray{size_x: szx as u32, size_y: szy as u32, size_z: szz as u32, data: dat});
	}
}

impl <T: Copy> VoxelStorage<T, u32> for VoxelArray<T> {
    fn get(&mut self, x: u32, y: u32, z: u32) -> Option<T> {
    	//Bounds-check.
    	if((x >= self.size_x) || 
    		(y >= self.size_y) || 
    		(z >= self.size_z))
    	{
    		return None;
    	}
    	//Packed array access
    	let result : Option<&T> = self.data.get((
    		(z * (self.size_x * self.size_y)) +
    		(y * (self.size_x))
    		+ x) as usize);
    	if(result.is_none()) {
    		return None;
    	}
    	else {
    		return Some(*result.unwrap());
    	}
    }
    
    fn set(&mut self, x: u32, y: u32, z: u32, value: T) {
    	if((x >= self.size_x) || 
    		(y >= self.size_y) || 
    		(z >= self.size_z))
    	{
    		return;
    	}
    	//Packed array access
    	(*self.data.get_mut((
    		(z * (self.size_x * self.size_y)) +
    		(y * (self.size_x))
    		+ x) as usize).unwrap()) = value;
    }

    //Intializes a voxel storage, with each cell set to default value.
    //fn init_new(&mut self, size_x: P, size_y: P, size_z: P, default: T);
    //Uninitialized version of the above. Still allocates, probably.
    //fn init_new_uninitialized(&mut self, size_x: P, size_y: P, size_z: P);
    
    //Gets how many bytes this structure takes up in memory.
    fn get_footprint(&self) -> usize {
    	return ((size_of::<T>() as u32) * (self.size_x * self.size_y * self.size_z)) as usize;
    }
    
    fn get_x_sz(&self) -> Option<usize> {
    	Some(self.size_x as usize)
    }
    fn get_y_sz(&self)  -> Option<usize> {
    	Some(self.size_y as usize)
    }
    fn get_z_sz(&self)  -> Option<usize> {
    	Some(self.size_z as usize)
    }
}
/*
impl <T: Copy> ContiguousVS<T, u32> for VoxelArray<T> {
	
	//A constructor. Takes ownership.
	pub fn load_new(szx: usize, szy: usize, szz: usize, mut dat: Vec<T>) -> VoxelStorage<T, u32> {
		let newVA : VoxelArray<T> = VoxelArray{size_x: szx as u32, size_y: szy as u32, size_z: szz as u32, data: dat};
		return (&newVA) as VoxelStorage<T, u32>;
	}
	//Takes ownership
	fn load(&mut self, mut data: Vec<T>) {
		self.data = data;
	}
	//Returns a borrow of our data.
	fn start_save(&mut self) -> &[T] {
		return self.data.as_slice();
	}
	//Just signals to our voxel storage structure that we're safe to write to it again.
	fn finish_save(&mut self) {}
}*/