use super::voxelstorage;
use super::voxelstorage::*;
use super::voxelarray::*;

use std::marker::PhantomData;
use std::string::String;
use std::vec::Vec;

use crate::common::voxelmath::VoxelCoord;
use crate::common::voxelmath::VoxelPos;
use crate::common::voxelmath::VoxelRange;

use crate::common::voxelmath::USizeAble;
use num::Integer;
use num::traits::identities::One;
use num::traits::identities::Zero;
use num::Unsigned;
use serde::de::value::UsizeDeserializer; 

use std::io;
use std::io::prelude::*;
use std::ops::{Index, IndexMut};
use std::collections::HashMap;
use std::hash::Hash;

use serde::{Serialize, Deserialize};

///Takes an underlying VoxelStorage and makes a palette of its values to another type.
///First type argument is resulting voxel, second type argument is underlying voxel, third is underyling VoxelStorage, fourth is position
#[derive(Clone, Debug)]
pub struct VoxelPalette<T, U, S, P> where T : Voxel + Eq + Hash,
            P : VoxelCoord,
            U : Voxel+Copy+Eq+Hash+USizeAble,
            S : VoxelStorage<U, P> { 
                
    pub base : S,
    pub index : Vec<T>,
    pub rev_index : HashMap<T, U>,
    pub _position_type_anchor: PhantomData<P>,
}
impl <T, U, S, P> VoxelPalette<T, U, S, P> where T : Voxel + Eq + Hash,
        P : VoxelCoord,
        U : Voxel+Copy+Eq+Hash+USizeAble,
        S : VoxelStorage<U, P> { 
    pub fn new( b : S) -> VoxelPalette<T, U, S, P> {
        VoxelPalette { base : b, index : Vec::new(), rev_index : HashMap::new(), _position_type_anchor: PhantomData::default()}
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

impl <T, U, S, P> VoxelStorage<T, P> for VoxelPalette<T, U, S, P> where T : Voxel + Eq + Hash,
        P : VoxelCoord,
        U : Voxel+Copy+Eq+Hash+USizeAble,
        S : VoxelStorage<U, P> {
                
    fn get(&self, pos: VoxelPos<P>) -> Result<T, voxelstorage::VoxelError> {
    	let voxmaybe = self.base.get(pos);
        voxmaybe.and_then(|val| {
            if val.as_usize() >= self.index.len() {
                Err(VoxelError::PaletteIssue(val.as_usize(), self.index.len()))
            }
            else { 
                Ok(self.index[val.as_usize()].clone())
            }
        })
    }

    fn set(&mut self, pos: VoxelPos<P>, value: T) -> Result<(), voxelstorage::VoxelError> {
        if self.rev_index.contains_key(&value) {
    	    self.base.set(pos, (*self.rev_index.get(&value).unwrap()))
        }
        else {
            let newidx = U::from_usize(self.index.len());
            self.index.push(value.clone());
            self.rev_index.insert(value.clone(), newidx.clone());
            self.base.set(pos, newidx)
        }
    }
}

impl <T, U, S, P> VoxelStorageBounded<T, P> for VoxelPalette<T, U, S, P> where T : Voxel + Eq + Hash,
        P : VoxelCoord,
        U : Voxel+Copy+Eq+Hash+USizeAble,
        S : VoxelStorage<U, P> + VoxelStorageBounded<U, P> {
    fn get_bounds(&self) -> VoxelRange<P> {
        return self.base.get_bounds();
    }
}

#[test]
fn test_palette() {
    const OURSIZE : usize  = 16 * 16 * 16;
    let mut test_chunk : Vec<u8> = vec![0; OURSIZE];

    let mut test_va : VoxelArray<u8> = VoxelArray::load_new(16, test_chunk);
    let mut test_p : VoxelPalette<String, u8, VoxelArray<u8>, u16> = VoxelPalette::new(test_va);
    let testmat1 = String::from("test1");
    let testmat2 = String::from("test2");
    test_p.init_default_value(String::from("ungenerated"), 0);
	test_p.set(vpos!(12, 12, 12), testmat1.clone()).unwrap();
    test_p.set(vpos!(4, 4, 4), testmat1.clone()).unwrap();
    test_p.set(vpos!(5, 5, 5), testmat2.clone()).unwrap();
    test_p.set(vpos!(5, 6, 5), testmat2.clone()).unwrap();
    println!("{}", test_p.get(vpos!(1, 1, 1)).unwrap());
    println!("{}", test_p.get(vpos!(12, 12, 12)).unwrap());
    println!("{}", test_p.get(vpos!(5, 5, 5)).unwrap());
    assert_eq!(test_p.get(vpos!(12, 12, 12)).unwrap(), testmat1);
    assert_eq!(test_p.get(vpos!(4, 4, 4)).unwrap(), testmat1);
    assert_eq!(test_p.get(vpos!(5, 5, 5)).unwrap(), testmat2);
    assert_eq!(test_p.get(vpos!(5, 6, 5)).unwrap(), testmat2);
    /*
    assert_eq!(test_va.get(12, 12, 12).unwrap(), 1);
    assert_eq!(test_va.get(4, 4, 4).unwrap(), 1);
    
    assert_eq!(test_va.get(5, 5, 5).unwrap(), 2);
    assert_eq!(test_va.get(5, 6, 5).unwrap(), 2);
    */
}