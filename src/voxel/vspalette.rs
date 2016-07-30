extern crate std;

extern crate serde;
extern crate serde_yaml;

use voxel::voxelstorage::*;
use voxel::voxelarray::*;
use std::ops::{Add, Sub, Mul, Div};
use std::cmp::{Ord, Eq};
use std::string::String;
use std::vec::Vec;
use util::voxelutil::VoxelPos;
use std::io;
use std::io::prelude::*;
use std::ops::{Index, IndexMut};
use std::collections::HashMap;
use std::hash::Hash;

use self::serde::ser::Serialize;
use self::serde_yaml::ser::Serializer;
use self::serde_yaml::ser::to_writer;
use self::serde::de::Deserialize;
use self::serde_yaml::de::Deserializer;
use self::serde_yaml::de::from_reader;

/* 
Rust's trait bound system HAAAAAAAAATES this file.

Consider reimplementing in another way.

...Or, I could implement my own trait for usize-able types.

Takes an underlying VoxelStorage and makes a palette of its values to another type.
First type argument is resulting type, second type argument is underlying type, third is position
*/
pub trait USizeAble {
    fn as_usize(&self) -> usize;
    fn from_usize(val : usize) -> Self;
}

impl USizeAble for u8 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u8
    }    
}
impl USizeAble for u16 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u16
    }    
}
impl USizeAble for u32 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u32
    }    
}
impl USizeAble for u64 {
    fn as_usize(&self) -> usize {
        (*self) as usize
    }
    fn from_usize(val : usize) -> Self {
        val as u64
    }    
}

#[derive(Clone, Debug)]
pub struct VoxelPalette<T, U, B, P> where T : Clone + Eq + Hash,
            P : Copy + Eq + Ord + Add + Sub + Mul + Div,
            U : Clone + Eq + USizeAble + Eq + Ord + Add + Sub + Mul + Div,
            B : VoxelStorage<U, P> { 
                
    pub base : Box<B>,
    pub index : Vec<T>,
    pub rev_index : HashMap<T, U>,
	position_type: std::marker::PhantomData<P>,
}
impl <T, U, B, P> VoxelPalette<T, U, B, P> where T : Clone + Eq + Hash,
            P : Copy + Eq + Ord + Add + Sub + Mul + Div,
            U : Clone + Eq + USizeAble + Eq + Ord + Add + Sub + Mul + Div, 
            B : VoxelStorage<U, P> { 
    pub fn new( b : Box<B>) -> VoxelPalette<T, U, B, P> {
        VoxelPalette { base : b, index : Vec::new(), rev_index : HashMap::new(), position_type: std::marker::PhantomData,}
    }
    pub fn init_default_value(&mut self, value : T, idx : U) {
        if self.index.len() <= idx.as_usize() { //We haven't initialized yet, which is great.
            self.index.push(value.clone());
            self.rev_index.insert(value.clone(), idx);
        }
        else { 
            self.index[idx.as_usize()] = value.clone();
            self.rev_index.remove(&value);
            self.rev_index.insert(value.clone(), idx);
        }
    }
}

impl <T, U, B, P> VoxelStorage<T, P> for VoxelPalette<T, U, B, P> where T : Clone + Eq + Hash,
            P : Copy + Eq + Ord + Add + Sub + Mul + Div,
            U : Clone + Eq + USizeAble + Eq + Ord + Add + Sub + Mul + Div,
            B : VoxelStorage<U, P> {
                
    fn get(&self, x: P, y: P, z: P) -> Option<T> {
    	let voxmaybe = self.base.get(x,y,z);
        if voxmaybe.is_some() {
            let val = voxmaybe.unwrap();
            if(val.as_usize() >= self.index.len()) {
                panic!("Invalid value for voxel palette! Either the map is corrupt or something is very wrong.");
            }
            return Some(self.index[val.as_usize()].clone());
        }
        return None;
    }

    fn set(&mut self, x: P, y: P, z: P, value: T) {
        if self.rev_index.contains_key(&value) {
    	    self.base.set(x,y,z, (*self.rev_index.get(&value).unwrap()).clone());
        }
        else {
            let newidx = U::from_usize(self.index.len());
            self.index.push(value.clone());
            self.rev_index.insert(value.clone(), newidx.clone());
            self.base.set(x,y,z, newidx);
        }
    }

    fn get_x_upper(&self) -> Option<P> {
    	self.base.get_x_upper()
    }
    fn get_y_upper(&self)  -> Option<P> {
    	self.base.get_y_upper()
    }
    fn get_z_upper(&self)  -> Option<P> {
    	self.base.get_z_upper()
    }
    
    fn get_x_lower(&self) -> Option<P> {
    	self.base.get_x_lower()
    }
    fn get_y_lower(&self)  -> Option<P>{
    	self.base.get_y_lower()
    }
    fn get_z_lower(&self)  -> Option<P>{
    	self.base.get_z_lower()
    }
    /*
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load(&mut self, reader: &mut Read) { 
        //TODO: Include the palette in here.
		self.base.load(reader);
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save(&self, writer: &mut Write) {
        //TODO: Include the palette in here.
		self.base.save(writer);
    }*/
}

impl <T, U, B, P> VoxelStorageIOAble<T, P> for VoxelPalette<T, U, B, P> where T : Serialize + Deserialize + Clone + Eq + Hash,
            P : Copy + Eq + Ord + Add + Sub + Mul + Div,
            U : Clone + Eq + USizeAble + Eq + Ord + Add + Sub + Mul + Div,
            B : VoxelStorageIOAble<U, P> {
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load<R: Read + Sized>(&mut self, reader: &mut R) { 
        //TODO: Include the palette in here.
		self.base.load(reader);
        self.index = from_reader(reader).unwrap(); //TODO: Error handling
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save<W: Write + Sized>(&self, writer: &mut W) -> Result<usize, std::io::Error> {
        //TODO: Include the palette in here.
		let res = self.base.save(writer);
        to_writer(writer, &self.index);
        return res;
    }
}
/*
impl <u32, T> Index<u32> for Vec<T> {
    type Output = T;

    fn index<'a>(&'a self, _index : u32) -> &'a T {
        self[_index as usize]
    }
}
impl <u32, T> IndexMut<u32> for Vec<T> {
    fn index_mut<'a>(&'a mut self, _index : u32) -> &'a T {
        self[_index as usize]
    }
}


impl <u16, T> Index<u16> for Vec<T> {
    type Output = T;

    fn index<'a>(&'a self, _index : u16) -> &'a T {
        self[_index as usize]
    }
}
impl <u16, T> IndexMut<u16> for Vec<T> {
    fn index_mut<'a>(&'a mut self, _index : u16) -> &'a T {
        self[_index as usize]
    }
}


impl <u8, T> Index<u8> for Vec<T> {
    type Output = T;

    fn index<'a>(&'a self, _index : u8) -> &'a T {
        self[_index as usize]
    }
}
impl <u8, T> IndexMut<u8> for Vec<T> {
    fn index_mut<'a>(&'a mut self, _index : u8) -> &'a T {
        self[_index as usize]
    }
}
*/

#[test]
fn test_palette() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = vec![0; OURSIZE];

    let mut test_va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    let mut test_p : VoxelPalette<String, u8, VoxelArray<u8>, u32> = VoxelPalette {base : test_va, index : Vec::new(), rev_index : HashMap::new() };
    let testmat1 = String::from("test1");
    let testmat2 = String::from("test2");
    test_p.init_default_value(String::from("ungenerated"), 0);
	test_p.set(12, 12, 12, testmat1.clone());
    test_p.set(4, 4, 4, testmat1.clone());
    test_p.set(5, 5, 5, testmat2.clone());
    test_p.set(5, 6, 5, testmat2.clone());
    println!("{}", test_p.get(1, 1, 1).unwrap());
    println!("{}", test_p.get(12, 12, 12).unwrap());
    println!("{}", test_p.get(5, 5, 5).unwrap());
    assert_eq!(test_p.get(12, 12, 12).unwrap(), testmat1);
    assert_eq!(test_p.get(4, 4, 4).unwrap(), testmat1);
    assert_eq!(test_p.get(5, 5, 5).unwrap(), testmat2);
    assert_eq!(test_p.get(5, 6, 5).unwrap(), testmat2);
    /*
    assert_eq!(test_va.get(12, 12, 12).unwrap(), 1);
    assert_eq!(test_va.get(4, 4, 4).unwrap(), 1);
    
    assert_eq!(test_va.get(5, 5, 5).unwrap(), 2);
    assert_eq!(test_va.get(5, 6, 5).unwrap(), 2);
    */
}
