use serde::{Serialize, Deserialize};
use crate::common::identity::NodeIdentity;
use crate::{common::voxelmath::VoxelPos, world::TileId};
use crate::net::{NetMsg, PacketGuarantees, StreamSelector};

pub mod voxel; 

// Client to server. Connect to the default entry point on the default world.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JoinDefaultEntry {
    pub display_name: String,
}


// Server to client. Let you know somebody joined! 
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JoinAnnounce {
    pub display_name: String,
    pub identity: NodeIdentity,
}

impl_netmsg!(JoinDefaultEntry, 8, ReliableOrdered);
impl_netmsg!(JoinAnnounce, 9, ReliableOrdered);