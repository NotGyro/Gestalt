extern crate num;
extern crate std;

use super::voxelstorage::*;
use crate::common::voxelmath::*;

#[allow(unused_variables)]
#[inline(always)]
pub const fn chunk_x_to_i_component(x: usize, chunk_size: usize) -> usize {
    x
}
#[inline(always)]
pub const fn chunk_y_to_i_component(y: usize, chunk_size: usize) -> usize {
    y * chunk_size
}
#[inline(always)]
pub const fn chunk_z_to_i_component(z: usize, chunk_size: usize) -> usize {
    z * (chunk_size * chunk_size)
}

#[inline(always)]
pub const fn chunk_xyz_to_i(x: usize, y: usize, z: usize, chunk_size: usize) -> usize {
    chunk_z_to_i_component(z, chunk_size)
        + chunk_y_to_i_component(y, chunk_size)
        + chunk_x_to_i_component(x, chunk_size)
}

#[inline(always)]
pub const fn chunk_i_to_xyz(i: usize, chunk_size: usize) -> (usize, usize, usize) {
    let chunk_squared = chunk_size * chunk_size;
    let z = i / (chunk_squared);
    let y = (i - z * chunk_squared) / chunk_size;
    let x = i - ((z * chunk_squared) + (y * chunk_size));
    (x, y, z)
}

#[inline(always)]
pub const fn get_pos_x_offset(i: usize, chunk_size: usize) -> Option<usize> {
    let chunk_volume = chunk_size * chunk_size * chunk_size;
    if (i + chunk_x_to_i_component(1, chunk_size) < chunk_volume)
        && (chunk_i_to_xyz(i, chunk_size).0 + 1 < chunk_size)
    {
        Some(i + chunk_x_to_i_component(1, chunk_size))
    } else {
        None
    }
}
#[inline(always)]
pub const fn get_neg_x_offset(i: usize, chunk_size: usize) -> Option<usize> {
    if chunk_i_to_xyz(i, chunk_size).0.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_x_to_i_component(1, chunk_size))
}
#[inline(always)]
pub const fn get_pos_y_offset(i: usize, chunk_size: usize) -> Option<usize> {
    let chunk_volume = chunk_size * chunk_size * chunk_size;
    if (i + chunk_y_to_i_component(1, chunk_size) < chunk_volume)
        && (chunk_i_to_xyz(i, chunk_size).1 + 1 < chunk_size)
    {
        Some(i + chunk_y_to_i_component(1, chunk_size))
    } else {
        None
    }
}
#[inline(always)]
pub const fn get_neg_y_offset(i: usize, chunk_size: usize) -> Option<usize> {
    if chunk_i_to_xyz(i, chunk_size).1.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_y_to_i_component(1, chunk_size))
}
#[inline(always)]
pub const fn get_pos_z_offset(i: usize, chunk_size: usize) -> Option<usize> {
    let chunk_volume = chunk_size * chunk_size * chunk_size;
    if (i + chunk_z_to_i_component(1, chunk_size) < chunk_volume)
        && (chunk_i_to_xyz(i, chunk_size).2 + 1 < chunk_size)
    {
        Some(i + chunk_z_to_i_component(1, chunk_size))
    } else {
        None
    }
}
#[inline(always)]
pub const fn get_neg_z_offset(i: usize, chunk_size: usize) -> Option<usize> {
    if chunk_i_to_xyz(i, chunk_size).2.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_z_to_i_component(1, chunk_size))
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum VoxelArrayError {
    #[error("Attempted to access a voxel at position {0}, which is out of bounds on this chunk.")]
    OutOfBounds(VoxelPos<u16>),
}

impl VoxelError for VoxelArrayError {
    fn kind(&self) -> VoxelErrorCategory {
        match self {
            VoxelArrayError::OutOfBounds(_) => VoxelErrorCategory::OutOfBounds,
        }
    }
}

/// A 3D packed array of voxels - it's a single flat buffer in memory,
/// which is indexed by voxel positions with some math done on them.
/// Should have a fixed, constant size after creation.
#[derive(Clone, Debug)]
pub struct VoxelArray<T: Voxel> {
    size: u16,
    data: Vec<T>,
    bounds: VoxelRange<u16>,
}

#[allow(dead_code)]
impl<T: Voxel> VoxelArray<T> {
    pub fn load_new(size: u16, dat: Vec<T>) -> VoxelArray<T> {
        let bnd = VoxelRange::<u16> {
            lower: VoxelPos::<u16> { x: 0, y: 0, z: 0 },
            upper: VoxelPos {
                x: size,
                y: size,
                z: size,
            },
        };

        VoxelArray {
            size,
            data: dat,
            bounds: bnd,
        }
    }
    /// Does not bounds check
    pub(crate) fn get_raw(&self, coord: VoxelPos<u16>) -> &T {
        &self.data[chunk_xyz_to_i(
            coord.x as usize,
            coord.y as usize,
            coord.z as usize,
            self.size as usize,
        )]
    }
    /// Does not bounds check
    pub(crate) fn set_raw(&mut self, coord: VoxelPos<u16>, value: T) {
        (*self
            .data
            .get_mut(chunk_xyz_to_i(
                coord.x as usize,
                coord.y as usize,
                coord.z as usize,
                self.size as usize,
            ))
            .unwrap()) = value;
    }

    pub(crate) fn get_raw_i(&self, i: usize) -> &T {
        &self.data[i]
    }
    pub(crate) fn set_raw_i(&mut self, i: usize, value: T) {
        (*self.data.get_mut(i).unwrap()) = value;
    }
}

impl<T: Voxel> VoxelStorage<T, u16> for VoxelArray<T> {
    type Error = VoxelArrayError;
    fn get(&self, coord: VoxelPos<u16>) -> Result<&T, Self::Error> {
        //Bounds-check.
        if (coord.x >= self.size) || (coord.y >= self.size) || (coord.z >= self.size) {
            return Err(Self::Error::OutOfBounds(VoxelPos {
                x: coord.x,
                y: coord.y,
                z: coord.z,
            }));
        }
        //Packed array access
        let result: Option<&T> = self.data.get(chunk_xyz_to_i(
            coord.x as usize,
            coord.y as usize,
            coord.z as usize,
            self.size as usize,
        ));
        return Ok(result.unwrap());
    }

    fn set(
        &mut self,
        coord: VoxelPos<u16>,
        value: T,
    ) -> Result<(), Self::Error> {
        if (coord.x >= self.size) || (coord.y >= self.size) || (coord.z >= self.size) {
            return Err(Self::Error::OutOfBounds(VoxelPos {
                x: coord.x,
                y: coord.y,
                z: coord.z,
            }));
        }
        //packed array access
        *self
            .data
            .get_mut(chunk_xyz_to_i(
                coord.x as usize,
                coord.y as usize,
                coord.z as usize,
                self.size as usize,
            ))
            .unwrap() = value;

        Ok(())
    }
}

impl<T: Voxel> VoxelStorageBounded<T, u16> for VoxelArray<T> {
    fn get_bounds(&self) -> VoxelRange<u16> {
        self.bounds
    }
}

/// A 3D packed array of voxels. This much like VoxelArray,
/// but statically-sized - this may lead to better performance
/// due to better compiler optimizations.
#[derive(Clone, Debug)]
pub struct VoxelArrayStatic<T: Voxel + Copy, const SIZE: usize>
where
    [u8; SIZE * SIZE * SIZE]: Sized,
{
    data: [T; SIZE * SIZE * SIZE],
}

impl<T: Voxel + Copy, const SIZE: usize> VoxelArrayStatic<T, SIZE>
where
    [u8; SIZE * SIZE * SIZE]: Sized,
{
    pub fn new(default_value: T) -> Self {
        VoxelArrayStatic {
            data: [default_value; SIZE * SIZE * SIZE],
        }
    }
    /// Does not bounds check
    pub(crate) fn get_raw(&self, coord: VoxelPos<u16>) -> &T {
        &self.data[chunk_xyz_to_i(coord.x as usize, coord.y as usize, coord.z as usize, SIZE)]
    }
    /// Does not bounds check
    pub(crate) fn set_raw(&mut self, coord: VoxelPos<u16>, value: T) {
        (*self
            .data
            .get_mut(chunk_xyz_to_i(
                coord.x as usize,
                coord.y as usize,
                coord.z as usize,
                SIZE,
            ))
            .unwrap()) = value;
    }

    pub(crate) fn get_raw_i(&self, i: usize) -> &T {
        &self.data[i]
    }
    pub(crate) fn set_raw_i(&mut self, i: usize, value: T) {
        (*self.data.get_mut(i).unwrap()) = value;
    }
}

impl<T: Voxel + Copy, const SIZE: usize> VoxelStorage<T, u16> for VoxelArrayStatic<T, SIZE>
where
    [u8; SIZE * SIZE * SIZE]: Sized,
{
    type Error = VoxelArrayError;
    fn get(&self, coord: VoxelPos<u16>) -> Result<&T, Self::Error> {
        //Bounds-check.
        if (coord.x >= SIZE as u16) || (coord.y >= SIZE as u16) || (coord.z >= SIZE as u16) {
            return Err(Self::Error::OutOfBounds(VoxelPos {
                x: coord.x,
                y: coord.y,
                z: coord.z,
            }));
        }
        //Packed array access
        let result: Option<&T> = self.data.get(chunk_xyz_to_i(
            coord.x as usize,
            coord.y as usize,
            coord.z as usize,
            SIZE,
        ));
        return Ok(result.unwrap());
    }

    fn set(
        &mut self,
        coord: VoxelPos<u16>,
        value: T,
    ) -> Result<(), Self::Error> {
        if (coord.x >= SIZE as u16) || (coord.y >= SIZE as u16) || (coord.z >= SIZE as u16) {
            return Err(Self::Error::OutOfBounds(VoxelPos {
                x: coord.x,
                y: coord.y,
                z: coord.z,
            }));
        }
        //packed array access
        (*self
            .data
            .get_mut(chunk_xyz_to_i(
                coord.x as usize,
                coord.y as usize,
                coord.z as usize,
                SIZE,
            ))
            .unwrap()) = value;

        Ok(())
    }
}

impl<T: Voxel + Copy, const SIZE: usize> VoxelStorageBounded<T, u16> for VoxelArrayStatic<T, SIZE>
where
    [u8; SIZE * SIZE * SIZE]: Sized,
{
    fn get_bounds(&self) -> VoxelRange<u16> {
        VoxelRange {
            lower: VoxelPos { x: 0, y: 0, z: 0 },
            upper: VoxelPos {
                x: SIZE as u16,
                y: SIZE as u16,
                z: SIZE as u16,
            },
        }
    }
}

#[test]
fn test_array_raccess() {
    const OURSIZE: usize = 16 * 16 * 16;
    let mut test_chunk: Vec<u16> = Vec::with_capacity(OURSIZE);
    for i in 0..OURSIZE {
        test_chunk.push(i as u16);
    }

    let mut test_va: VoxelArray<u16> = VoxelArray::load_new(16, test_chunk);

    assert!(*test_va.get(vpos!(14, 14, 14)).unwrap() == 3822);
    test_va.set(vpos!(14, 14, 14), 9).unwrap();
    assert!(*test_va.get(vpos!(14, 14, 14)).unwrap() == 9);
}

#[test]
fn test_array_iterative() {
    const OURSIZE: usize = 16 * 16 * 16;
    let mut test_chunk: Vec<u16> = Vec::with_capacity(OURSIZE);
    for _i in 0..OURSIZE {
        test_chunk.push(16);
    }

    let mut test_va: VoxelArray<u16> = VoxelArray::load_new(16, test_chunk);
    let xsz: u16 = test_va.get_bounds().upper.x;
    let ysz: u16 = test_va.get_bounds().upper.y;
    let zsz: u16 = test_va.get_bounds().upper.z;
    for x in 0..xsz as u16 {
        for y in 0..ysz as u16 {
            for z in 0..zsz as u16 {
                assert!(*test_va.get(vpos!(x, y, z)).unwrap() == 16);
                test_va.set(vpos!(x, y, z), x as u16 % 10).unwrap();
            }
        }
    }
    assert!(*test_va.get(vpos!(10, 0, 0)).unwrap() == 0);
    assert!(*test_va.get(vpos!(11, 0, 0)).unwrap() == 1);
}

#[test]
fn chunk_index_reverse() {
    use rand::Rng;

    let chunk_sz = 16 as usize;
    let mut rng = rand::thread_rng();
    for _ in 0..4096 {
        let x = rng.gen_range(0..16);
        let y = rng.gen_range(0..16);
        let z = rng.gen_range(0..16);

        let i_value = chunk_xyz_to_i(x, y, z, chunk_sz);
        let (x1, y1, z1) = chunk_i_to_xyz(i_value, chunk_sz);

        assert_eq!(x, x1);
        assert_eq!(y, y1);
        assert_eq!(z, z1);
    }
}

#[test]
fn chunk_index_bounds() {
    let chunk_sz = 16 as usize;
    let chunk_volume = chunk_sz * chunk_sz * chunk_sz;
    for x in 0..chunk_sz {
        for y in 0..chunk_sz {
            for z in 0..chunk_sz {
                assert!(chunk_xyz_to_i(x, y, z, chunk_sz) < chunk_volume);
            }
        }
    }
}
