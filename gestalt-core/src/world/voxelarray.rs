use std::{fmt::Debug, marker::PhantomData};

use super::voxelstorage::*;
use crate::common::voxelmath::*;

#[allow(unused_variables)]
#[inline(always)]
pub(crate) const fn chunk_x_to_i_component(x: usize, chunk_size: usize) -> usize {
    x * chunk_size
}
#[allow(unused_variables)]
#[inline(always)]
pub(crate) const fn chunk_y_to_i_component(y: usize, chunk_size: usize) -> usize {
    y
}
#[allow(unused_variables)]
#[inline(always)]
pub(crate) const fn chunk_z_to_i_component(z: usize, chunk_size: usize) -> usize {
    z * (chunk_size * chunk_size)
}

#[inline(always)]
pub(crate) const fn chunk_xyz_to_i(x: usize, y: usize, z: usize, chunk_size: usize) -> usize {
    chunk_z_to_i_component(z, chunk_size)
        + chunk_y_to_i_component(y, chunk_size)
        + chunk_x_to_i_component(x, chunk_size)
}

#[inline(always)]
pub(crate) const fn chunk_i_to_xyz(i: usize, chunk_size: usize) -> (usize, usize, usize) {
    let chunk_squared = chunk_size * chunk_size;
    let z = i / (chunk_squared);
    let x = (i - z * chunk_squared) / chunk_size;
    let y = i - ((z * chunk_squared) + (x * chunk_size));
    (x, y, z)
}

#[inline(always)]
pub(crate) const fn get_pos_x_offset(i: usize, chunk_size: usize) -> Option<usize> {
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
#[allow(clippy::question_mark)]
pub(crate) const fn get_neg_x_offset(i: usize, chunk_size: usize) -> Option<usize> {
    if chunk_i_to_xyz(i, chunk_size).0.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_x_to_i_component(1, chunk_size))
}
#[inline(always)]
pub(crate) const fn get_pos_y_offset(i: usize, chunk_size: usize) -> Option<usize> {
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
#[allow(clippy::question_mark)]
pub(crate) const fn get_neg_y_offset(i: usize, chunk_size: usize) -> Option<usize> {
    if chunk_i_to_xyz(i, chunk_size).1.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_y_to_i_component(1, chunk_size))
}
#[inline(always)]
pub(crate) const fn get_pos_z_offset(i: usize, chunk_size: usize) -> Option<usize> {
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
#[allow(clippy::question_mark)]
pub(crate) const fn get_neg_z_offset(i: usize, chunk_size: usize) -> Option<usize> {
    if chunk_i_to_xyz(i, chunk_size).2.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_z_to_i_component(1, chunk_size))
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum VoxelArrayError<T> where T: VoxelCoord + Debug {
    #[error("Attempted to access a voxel at position {0}, which is out of bounds on this chunk.")]
    OutOfBounds(VoxelPos<T>),
}

impl<T> VoxelError for VoxelArrayError<T> where T: VoxelCoord + Debug {
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
pub struct VoxelArray<T: Voxel, P: VoxelCoord> {
    pub(crate) size: usize,
    pub(crate) data: Vec<T>,
    pub(crate) bounds: VoxelRange<P>,
}

#[allow(dead_code)]
impl<T, P> VoxelArray<T, P> 
        where T: Voxel,
        P: VoxelCoord + USizeAble {
    pub fn load_new(side_length: P, dat: Vec<T>) -> VoxelArray<T, P> {
        let side_length_usize: usize = side_length.as_usize(); 
        assert!(dat.len() <= (side_length_usize * side_length_usize * side_length_usize) );
        let zero = P::from_usize(0usize); 
        let bnd = VoxelRange::<P> {
            lower: VoxelPos::<P> { x: zero, y: zero, z: zero },
            upper: VoxelPos {
                x: side_length,
                y: side_length,
                z: side_length,
            },
        };

        VoxelArray {
            size: side_length_usize,
            data: dat,
            bounds: bnd,
        }
    }
    /// Does not bounds check
    pub(crate) fn get_raw(&self, coord: VoxelPos<P>) -> &T {
        &self.data[chunk_xyz_to_i(
            coord.x.as_usize(),
            coord.y.as_usize(),
            coord.z.as_usize(),
            self.size,
        )]
    }
    /// Does not bounds check
    pub(crate) fn set_raw(&mut self, coord: VoxelPos<P>, value: T) {
        (*self
            .data
            .get_mut(chunk_xyz_to_i(
                coord.x.as_usize(),
                coord.y.as_usize(),
                coord.z.as_usize(),
                self.size,
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

impl<T, P> VoxelStorage<T, P> for VoxelArray<T,P>
        where T: Voxel,
        P: VoxelCoord + USizeAble {
    type Error = VoxelArrayError<P>;
    fn get(&self, coord: VoxelPos<P>) -> Result<&T, Self::Error> {
        let size = P::from_usize(self.size); 
        //Bounds-check.
        if (coord.x >= size) || (coord.y >= size) || (coord.z >= size) {
            return Err(Self::Error::OutOfBounds(VoxelPos {
                x: coord.x,
                y: coord.y,
                z: coord.z,
            }));
        }
        //Packed array access
        let result: Option<&T> = self.data.get(chunk_xyz_to_i(
            coord.x.as_usize(),
            coord.y.as_usize(),
            coord.z.as_usize(),
            self.size,
        ));
        Ok(result.unwrap())
    }

    fn set(
        &mut self,
        coord: VoxelPos<P>,
        value: T,
    ) -> Result<(), Self::Error> {
        let size = P::from_usize(self.size); 
        if (coord.x >= size) || (coord.y >= size) || (coord.z >= size) {
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
                coord.x.as_usize(),
                coord.y.as_usize(),
                coord.z.as_usize(),
                self.size,
            ))
            .unwrap() = value;

        Ok(())
    }
}

impl<T, P> VoxelStorageBounded<T, P> for VoxelArray<T, P>
        where T: Voxel,
        P: VoxelCoord + USizeAble {
    fn get_bounds(&self) -> VoxelRange<P> {
        self.bounds
    }
}

/// A 3D packed array of voxels. This much like VoxelArray,
/// but statically-sized - this may lead to better performance
/// due to better compiler optimizations.
#[derive(Clone, Debug)]
pub struct VoxelArrayStatic<T: Voxel + Copy, P: VoxelCoord + USizeAble, const SIZE: usize>
where
    [T; SIZE * SIZE * SIZE]: Sized,
{
    pub(crate) data: [T; SIZE * SIZE * SIZE],
    _phantom_coord: PhantomData<P>
}

impl<T: Voxel + Copy, P: VoxelCoord + USizeAble, const SIZE: usize> VoxelArrayStatic<T, P, SIZE>
where
    [T; SIZE * SIZE * SIZE]: Sized,
{
    pub fn load_new(data: [T; SIZE * SIZE * SIZE]) -> Self { 
        VoxelArrayStatic {
            data,
            _phantom_coord: PhantomData::default(),
        }
    }
    pub fn new(default_value: T) -> Self {
        VoxelArrayStatic {
            data: [default_value; SIZE * SIZE * SIZE],
            _phantom_coord: PhantomData::default(),
        }
    }
    /// Does not bounds check
    pub(crate) fn get_raw(&self, coord: VoxelPos<P>) -> &T {
        &self.data[chunk_xyz_to_i(coord.x.as_usize(), coord.y.as_usize(), coord.z.as_usize(), SIZE)]
    }
    /// Does not bounds check
    pub(crate) fn set_raw(&mut self, coord: VoxelPos<P>, value: T) {
        (*self
            .data
            .get_mut(chunk_xyz_to_i(
                coord.x.as_usize(),
                coord.y.as_usize(),
                coord.z.as_usize(),
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
    pub(crate) fn get_raw_ref<'a>(&'a self) -> &'a [T; SIZE * SIZE * SIZE] { 
        &self.data
    }
}

impl<T, P, const SIZE: usize> VoxelStorage<T, P> for VoxelArrayStatic<T, P, SIZE>
        where
            [u8; SIZE * SIZE * SIZE]: Sized,
            T: Voxel + Copy,
            P: VoxelCoord + USizeAble {
    type Error = VoxelArrayError<P>;
    fn get(&self, coord: VoxelPos<P>) -> Result<&T, Self::Error> {
        let size = P::from_usize(SIZE);
        //Bounds-check.
        if (coord.x >= size) || (coord.y >= size) || (coord.z >= size) {
            return Err(Self::Error::OutOfBounds(VoxelPos {
                x: coord.x,
                y: coord.y,
                z: coord.z,
            }));
        }
        //Packed array access
        let result: Option<&T> = self.data.get(chunk_xyz_to_i(
            coord.x.as_usize(),
            coord.y.as_usize(),
            coord.z.as_usize(),
            SIZE,
        ));
        Ok(result.unwrap())
    }

    fn set(
        &mut self,
        coord: VoxelPos<P>,
        value: T,
    ) -> Result<(), Self::Error> {
        let size = P::from_usize(SIZE);
        if (coord.x >= size) || (coord.y >= size) || (coord.z >= size) {
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
                coord.x.as_usize(),
                coord.y.as_usize(),
                coord.z.as_usize(),
                SIZE,
            ))
            .unwrap()) = value;

        Ok(())
    }
}

impl<T, P, const SIZE: usize> VoxelStorageBounded<T, P> for VoxelArrayStatic<T, P, SIZE>
    where
        [u8; SIZE * SIZE * SIZE]: Sized,
        T: Voxel + Copy,
        P: VoxelCoord + USizeAble {
    fn get_bounds(&self) -> VoxelRange<P> {
        let zero = P::from_usize(0usize);
        let size = P::from_usize(SIZE);
        VoxelRange {
            lower: VoxelPos { x: zero, y: zero, z: zero },
            upper: VoxelPos {
                x: size,
                y: size,
                z: size,
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

    let mut test_va: VoxelArray<u16, u16> = VoxelArray::load_new(16, test_chunk);

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

    let mut test_va: VoxelArray<u16, u16> = VoxelArray::load_new(16, test_chunk);
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

    let chunk_sz = 16_usize;
    let mut rng = rand::thread_rng();
    for _ in 0..4096 {
        let x = rng.gen_range(0, 16);
        let y = rng.gen_range(0, 16);
        let z = rng.gen_range(0, 16);

        let i_value = chunk_xyz_to_i(x, y, z, chunk_sz);
        let (x1, y1, z1) = chunk_i_to_xyz(i_value, chunk_sz);

        assert_eq!(x, x1);
        assert_eq!(y, y1);
        assert_eq!(z, z1);
    }
}

#[test]
fn chunk_index_bounds() {
    let chunk_sz = 16_usize;
    let chunk_volume = chunk_sz * chunk_sz * chunk_sz;
    for x in 0..chunk_sz {
        for y in 0..chunk_sz {
            for z in 0..chunk_sz {
                assert!(chunk_xyz_to_i(x, y, z, chunk_sz) < chunk_volume);
            }
        }
    }
}
