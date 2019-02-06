use util::event::*;

extern crate serde;
extern crate std;
extern crate num;

use std::fmt::Debug;
use std::result::Result;

use num::Integer;
use voxel::*;
use voxel::voxelmath::*;
use voxel::voxelstorage::VoxelStorage;
use voxel::voxelarray::VoxelArray;

#[derive(Clone, Serialize, Debug)]
pub struct OneVoxelChange<T : Clone, P : Copy + Integer> {
    new_value : T,
    pos : VoxelPos<P>,
}

impl <T, P> OneVoxelChange<T, P> where T : Clone, P : Copy + Integer {
    fn apply(&self, stor : &mut VoxelStorage<T, P>) { 
        stor.set(self.pos, self.new_value.clone());
    }
}

#[derive(Clone, Serialize, Debug)]
pub struct SetVoxelRange<T : Clone, P : Copy + Integer> { 
    new_value : T, 
    range : VoxelRange<P>,
}

impl <T, P> SetVoxelRange<T, P> where T : Clone, P : Copy + Integer {
    fn apply(&self, stor : &mut VoxelStorage<T, P>) { 
        for pos in self.range { 
            stor.set(pos, self.new_value.clone());
        }
    }
}

#[derive(Clone, Serialize, Debug)]
enum VoxelEvent<T : Clone, P : Copy + Integer> {
    ChangeOne(OneVoxelChange<T, P>),
    SetRange(SetVoxelRange<T, P>),
}

impl <T, P> VoxelEvent<T, P> where T : Clone, P : Copy + Integer {
    fn apply(&self, stor : &mut VoxelStorage<T, P>) {
        match self {
            VoxelEvent::ChangeOne(evt) => evt.apply(stor),
            VoxelEvent::SetRange(evt) => evt.apply(stor),
        }
    }
}
const CHUNK_X_LENGTH : u32 = 16;
const CHUNK_Y_LENGTH : u32 = 16;
const CHUNK_Z_LENGTH : u32 = 16;
const OURSIZE : usize = (CHUNK_X_LENGTH * CHUNK_Y_LENGTH * CHUNK_Z_LENGTH) as usize;

#[test]
fn test_apply_voxel_event() { 
    let mut array : Vec<String> = vec!["Hello!".to_string(); OURSIZE];
    let mut storage : VoxelArray<String, u32> = VoxelArray::load_new(CHUNK_X_LENGTH, CHUNK_Y_LENGTH, CHUNK_Z_LENGTH, array);
    let evt : OneVoxelChange<String, u32> = OneVoxelChange{ new_value : "World!".to_string(), pos : VoxelPos { x: 7, y: 7, z:7}}; 
    evt.apply(&mut storage);
    assert_eq!(storage.get(VoxelPos{x: 6, y: 6, z: 6} ).unwrap(), "Hello!".to_string());
    assert_eq!(storage.get(VoxelPos{x: 7, y: 7, z: 7} ).unwrap(), "World!".to_string());
}
