extern crate std;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};
use std::string::String;
use std::vec::Vec;
use util::coord3::Coord3;
use util::axis::Axis;

/* A basic trait for any 3d grid data structure.
Type arguments is type of element.

For this trait, a single level of detail is assumed.

For voxel data structures with a level of detail, we will
assume that the level of detail is a signed integer, and
calling these methods / treating them as "flat" voxel
structures implies acting on a level of detail of 0. 
*/
pub trait VoxelStorage<T: Copy, P = u32> where P : Eq + Ord + Add + Sub + Mul + Div {
	//Self is mutable here for caching reasons.
    fn get(&mut self, x: P, y: P, z: P) -> Option<T>;
	//Self is mutable here for caching reasons.
    fn getv(&mut self, coord: Coord3<P>) -> Option<T> {
        self.get(coord.x, coord.y, coord.z)
    }
    
    fn set(&mut self, x: P, y: P, z: P, value: T);
    fn setv(&mut self, coord: Coord3<P>, value: T) {
        self.set(coord.x, coord.y, coord.z, value);
    }

    //Intializes a voxel storage, with each cell set to default value.
    //fn init_new(&mut self, size_x: P, size_y: P, size_z: P, default: T);
    //Uninitialized version of the above. Still allocates, probably.
    //fn init_new_uninitialized(&mut self, size_x: P, size_y: P, size_z: P);
    
    //Gets how many bytes this structure takes up in memory.
    fn get_footprint(&self) -> usize;
    
    //A value of None means our VoxelStorage is pseudo-infinite in this direction
    fn get_x_sz(&self) -> Option<usize>;
    //A value of None means our VoxelStorage is pseudo-infinite in this direction
    fn get_y_sz(&self)  -> Option<usize>;
    //A value of None means our VoxelStorage is pseudo-infinite in this direction
    fn get_z_sz(&self)  -> Option<usize>;
    
    /*Takes a raw header (as read from file) and a number of pages. 
    Takes ownership of pages.*/
    //fn load(&mut self, header: Vec<u8>, mut pages: Vec<Vec<u8>>);
    /*Construct VS from saved data. Takes a raw header (as read from file) 
    and a number of pages. Takes ownership of pages. */
    //fn load_new(header: Vec<u8>, mut pages: Vec<Vec<u8>>);
    //fn start_save();
    //TODO: Method to get iterator over all voxels
    
    fn get_adjacent(&self, direction : Axis) -> Option<&VoxelStorage<T, P>>;    
    fn get_adjacent_mut(&mut self, direction : Axis) -> Option<&mut VoxelStorage<T, P>>;
}

/* TODO: Utility functions to copy a range of one VoxelStorage to a range of another,
or construct a new VoxelStorage from another, etc.
*/
/*
//The loading and saving architecture of Voxel Storage objects are not 
//uniform across all VSes. This solves a lot of architectural problems.
pub trait ContiguousVS<T: Clone, P> : VoxelStorage<T, P>
where P : Eq + Ord + Add<Output=P> + Sub<Output=P> + Mul<Output=P> + Div<Output=P> {
	//A constructor. Takes ownership.
	fn load_new(szx: usize, szy: usize, szz: usize, mut data: Vec<T>) -> VoxelStorage<T, P>;
	//Takes ownership
	fn load(&mut self, mut data: Vec<T>);
	//Returns a borrow of our data.
	fn start_save(&mut self) -> &[T];
	//Just signals to our voxel storage structure that we're safe to write to it again.
	fn finish_save(&mut self);
}
pub trait NonContiguousVS<T: Clone, P> : VoxelStorage<T, P>
where P : Eq + Ord + Add<Output=P> + Sub<Output=P> + Mul<Output=P> + Div<Output=P> {
	*/
	/* In Gestalt files, page size is uniform. It's not CONSTANT, exactly,
	because it can vary between files, but each file may only have one page size.
	*/
	/*
	fn load(&mut self, page_sz: usize, mut pages: Vec<Vec<T>>);
	
	fn start_save(&mut self, page_sz: usize) -> Vec<&[T]>;
	//Just signals to our voxel storage structure that we're safe to write to it again.
	fn finish_save();
}
*/