//extern crate serde;
extern crate std;
extern crate num;
extern crate serde;

use std::error::Error;
use std::fmt::Debug;
use std::result::Result;
use serde::{Serialize, Deserialize};

use voxel::voxelmath::*;
use voxel::voxelstorage::{VoxelStorage, Voxel, VoxelError};
#[cfg(test)]
use voxel::voxelarray::VoxelArray;

pub type EventTypeID = u8;

pub trait VoxelEventBounds : Clone + Debug + Default {}
impl<T> VoxelEventBounds for T where T : Clone + Debug + Default {}

/*
#[derive(Debug, Clone)]
struct EventApplyError {}

impl Error for EventApplyError {
    fn description(&self) -> &str {
        "An attempt to apply a VoxelEvent to a VoxelStorage has failed."
    }
}
*/
pub type EventApplyResult = Result<(), VoxelError>;

/// Represents a change to the contents of a Voxel Storage.
/// Type arguments are voxel type, position type. This is the version of this trait
/// with no run-time type information.
pub trait VoxelEventInner <T, P> : VoxelEventBounds where T : Voxel, P : VoxelCoord {
    /// Applies a voxel event to a VoxelStorage.
    /// The intended use of this is as a default case, and ideally specific 
    /// VoxelStorage implementations could provide better-optimized 
    fn apply_blind(&self, stor : &mut dyn VoxelStorage<T, P>) -> EventApplyResult;
}

/*
/// Type arguments are voxel type, position type.
pub trait VoxelEvent<T, P>: VoxelEventUntyped<T, P> where T : Clone + Debug + Send + Sync, P : Copy + Integer + Debug + Send + Sync {
    const TYPE_ID: EventTypeID;
    fn get_type_id() -> EventTypeID { Self::TYPE_ID }
}
*/

// ---- Actual event structs and their VoxelEventUntyped implementations. ----

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OneVoxelChange<T, P> where T : Voxel, P : VoxelCoord {
    pub new_value : T,
    pub pos : VoxelPos<P>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct SetVoxelRange<T, P> where T : Voxel, P : VoxelCoord { 
    pub new_value : T, 
    pub range : VoxelRange<P>,
}

impl <T, P> VoxelEventInner<T, P> for OneVoxelChange<T, P> where T : Voxel, P : VoxelCoord {
    fn apply_blind(&self, stor : &mut dyn VoxelStorage<T, P>) -> EventApplyResult {
        stor.set(self.pos, self.new_value.clone())?;
        Ok(()) // TODO: modify VoxelStorage's "Set" method to return errors rather than silently fail
    }
}

impl <T, P> VoxelEventInner<T, P> for SetVoxelRange<T, P> where T : Voxel, P : VoxelCoord {
    fn apply_blind(&self, stor : &mut dyn VoxelStorage<T, P>) -> EventApplyResult {
        for pos in self.range {
            stor.set(pos, self.new_value.clone())?; 
        }
        Ok(()) // TODO: modify VoxelStorage's "Set" method to return errors rather than silently fail
    }
}

//TODO: Generate this with a macro
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VoxelEvent<T, P> where T : Voxel, P : VoxelCoord {
    Nop,
    SetOne(OneVoxelChange<T,P>),
    SetRange(SetVoxelRange<T,P>),
}

impl <T, P> VoxelEventInner<T, P> for VoxelEvent<T, P> where T : Voxel, P : VoxelCoord {
    fn apply_blind(&self, stor : &mut dyn VoxelStorage<T, P>) -> EventApplyResult {
        match self { 
            VoxelEvent::SetOne(evt) => evt.apply_blind(stor),
            VoxelEvent::SetRange(evt) => evt.apply_blind(stor),
            VoxelEvent::Nop => Ok(()),
        }
    }
}

impl <T: Voxel, P:VoxelCoord> Default for VoxelEvent<T, P> {
    /// Make a new VoxelArray wherein every value is set to T::Default
    #[inline]
    fn default() -> Self { VoxelEvent::Nop }
}

// ------ Temporary impls before we make the macro ------
/*
impl <T, P> VoxelEvent<T, P> for OneVoxelChange<T, P> where T : Clone + Debug + Send + Sync, P : Copy + Integer + Debug + Send + Sync{
    const TYPE_ID: EventTypeID = 2;
}

impl <T, P> VoxelEvent<T, P> for SetVoxelRange<T, P> where T : Clone + Debug + Send + Sync, P : Copy + Integer + Debug + Send + Sync{
    const TYPE_ID: EventTypeID = 3;
}*/

// ----------------------- Tests -----------------------

// Used for tests
const CHUNK_X_LENGTH : u32 = 16;
const CHUNK_Y_LENGTH : u32 = 16;
const CHUNK_Z_LENGTH : u32 = 16;
const OURSIZE : usize = (CHUNK_X_LENGTH * CHUNK_Y_LENGTH * CHUNK_Z_LENGTH) as usize;

#[test]
fn test_apply_voxel_event() { 
    let array : Vec<String> = vec!["Hello!".to_string(); OURSIZE];
    let mut storage : VoxelArray<String, u32> = VoxelArray::load_new(CHUNK_X_LENGTH, CHUNK_Y_LENGTH, CHUNK_Z_LENGTH, array);
    let evt : OneVoxelChange<String, u32> = OneVoxelChange{ new_value : "World!".to_string(), pos : VoxelPos { x: 7, y: 7, z:7}}; 
    evt.apply_blind(&mut storage).unwrap();
    assert_eq!(storage.get(VoxelPos{x: 6, y: 6, z: 6} ).unwrap(), "Hello!".to_string());
    assert_eq!(storage.get(VoxelPos{x: 7, y: 7, z: 7} ).unwrap(), "World!".to_string());
}