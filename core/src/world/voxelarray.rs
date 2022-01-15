extern crate std;
extern crate num;

use super::voxelstorage::*;
use crate::common::voxelmath::*;

/// A 3D packed array of voxels - it's a single flat buffer in memory,
/// which is indexed by voxel positions with some math done on them. 
/// Should have a fixed, constant size after creation.
#[derive(Clone, Debug)]
pub struct VoxelArray<T: Voxel> {
    size_x: u16, size_y: u16, size_z: u16,
    data: Vec<T>,
    bounds : VoxelRange<u16>,
}

impl <T:Voxel> VoxelArray<T> {

	pub fn load_new(szx: u16, szy: u16, szz: u16, dat: Vec<T>) -> Box<VoxelArray<T>> {
		let bnd = VoxelRange::<u16> { lower : VoxelPos::<u16>{x : 0, y : 0, z : 0},
              upper : VoxelPos{x : szx, y : szy, z : szy}};
        return Box::new(VoxelArray{size_x: szx, size_y: szy, size_z: szz, 
            data: dat, bounds : bnd});
	}
}

impl <T: Voxel> VoxelStorage<T, u16> for VoxelArray<T> {
    fn get(&self, coord: VoxelPos<u16>) -> Result<T, VoxelError> {
    	//Bounds-check.
    	if (coord.x >= self.size_x) ||
    		(coord.y >= self.size_y) ||
    		(coord.z >= self.size_z)
    	{
    		return Err(VoxelError::OutOfBounds(
                VoxelPos { 
                    x: coord.x as i32,
                    y: coord.y as i32,
                    z: coord.z as i32,
                }
            ));
    	}
    	//Packed array access
    	let result : Option<&T> = self.data.get((
    		(coord.z * (self.size_x * self.size_y)) +
    		(coord.y * (self.size_x))
    		+ coord.x) as usize);
    	return Ok(result.unwrap().clone())
    }

    fn set(&mut self, coord: VoxelPos<u16>, value: T) -> Result<(), super::voxelstorage::VoxelError> {
    	if (coord.x >= self.size_x) ||
    		(coord.y >= self.size_y) ||
    		(coord.z >= self.size_z)
    	{
    		return Err(VoxelError::OutOfBounds(
                VoxelPos { 
                    x: coord.x as i32,
                    y: coord.y as i32,
                    z: coord.z as i32,
                }));
    	}
    	//u16acked array access
    	(*self.data.get_mut((
    		(coord.z * (self.size_x * self.size_y)) +
    		(coord.y * (self.size_x))
    		+ coord.x) as usize).unwrap()) = value;

        Ok(())
    }
}

impl <T: Voxel> VoxelStorageBounded<T, u16> for VoxelArray<T> { 
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

    assert!(test_va.get(vpos!(14,14,14)).unwrap() == 3822);
    test_va.set(vpos!(14,14,14),9).unwrap();
    assert!(test_va.get(vpos!(14,14,14)).unwrap() == 9);
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
				assert!(test_va.get(vpos!(x,y,z)).unwrap() == 16);
				test_va.set(vpos!(x,y,z), x as u16 % 10).unwrap();
			}
		}
	}
	assert!(test_va.get(vpos!(10,0,0)).unwrap() == 0);
	assert!(test_va.get(vpos!(11,0,0)).unwrap() == 1);
}