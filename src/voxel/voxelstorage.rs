/* A basic trait for any 3d grid of data.
For this trait, a single level of detail is assumed.

For voxel data structures with a level of detail, we will
assume that the level of detail is a signed integer, and
calling these methods / treating them as "flat" voxel
structures implies acting on a level of detail of 0. */

extern crate num;
using num::traits::PrimInt;


// Type arguments are type of element, type of position / index.
pub trait VoxelStorage<T, P: PrimInt> {
    pub fn get(&self, P x, P y, P z) -> T;
    pub fn get(&self, Coord3<P> coord) -> T {
        get(coord.x, coord.y, coord.z)
    }
    
    //Throws if we are read-only.
    pub fn set(&self, P x, P y, P z, T value);
    pub fn set(&self, Coord3<P> coord, T value) {
        set(coord.x, coord.y, coord.z, value);
    }

    //Intializes a voxel storage, with each cell set to default value.
    pub fn init(Coord3<P> size, T default);
    //Uninitialized version of the above. Still allocates, probably.
    pub fn init(Coord3<P> size);
}

