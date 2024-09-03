use gestalt_proc_macros::netmsg;
use serde::{Deserialize, Serialize};

use crate::{common::voxelmath::VoxelPos, world::TileId};

/// Usually client-to-server.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[netmsg(40, ClientToServer, ReliableOrdered)]
pub struct VoxelChangeRequest {
	pub pos: VoxelPos<i32>,
	pub new_tile: TileId,
}

/// Used as both an in-engine message and by servers, to tell clients authoritatively that a voxel has changed.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[netmsg(41, ServerToClient, ReliableOrdered)]
pub struct VoxelChangeAnnounce {
	pub pos: VoxelPos<i32>,
	pub new_tile: TileId,
}

impl Into<VoxelChangeAnnounce> for VoxelChangeRequest {
	fn into(self) -> VoxelChangeAnnounce {
		VoxelChangeAnnounce {
			pos: self.pos,
			new_tile: self.new_tile,
		}
	}
}
