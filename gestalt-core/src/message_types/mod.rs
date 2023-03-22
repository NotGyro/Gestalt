use crate::common::identity::NodeIdentity;
use serde::{Deserialize, Serialize};

pub mod voxel;

// Client to server. Connect to the default entry point on the default world.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[netmsg(8, ClientToServer, ReliableOrdered)]
pub struct JoinDefaultEntry {
	pub display_name: String,
}

// Server to client. Let you know somebody joined!
#[derive(Serialize, Deserialize, Clone, Debug)]
#[netmsg(9, ServerToClient, ReliableOrdered)]
pub struct JoinAnnounce {
	pub display_name: String,
	pub identity: NodeIdentity,
}
