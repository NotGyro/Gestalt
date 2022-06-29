use std::collections::HashMap;
use toolbelt::once::InitOnce;
use crate::net::netmsg::{NetMsg, NetMsgId, NetMsgType};

static NETMSG_LOOKUP_TABLE: InitOnce<HashMap<NetMsgId, NetMsgType>> = InitOnce::uninitialized();

pub(crate) fn get_netmsg_table() -> &'static HashMap<NetMsgId, NetMsgType> {
    //If it's already there, just get it.
    if let Some(out) = NETMSG_LOOKUP_TABLE.try_get() { 
        return out;
    }
    //If not, initialize it.
    NETMSG_LOOKUP_TABLE.get_or_init(|| {
        let mut msgs = HashMap::new();
        
        msgs.insert(crate::message_types::JoinDefaultEntry::net_msg_id(), crate::message_types::JoinDefaultEntry::net_msg_type());
        msgs.insert(crate::message_types::JoinAnnounce::net_msg_id(), crate::message_types::JoinAnnounce::net_msg_type());
        msgs.insert(crate::message_types::voxel::VoxelChangeRequest::net_msg_id(), crate::message_types::voxel::VoxelChangeRequest::net_msg_type());
        msgs.insert(crate::message_types::voxel::VoxelChangeAnnounce::net_msg_id(), crate::message_types::voxel::VoxelChangeAnnounce::net_msg_type());
        msgs.insert(crate::net::DisconnectMsg::net_msg_id(), crate::net::DisconnectMsg::net_msg_type());
        #[cfg(test)] {
                msgs.insert(crate::net::test::TestNetMsg::net_msg_id(), crate::net::test::TestNetMsg::net_msg_type());
        }
        msgs
    }).unwrap()
}
