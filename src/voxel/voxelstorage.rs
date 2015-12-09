extern crate std;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};
use std::string::String;
use std::vec::Vec;
use util::vec3::Vec3;
use util::axis::Axis;
use std::io;
use std::io::prelude::*;

/* A basic trait for any 3d grid data structure.
Type arguments is type of element.

For this trait, a single level of detail is assumed.

For voxel data structures with a level of detail, we will
assume that the level of detail is a signed integer, and
calling these methods / treating them as "flat" voxel
structures implies acting on a level of detail of 0.
*/
pub trait VoxelStorage<T: Copy, P = u32> where P : Eq + Ord + Add + Sub + Mul + Div {
	//Mutable for caching reasons
    fn get(&self, x: P, y: P, z: P) -> Option<T>;
	//Mutable for caching reasons
    fn getv(&self, coord: Vec3<P>) -> Option<T> {
        self.get(coord.x, coord.y, coord.z)
    }

    fn set(&mut self, x: P, y: P, z: P, value: T);
    fn setv(&mut self, coord: Vec3<P>, value: T) {
        self.set(coord.x, coord.y, coord.z, value);
    }

    //Intializes a voxel storage, with each cell set to default value.
    //fn init_new(&mut self, size_x: P, size_y: P, size_z: P, default: T);
    //Uninitialized version of the above. Still allocates, probably.
    //fn init_new_uninitialized(&mut self, size_x: P, size_y: P, size_z: P);

    //Gets how many bytes this structure takes up in memory.
    //fn get_footprint(&self) -> usize;

    //A value of None means our VoxelStorage is pseudo-infinite in this direction
    fn get_x_sz(&self) -> Option<P>;
    //A value of None means our VoxelStorage is pseudo-infinite in this direction
    fn get_y_sz(&self)  -> Option<P>;
    //A value of None means our VoxelStorage is pseudo-infinite in this direction
    fn get_z_sz(&self)  -> Option<P>;
    
    fn load(&mut self, reader: &mut Read);
    fn save(&mut self, writer: &mut Write);
}