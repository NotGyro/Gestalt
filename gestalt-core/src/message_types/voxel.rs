use serde::{Serialize, Deserialize};

use crate::{common::voxelmath::VoxelPos, world::TileId};

use crate::net::{NetMsg, PacketGuarantees, StreamSelector};

/// Usually client-to-server. 
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VoxelChangeRequest {
    pub pos: VoxelPos<i32>,
    pub new_tile: TileId,
}

/// Used as both an in-engine message and by servers, to tell clients authoritatively that a voxel has changed.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct VoxelChangeAnnounce {
    pub pos: VoxelPos<i32>,
    pub new_tile: TileId,
}

impl_netmsg!(VoxelChangeRequest, 40, ReliableOrdered);
impl_netmsg!(VoxelChangeAnnounce, 41, ReliableOrdered);

impl Into<VoxelChangeAnnounce> for VoxelChangeRequest {
    fn into(self) -> VoxelChangeAnnounce {
        VoxelChangeAnnounce { 
            pos: self.pos,
            new_tile: self.new_tile,
        }
    }
}