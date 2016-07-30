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
use voxel::voxelstorage::*;
//use voxel::voxelstorage::ContiguousVS;
use std::io;
use std::io::prelude::*;
use std::mem;
use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::borrow::Cow;

// Type arguments are type of element, type of position / index.
#[derive(Clone, Debug)]
pub struct VoxelArray<T: Clone> {
    size_x: u16, size_y: u16, size_z: u16,
    data: Vec<T>,
}

impl <T:Clone> VoxelArray<T> {

	pub fn load_new(szx: u16, szy: u16, szz: u16, dat: Vec<T>) -> Box<VoxelArray<T>> {
		return Box::new(VoxelArray{size_x: szx, size_y: szy, size_z: szz, data: dat});
	}
}

impl <T: Clone> VoxelStorage<T> for VoxelArray<T> {
    fn get(&self, x: u16, y: u16, z: u16) -> Option<T> {
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
    		return Some(result.unwrap().clone());
    	}
    }

    fn set(&mut self, x: u16, y: u16, z: u16, value: T) {
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
    	return ((size_of::<T>() as u16) * (self.size_x * self.size_y * self.size_z)) as usize;
    }*/

    fn get_x_upper(&self) -> Option<u16> {
    	Some(self.size_x as u16)
    }
    fn get_y_upper(&self)  -> Option<u16> {
    	Some(self.size_y as u16)
    }
    fn get_z_upper(&self)  -> Option<u16> {
    	Some(self.size_z as u16)
    }
    
    fn get_x_lower(&self) -> Option<u16> {
        Some(0)
    }
    fn get_y_lower(&self)  -> Option<u16>{
        Some(0)
    }
    fn get_z_lower(&self)  -> Option<u16>{
        Some(0)
    }
    
}
impl <T: Clone> VoxelStorageIOAble<T> for VoxelArray<T> { 
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load<R: Read + Sized>(&mut self, reader: &mut R) { 
		let array: &mut [u8] = unsafe { mem::transmute(&*self.data) };
    	reader.read(array);
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save<W: Write + Sized>(&self, writer: &mut W) -> Result<usize, std::io::Error> {
		let array: &[u8] = unsafe { mem::transmute(&*self.data) };
    	writer.write(array)
    }
}
/*
impl <T: Clone> VSContiguousWritable<T> for VoxelArray<T> {
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load(&mut self, reader: &mut Read) { 
		let array: &mut [u8] = unsafe { mem::transmute(&*self.data) };
    	reader.read(array);
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save(&self, writer: &mut Write) {
		let array: &[u8] = unsafe { mem::transmute(&*self.data) };
    	writer.write(array).unwrap();
    }
    fn is_constant_size(&self) -> bool { true }
    /*Gets the size of data which would be written. 
    Doesn't represent the memory footprint of the struct - the size of some things, 
    like caches and certain indexes, might not need to be saved.*/
    fn get_data_size(&self) -> usize { self.data.len() * std::mem::size_of::<T>() }
}*/

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
    let xsz : u16 = test_va.get_x_upper().unwrap();
    let ysz : u16 = test_va.get_y_upper().unwrap();
    let zsz : u16 = test_va.get_z_upper().unwrap();
	for x in 0 .. xsz as u16 {
		for y in 0 .. ysz as u16 {
			for z in 0 .. zsz as u16 {
				assert!(test_va.get(x,y,z).unwrap() == 16);
				test_va.set(x,y,z, (x as u16 % 10));
			}
		}
	}
	assert!(test_va.get(10,0,0).unwrap() == 0);
	assert!(test_va.get(11,0,0).unwrap() == 1);
    assert_eq!(test_va.get_data_size(), (OURSIZE * 2));
}
/*
#[test]
fn test_array_fileio() {
    let path = Path::new("hello.bin");
    _save_test(path);
    _load_test(path);
}
#[allow(dead_code)]
fn _save_test(path : &Path) {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u16> = Vec::with_capacity(OURSIZE);
    for i in 0 .. OURSIZE {
    	test_chunk.push(i as u16);
    }

    let mut test_va : Box<VoxelArray<u16>> = VoxelArray::load_new(16, 16, 16, test_chunk);

    assert!(test_va.get(14,14,14).unwrap() == 3822);
    test_va.set(14,14,14,9);
    assert!(test_va.get(14,14,14).unwrap() == 9);
    

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
}



#[allow(dead_code)]
fn _load_test(path : &Path) {
	//let mut options = OpenOptions::new();
	// We want to write to our file as well as append new data to it.
	//options.write(true).append(true);
    let display = path.display();
	let mut file = match OpenOptions::new()
            .read(true)
            .open(&path) {
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
}

*/