use std::result::Result;
use std::error::Error;
use std::fmt::Debug;
/*
use crate::common::voxelmath::*;
use crate::common::message::*;
use crate::world::{TilePos, TileCoord};
use crate::world::Space;
use crate::world::tile::TileId;

use ustr::*;
use serde::{Serialize, Deserialize};

pub const VOXEL_EVENT_SINGLE_ID : u8 = 1;
pub const VOXEL_EVENT_CUBOID_ID : u8 = 2;

#[enum_dispatch(VoxelEvent)]
trait VoxelEventApply {
    fn apply_to(&self, space: &mut Space) -> Result<(), Box<dyn Error>>;
    fn type_num(&self) -> u8;
    fn serialize_data(&self) -> Vec<u8>;
}

impl VoxelEventApply for VoxelEventSingle {
    fn apply_to(&self, space: &mut Space) -> Result<(), Box<dyn Error>> {    
        Ok(space.set(self.position, self.new_id)?)
    }
    fn type_num(&self) -> u8 { 
        VOXEL_EVENT_SINGLE_ID
    }
    fn serialize_data(&self) -> Vec<u8> { 
        bincode::serialize(&self).unwrap()
    }
}

impl VoxelEventApply for VoxelEventCuboid {
    #[no_mangle]
    fn apply_to(&self, space: &mut Space) -> Result<(), Box<dyn Error>> {   
        for position in self.cuboid {
            space.set(position, self.new_id)?;
        }
        Ok(())
    }
    fn type_num(&self) -> u8 { 
        VOXEL_EVENT_CUBOID_ID
    }
    fn serialize_data(&self) -> Vec<u8> { 
        bincode::serialize(&self).unwrap()
    }
}

/// Sets one block. 
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoxelEventSingle {
    pub new_id: TileId,
    pub position: TilePos,
}
/// Sets a cuboid range of blocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VoxelEventCuboid {
    pub new_id: TileId,
    pub cuboid: VoxelRange<TileCoord>,
}

#[enum_dispatch]
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum VoxelEvent {
    Single(VoxelEventSingle),
    Cuboid(VoxelEventCuboid),
}

impl RegisteredMessage for VoxelEvent {
    fn msg_ty() -> MsgTypeId { ustr("gestalt.VoxelEvent") }
    fn unpack(msg: &MsgData) -> Result<Self, Box<dyn Error>>{
        let (ty_vec, evt_data) = msg.split_at(1);
        let ty : u8 = ty_vec[0];
        Ok(
            match ty {
                //Reserving 0 for now.
                0 => return Err(Box::new(MessageError::InvalidMessage{msg_ty: Self::msg_ty()})),
                VOXEL_EVENT_SINGLE_ID => { 
                    VoxelEvent::Single(bincode::deserialize(evt_data)?)
                },
                VOXEL_EVENT_CUBOID_ID => { 
                    VoxelEvent::Cuboid(bincode::deserialize(evt_data)?)
                },
                _ => return Err(Box::new(MessageError::InvalidMessage{msg_ty: Self::msg_ty()})),
            }
        )
    }
    fn construct_message(&self) -> Result<Message, Box<dyn Error>> {
        let id : u8 = match self { 
            VoxelEvent::Single(_) => VOXEL_EVENT_SINGLE_ID,
            VoxelEvent::Cuboid(_) => VOXEL_EVENT_CUBOID_ID,
        };
        let mut dat : Vec<u8> = Vec::new();
        dat.push(id);
        let mut event_data = self.serialize_data();
        dat.append(&mut event_data);
        Ok(Message{ty: Self::msg_ty(), data: dat } )
    }
}

#[test] 
pub fn apply_event() {
    let id = 1;
    let mut space = Space::new();
    space.load_or_gen_chunk(vpos!(0,0,0)).unwrap();
    let evt = VoxelEvent::Single( VoxelEventSingle{new_id: id, position: vpos!(1,1,1)});

    {
        evt.apply_to(&mut space).unwrap();
    }

    assert_eq!(space.get(vpos!(1,1,1)).unwrap(), id);

    let id = 2;
    let range : VoxelRange<TileCoord> = VoxelRange{lower: vpos!(0,0,0), upper: vpos!(8,8,8)}; 
    let evt = VoxelEvent::Cuboid( VoxelEventCuboid{new_id: id, cuboid: range} );
    
    {
        evt.apply_to(&mut space).unwrap();    
    }

    for pos in range {
        assert_eq!(space.get(pos).unwrap(), id); 
    }
}


#[allow(unused_must_use)]
#[test]
fn test_voxel_message_system() {
    let id = 1;
    let id2 = 2;

    let mut space = Space::new();
    space.load_or_gen_chunk(vpos!(0,0,0)).unwrap();
    
    let mut channel = EventBus::new();
    let mut receiver : TypedMsgReceiver<VoxelEvent> = channel.subscribe_typed();
    
    let range : VoxelRange<TileCoord> = VoxelRange{lower: vpos!(3,3,3), upper: vpos!(8,8,8)}; 

    let msg1 : VoxelEvent = VoxelEventSingle { new_id: id, position: vpos!(0,0,0)}.into();
    let msg2 : VoxelEvent = VoxelEventCuboid{new_id: id2, cuboid: range}.into();

    channel.broadcast(msg1.construct_message().unwrap());
    channel.broadcast(msg2.construct_message().unwrap());

    //This is a typed receiver that only gets "TestMessage" type messages. 
    //Even if we send a TestMessage2, this receiver should not get it.
    let mut count = 0;
    for _ in 0..10 {
        channel.process();
        while let Some(msg) = receiver.poll() {
            count += 1;
            msg.apply_to(&mut space).unwrap();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(count, 2);
    assert_eq!(space.get(vpos!(0,0,0)).unwrap(), id);
    assert_eq!(space.get(vpos!(5,5,5)).unwrap(), id2);
}*/