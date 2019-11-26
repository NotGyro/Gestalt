use crate::voxel::subdivmath::OctPos;
use crate::voxel::traits::{VoxelSourceAbstract, VoxelDrainAbstract, VoxelStorageAbstract};
use std::fmt::Debug;
use serde::export::Formatter;
use serde::export::fmt::Error;


// Definitions /////////////////////////////////////////////////////////////////////////////////////


pub const CHUNK_SCALE: usize = 5;
pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_ELEMENTS: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

pub type ArrayStorageType = [u8; CHUNK_ELEMENTS];


// Storage Type ////////////////////////////////////////////////////////////////////////////////////


pub struct ArrayVoxelStorage {
    pub data: [u8; CHUNK_ELEMENTS],
}
impl Debug for ArrayVoxelStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), Error> {
        f.write_str("[ArrayVoxelStorage]").unwrap();
        Ok(())
    }
}

impl ArrayVoxelStorage {
    pub fn new() -> Self {
        Self {
            data: [0u8; CHUNK_ELEMENTS],
        }
    }
}

impl VoxelSourceAbstract<u8, (), usize> for ArrayVoxelStorage {
    fn get(&self, coord: OctPos<usize>) -> Result<&u8, String> {
        if bounds_check_coord(coord) {
            Ok(&self.data[coord_to_index(coord)])
        }
        else {
            Err("OutOfBounds".into())
        }
    }

    fn get_max_scale(&self) -> i8 { 5 }
    fn get_min_scale(&self) -> i8 { 0 }

    fn traverse<F>(&self, func: &mut F) where F: FnMut(OctPos<usize>, &u8) -> bool {
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    if !func(OctPos::from_four(x, y, z, 0), &self.data[xyz_to_index(x, y, z)]) {
                        return;
                    }
                }
            }
        }
    }
}

impl VoxelDrainAbstract<u8, usize> for ArrayVoxelStorage {
    fn set(&mut self, coord: OctPos<usize>, value: u8) -> Result<(), String> {
        if bounds_check_coord(coord) {
            self.data[coord_to_index(coord)] = value;
            Ok(())
        }
        else {
            Err("OutOfBounds".into())
        }
    }

    fn traverse_mut<F>(&mut self, _func: &mut F) where F: FnMut(OctPos<usize>, &mut u8) -> bool {
        unimplemented!();
    }
}

impl VoxelStorageAbstract<ArrayStorageType> for ArrayVoxelStorage {
    fn replace_data(&mut self, new_data: [u8; 32768]) {
        self.data = new_data;
    }
}


// Helper functions ////////////////////////////////////////////////////////////////////////////////


fn coord_to_index(pos: OctPos<usize>) -> usize {
    ((pos.pos.x * CHUNK_SIZE * CHUNK_SIZE) + (pos.pos.y * CHUNK_SIZE) + pos.pos.z)
}

pub fn xyz_to_index(x: usize, y: usize, z: usize) -> usize {
    ((x * CHUNK_SIZE * CHUNK_SIZE) + (y * CHUNK_SIZE) + z)
}

fn bounds_check_coord(coord: OctPos<usize>) -> bool {
    coord.pos.x < CHUNK_SIZE &&
        coord.pos.y < CHUNK_SIZE &&
        coord.pos.z < CHUNK_SIZE
}

#[allow(dead_code)]
fn bounds_check_index(i: usize) -> bool {
    i < CHUNK_ELEMENTS
}