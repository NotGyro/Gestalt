#![feature(collections)]

pub mod voxel;
use voxel::VoxelStorage;
use voxel::VoxelArray;

fn main() {
    const oursize : usize  = 16 * 16 * 16;
    let testchunk: [mut u8; oursize] = [0, oursize];

    let testVA : VoxelArray<u8, u16> = VoxelArray { sizeX : 16,
        sizeY : 16,
        sizeZ : 16,
        data : &testchunk,
    };
    
    println!("Hello, world!");
}
