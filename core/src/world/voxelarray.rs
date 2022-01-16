extern crate std;
extern crate num;

use super::voxelstorage::*;
use crate::common::voxelmath::*;

/// X, Y, and Z coords to standard buffer index.
#[inline(always)] 
pub const fn chunk_xyz_to_i(x : usize, y : usize, z : usize, chunk_size: usize) -> usize {
    (z * (chunk_size*chunk_size) ) + (y * chunk_size) + x
}

/// Chunk buffer index to XYZ coords.
#[inline(always)] 
pub const fn chunk_i_to_xyz(i : usize, chunk_size: usize) -> (usize, usize, usize) {
    let x = i % chunk_size;
    let z = (i-x)/(chunk_size*chunk_size); //The remainder on this (the y value) just gets thrown away, which is good here.
    let y = (i - (z * (chunk_size*chunk_size) ))/chunk_size;
    (x, y, z)
}

/// A 3D packed array of voxels - it's a single flat buffer in memory,
/// which is indexed by voxel positions with some math done on them. 
/// Should have a fixed, constant size after creation.
#[derive(Clone, Debug)]
pub struct VoxelArray<T: Voxel> {
    size: u16,
    data: Vec<T>,
    bounds : VoxelRange<u16>,
}

impl <T:Voxel> VoxelArray<T> {
	pub fn load_new(size: u16, dat: Vec<T>) -> VoxelArray<T> {
		let bnd = VoxelRange::<u16> { lower : VoxelPos::<u16>{x : 0, y : 0, z : 0},
              upper : VoxelPos{x : size, y : size, z : size}};
        
        VoxelArray{
            size, 
            data: dat, 
            bounds : bnd
        }
	}
}

impl <T: Voxel> VoxelStorage<T, u16> for VoxelArray<T> {
    fn get(&self, coord: VoxelPos<u16>) -> Result<T, VoxelError> {
    	//Bounds-check.
    	if (coord.x >= self.size) ||
    		(coord.y >= self.size) ||
    		(coord.z >= self.size)
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
    	let result : Option<&T> = self.data.get(chunk_xyz_to_i(coord.x as usize, coord.y as usize, coord.z as usize, self.size as usize) );
    	return Ok(result.unwrap().clone())
    }

    fn set(&mut self, coord: VoxelPos<u16>, value: T) -> Result<(), super::voxelstorage::VoxelError> {
    	if (coord.x >= self.size) ||
    		(coord.y >= self.size) ||
    		(coord.z >= self.size)
    	{
    		return Err(VoxelError::OutOfBounds(
                VoxelPos { 
                    x: coord.x as i32,
                    y: coord.y as i32,
                    z: coord.z as i32,
                }));
    	}
    	//u16acked array access
    	*self.data.get_mut(chunk_xyz_to_i(coord.x as usize, coord.y as usize, coord.z as usize, self.size as usize ) ).unwrap() = value;

        Ok(())
    }
}

impl <T: Voxel> VoxelStorageBounded<T, u16> for VoxelArray<T> { 
    fn get_bounds(&self) -> VoxelRange<u16> { 
        return self.bounds;
    }
}

/// A 3D packed array of voxels. This much like VoxelArray, 
/// but statically-sized - this may lead to better performance
/// due to better compiler optimizations.
#[derive(Clone, Debug)]
pub struct VoxelArrayStatic<T: Voxel + Copy, const SIZE: usize> where [u8; SIZE*SIZE*SIZE]: Sized {
    data: [T; SIZE*SIZE*SIZE],
}

impl <T: Voxel + Copy, const SIZE: usize> VoxelArrayStatic<T, SIZE> where [u8; SIZE*SIZE*SIZE]: Sized {
    pub fn new(default_value: T) -> Self {
        VoxelArrayStatic{ 
            data: [default_value; SIZE*SIZE*SIZE],
        }
    }
}

impl <T: Voxel + Copy, const SIZE: usize> VoxelStorage<T, u16> for VoxelArrayStatic<T, SIZE> where [u8; SIZE*SIZE*SIZE]: Sized {
    fn get(&self, coord: VoxelPos<u16>) -> Result<T, VoxelError> {
    	//Bounds-check.
    	if (coord.x >= SIZE as u16) ||
    		(coord.y >= SIZE as u16) ||
    		(coord.z >= SIZE as u16)
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
    	let result : Option<&T> = self.data.get(chunk_xyz_to_i(coord.x as usize, coord.y as usize, coord.z as usize, SIZE) );
    	return Ok(result.unwrap().clone())
    }

    fn set(&mut self, coord: VoxelPos<u16>, value: T) -> Result<(), super::voxelstorage::VoxelError> {
    	if (coord.x >= SIZE as u16) ||
    		(coord.y >= SIZE as u16) ||
    		(coord.z >= SIZE as u16)
    	{
    		return Err(VoxelError::OutOfBounds(
                VoxelPos { 
                    x: coord.x as i32,
                    y: coord.y as i32,
                    z: coord.z as i32,
                }));
    	}
    	//u16acked array access
    	(*self.data.get_mut(chunk_xyz_to_i(coord.x as usize, coord.y as usize, coord.z as usize, SIZE) ).unwrap()) = value;

        Ok(())
    }
}

impl <T: Voxel + Copy, const SIZE: usize> VoxelStorageBounded<T, u16> for VoxelArrayStatic<T, SIZE> where [u8; SIZE*SIZE*SIZE]: Sized { 
    fn get_bounds(&self) -> VoxelRange<u16> { 
        return VoxelRange {
            lower: VoxelPos {
                x: 0,
                y: 0,
                z: 0,
            },
            upper: VoxelPos {
                x: SIZE as u16,
                y: SIZE as u16,
                z: SIZE as u16,
            }
        };
    }
}

#[test]
fn test_array_raccess() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u16);
    }

    let mut test_va : VoxelArray<u16> = VoxelArray::load_new(16, test_chunk);

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

    let mut test_va : VoxelArray<u16> = VoxelArray::load_new(16, test_chunk);
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