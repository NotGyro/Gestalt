/* A basic trait for any 3d grid of data.
For this trait, a single level of detail is assumed.

For voxel data structures with a level of detail, we will
assume that the level of detail is a signed integer, and
calling these methods / treating them as "flat" voxel
structures implies acting on a level of detail of 0. */

extern crate std;
//use std::ops::{Add, Sub, Mul, Div};
//use std::cmp::{Ord, Eq};
use std::mem::size_of;
use voxel::voxelstorage::VoxelStorage;
//use voxel::voxelstorage::ContiguousVS;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;

// Type arguments are type of element, type of position / index.
pub struct VoxelArray<T: Copy> {
    size_x: u32, size_y: u32, size_z: u32,
    data: Vec<T>,
}

impl <T:Copy> VoxelArray<T> {

	pub fn load_new(szx: u32, szy: u32, szz: u32, dat: Vec<T>) -> Box<VoxelArray<T>> {
		return Box::new(VoxelArray{size_x: szx, size_y: szy, size_z: szz, data: dat});
	}
}

impl <T: Copy> VoxelStorage<T> for VoxelArray<T> {
    fn get(&self, x: u32, y: u32, z: u32) -> Option<T> {
    	//Bounds-check.
    	if (x >= self.size_x) ||
    		(y >= self.size_y) ||
    		(z >= self.size_z)
    	{
    		return None;
    	}
    	//Packed array access
    	let result : Option<&T> = self.data.get((
    		(z * (self.size_x * self.size_y)) +
    		(y * (self.size_x))
    		+ x) as usize);
    	if result.is_none() {
    		return None;
    	}
    	else {
    		return Some(*result.unwrap());
    	}
    }

    fn set(&mut self, x: u32, y: u32, z: u32, value: T) {
    	if (x >= self.size_x) ||
    		(y >= self.size_y) ||
    		(z >= self.size_z)
    	{
    		return;
    	}
    	//Packed array access
    	(*self.data.get_mut((
    		(z * (self.size_x * self.size_y)) +
    		(y * (self.size_x))
    		+ x) as usize).unwrap()) = value;
    }

    //Intializes a voxel storage, with each cell set to default value.
    //fn init_new(&mut self, size_x: P, size_y: P, size_z: P, default: T);
    //Uninitialized version of the above. Still allocates, probably.
    //fn init_new_uninitialized(&mut self, size_x: P, size_y: P, size_z: P);

    //Gets how many bytes this structure takes up in memory.
    /*fn get_footprint(&self) -> usize {
    	return ((size_of::<T>() as u32) * (self.size_x * self.size_y * self.size_z)) as usize;
    }*/

    fn get_x_upper(&self) -> Option<u32> {
    	Some(self.size_x as u32)
    }
    fn get_y_upper(&self)  -> Option<u32> {
    	Some(self.size_y as u32)
    }
    fn get_z_upper(&self)  -> Option<u32> {
    	Some(self.size_z as u32)
    }
    
    fn get_x_lower(&self) -> Option<u32> {
        Some(0)
    }
    fn get_y_lower(&self)  -> Option<u32>{
        Some(0)
    }
    fn get_z_lower(&self)  -> Option<u32>{
        Some(0)
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load(&mut self, reader: &mut Read) { 
		let array: &mut [u8] = unsafe { mem::transmute(&*self.data) };
    	reader.read(array);
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save(&mut self, writer: &mut Write) {
		let array: &mut [u8] = unsafe { mem::transmute(&*self.data) };
    	writer.write(array).unwrap();
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

    assert!(test_va.get(14,14,14).unwrap() == 3822);
    test_va.set(14,14,14,9);
    assert!(test_va.get(14,14,14).unwrap() == 9);
}


#[test]
fn test_array_iterative() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for _i in 0 .. OURSIZE {
    	test_chunk.push(16);
    }

    let mut test_va : Box<VoxelArray<u16>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    let xsz : u32 = test_va.get_x_sz().unwrap();
    let ysz : u32 = test_va.get_y_sz().unwrap();
    let zsz : u32 = test_va.get_z_sz().unwrap();
	for x in 0 .. xsz as u32 {
		for y in 0 .. ysz as u32 {
			for z in 0 .. zsz as u32 {
				assert!(test_va.get(x,y,z).unwrap() == 16);
				test_va.set(x,y,z, (x as u16 % 10));
			}
		}
	}
	assert!(test_va.get(10,0,0).unwrap() == 0);
	assert!(test_va.get(11,0,0).unwrap() == 1);
}

#[test]
fn test_array_fileio() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u16);
    }

    let mut test_va : Box<VoxelArray<u16>> = VoxelArray::load_new(16, 16, 16, test_chunk);

    assert!(test_va.get(14,14,14).unwrap() == 3822);
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

#[allow(dead_code)]
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
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for _i in 0 .. OURSIZE {
    	test_chunk.push(0);
    }
	
    let mut va : Box<VoxelArray<u16>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    
    va.load(&mut file);
    
    assert!(va.get(14,14,14).unwrap() == 9);

    return true;
}

