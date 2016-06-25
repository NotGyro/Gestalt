extern crate std;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};
use std::string::String;
use std::vec::Vec;
use voxel::voxelstorage::VoxelStorage;

pub fn render_text(vox : &VoxelStorage<u8>, z : u32) {
	let mut out = String::new();
	out.push_str("\n");
    let xsz : u32 = vox.get_x_upper().unwrap();
    let ysz : u32 = vox.get_y_upper().unwrap();
    //let zsz : u32 = vox.get_z_upper().unwrap();
	for y in 0 .. ysz as u32 {
		for x in 0 .. xsz as u32 {
			if vox.get(x, y, z).unwrap() > 64 {
				out.push_str(".");
			}
			else {
				out.push_str("*");
			}
		}
		out.push_str("\n");
	}
	print!("{}", out);
}