#![allow(unused_imports)]
//#![feature(collections)]
pub mod util;
pub mod voxel;
pub mod render;
//extern crate std;
use std::vec::Vec;
use voxel::voxelstorage::VoxelStorage;
use voxel::voxelarray::VoxelArray;
use render::dwarfmode::*;
use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io;

fn test_array_fileio() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u8);
    }

    let mut test_va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);

    assert!(test_va.get(14,14,14).unwrap() == 238);
    test_va.set(14,14,14,9);
    assert!(test_va.get(14,14,14).unwrap() == 9);
    
    let path = Path::new("hello.bin");

    // Open the path in read-only mode, returns `io::Result<File>`
    //let file : &mut File = try!(File::open(path));
    
    let display = path.display();
	let mut file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path) {
        // The `description` method of `io::Error` returns a string that
        // describes the error
        Err(why) => panic!("couldn't open {}: {}", display,
                                                   Error::description(&why)),
        Ok(file) => file,
    };
	// We create a buffered writer from the file we get
	//let mut writer = BufWriter::new(&file);
	    
   	test_va.save(&mut file);
    _load_test(path);
}

fn _load_test(path : &Path) -> bool {
	//let mut options = OpenOptions::new();
	// We want to write to our file as well as append new data to it.
	//options.write(true).append(true);
    let display = path.display();
	let mut file = match File::open(&path) {
        // The `description` method of `io::Error` returns a string that
        // describes the error
        Err(why) => panic!("couldn't open {}: {}", display,
                                                   Error::description(&why)),
        Ok(file) => file,
    };
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = Vec::with_capacity(OURSIZE);
    for _i in 0 .. OURSIZE {
    	test_chunk.push(0);
    }
	
    let mut va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    
    va.load(&mut file);
    
    assert!(va.get(14,14,14).unwrap() == 9);

    return true;
}

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
    
   	test_array_fileio();
	let mut input = String::new();
	
    for x in 0 .. test_va.get_x_sz().unwrap() {
	    for y in 0 .. test_va.get_y_sz().unwrap() {
		    for z in 0 .. test_va.get_z_sz().unwrap() {
	       		println!("Value of ({}, {}, {}) is {}", x, y, z, test_va.get(x, y, z).unwrap());
		    }
	    }
    }
    render_text(test_va.as_ref(), 14);
    println!("Enter any text to continue...");
	match io::stdin().read_line(&mut input) {
	    Ok(n) => {
	        println!("{} bytes read", n);
	        println!("{}", input);
	    }
	    Err(error) => println!("error: {}", error),
	}
}