//#![feature(collections)]

pub mod voxel;
//extern crate std;
use std::vec::Vec;
use voxel::voxelstorage::VoxelStorage;
use voxel::voxelarray::VoxelArray;

fn main() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u8);
    }

    let mut test_va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    
    println!("Our value = {}", test_va.get(14,14,14).unwrap());
    println!("Setting...");
    test_va.set(14,14,14,9);
    println!("Our value = {}", test_va.get(14,14,14).unwrap());
}