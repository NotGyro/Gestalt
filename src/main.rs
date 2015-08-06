#![allow(unused_imports)]
//#![feature(collections)]
pub mod util;
pub mod voxel;
//extern crate std;
use std::vec::Vec;
use voxel::voxelstorage::VoxelStorage;
use voxel::voxelarray::VoxelArray;

//pub mod test;
#[allow(dead_code)]
fn main() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u8);
    }

    let mut test_va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    
    assert!(test_va.get(14,14,14).unwrap() == 238);
    test_va.set(14,14,14,9);
    assert!(test_va.get(14,14,14).unwrap() == 9);
}