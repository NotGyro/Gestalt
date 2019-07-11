extern crate std;
extern crate num;

use std::marker::Copy;

use voxel::voxelstorage::*;
use util::numbers::USizeAble;
use voxel::voxelmath::*;
use std::io::prelude::*;
use std::mem;

use num::Integer;
use num::traits::identities::One;
use num::traits::identities::Zero;

/// A 3D packed array of voxels - it's a single flat buffer in memory,
/// which is indexed by voxel positions with some math done on them. 
/// Should have a fixed, constant size after creation.
#[derive(Clone, Debug)]
pub struct VoxelArray<T: Clone, P: Copy + Integer + One + Zero + USizeAble> {
    size_x: P, size_y: P, size_z: P,
    data: Vec<T>,
    bounds : VoxelRange<P>,
}

impl <T:Clone, P: Copy + Integer + One + Zero + USizeAble> VoxelArray<T, P> {
    pub fn load_new(szx: P, szy: P, szz: P, dat: Vec<T>) -> VoxelArray<T, P> {
	let bnd = VoxelRange::<P> { lower : VoxelPos::<P>{x : P::zero(), y : P::zero(), z : P::zero()},
                                    upper : VoxelPos{x : szx, y : szy, z : szy}};
        return VoxelArray{size_x: szx, size_y: szy, size_z: szz, 
                                    data: dat, bounds : bnd};
    }
}

impl <T: Clone, P: Copy + Integer + One + Zero + USizeAble> VoxelStorage<T, P> for VoxelArray<T, P> {
    fn get(&self, coord: VoxelPos<P>) -> Option<T> {
    	//Bounds-check.
    	if (coord.x >= self.size_x) ||
    		(coord.y >= self.size_y) ||
    		(coord.z >= self.size_z)
    	{
    		return None;
    	}
    	//Packed array access
    	let result : Option<&T> = self.data.get((
    		(coord.z * (self.size_x * self.size_y)) +
    		(coord.y * (self.size_x))
    		+ coord.x).as_usize());
    	if result.is_none() {
    		return None;
    	}
    	else {
    		return Some(result.unwrap().clone());
    	}
    }

    fn set(&mut self, coord: VoxelPos<P>, value: T) {
    	if (coord.x >= self.size_x) ||
    		(coord.y >= self.size_y) ||
    		(coord.z >= self.size_z)
    	{
    		return;
    	}
    	//Packed array access
    	(*self.data.get_mut((
    		(coord.z * (self.size_x * self.size_y)) +
    		(coord.y * (self.size_x))
    		+ coord.x).as_usize()).unwrap()) = value;
    }

    //Intializes a voxel storage, with each cell set to default value.
    //fn init_new(&mut self, size_x: P, size_y: u16, size_z: u16, default: T);
    //Uninitialized version of the above. Still allocates, probably.
    //fn init_new_uninitialized(&mut self, size_x: P, size_y: u16, size_z: P);

    //Gets how many bytes this structure takes up in memory.
    /*fn get_footprint(&self) -> usize {
    	return ((size_of::<T>() as u16) * (self.size_x * self.size_y * self.size_z)) as usize;
    }*/
}
impl <T: Clone, P> VoxelStorageIOAble<T, P> for VoxelArray<T, P> where P : Copy + Integer + One + Zero + USizeAble { 
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

impl <T: Clone, P> VoxelStorageBounded<T, P> for VoxelArray<T, P> where P : Copy + Integer + One + Zero + USizeAble { 
    fn get_bounds(&self) -> VoxelRange<P> { 
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

    let mut test_va : VoxelArray<u16,u16> = VoxelArray::load_new(16, 16, 16, test_chunk);
    
    let testpos = VoxelPos{x: 14, y: 14, z: 14};
    assert!(test_va.get(testpos).unwrap() == 3822);
    test_va.set(testpos,9);
    assert!(test_va.get(testpos).unwrap() == 9);
}


#[test]
fn test_array_iterative() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for _i in 0 .. OURSIZE {
    	test_chunk.push(16);
    }

    let mut test_va : VoxelArray<u16, u16> = VoxelArray::load_new(16, 16, 16, test_chunk);
    for pos in test_va.get_bounds() {
    	assert!(test_va.get(pos).unwrap() == 16);
    	test_va.set(pos, (pos.x as u16 % 10));
    }
    assert!(test_va.get(VoxelPos{x: 10, y: 0, z: 0}).unwrap() == 0);
    assert!(test_va.get(VoxelPos{x: 11, y: 0, z: 0}).unwrap() == 1);
    //assert_eq!(test_va.get_data_size(), (OURSIZE * 2));
}
