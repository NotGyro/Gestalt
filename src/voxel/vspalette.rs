extern crate std;

extern crate serde;
extern crate serde_yaml;

extern crate num;

use std::marker::Copy;

use voxel::voxelstorage::*;
use voxel::voxelarray::*;

use std::string::String;
use std::vec::Vec;

use util::voxelutil::VoxelPos;
use util::voxelutil::VoxelRange;

use util::numbers::USizeAble;
use num::Integer;
use num::traits::identities::One;
use num::traits::identities::Zero;
use num::Unsigned; 

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

 
///Takes an underlying VoxelStorage and makes a palette of its values to another type.
///First type argument is resulting voxel, second type argument is underlying voxel, third is underyling VoxelStorage, fourth is position
#[derive(Clone, Debug)]
pub struct VoxelPalette<T, U, B, P> where T : Clone + Eq + Hash,
            P : Copy + Integer,
            U : Clone + Integer + Unsigned + USizeAble,
            B : VoxelStorage<U, P> { 
                
    pub base : Box<B>,
    pub index : Vec<T>,
    pub rev_index : HashMap<T, U>,
	position_type: std::marker::PhantomData<P>,
}
impl <T, U, B, P> VoxelPalette<T, U, B, P> where T : Clone + Eq + Hash,
            P : Copy + Integer,
            U : Clone + Integer + Unsigned + USizeAble, 
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
            P : Copy + Integer,
            U : Clone + Integer + Unsigned + USizeAble,
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
}

impl <T, U, B, P> VoxelStorageBounded<T, P> for VoxelPalette<T, U, B, P> where T : Clone + Eq + Hash,
            P : Copy + Integer,
            U : Clone + Integer + Unsigned + USizeAble,
            B : VoxelStorage<U, P> + VoxelStorageBounded<U,P> {
    fn get_bounds(&self) -> VoxelRange<P> { 
        return self.base.get_bounds();
    }
}

impl <T, U, B, P> VoxelStorageIOAble<T, P> for VoxelPalette<T, U, B, P> where T : Serialize + Deserialize + Clone + Eq + Hash,
            P : Copy + Integer,
            U : Clone + Integer + Unsigned + USizeAble,
            B : VoxelStorageIOAble<U, P> {
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn load<R: Read + Sized>(&mut self, reader: &mut R) { 
		self.base.load(reader);
        self.index = from_reader(reader).unwrap(); //TODO: Error handling
    }
    
    #[allow(mutable_transmutes)]
    #[allow(unused_must_use)]
    fn save<W: Write + Sized>(&self, writer: &mut W) -> Result<usize, std::io::Error> {
		let res = self.base.save(writer);
        to_writer(writer, &self.index);
        return res;
    }
}

#[test]
fn test_palette() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = vec![0; OURSIZE];

    let mut test_va : Box<VoxelArray<u8>> = VoxelArray::load_new(16, 16, 16, test_chunk);
    let mut test_p : VoxelPalette<String, u8, VoxelArray<u8>, u16> = VoxelPalette::new(test_va);
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
