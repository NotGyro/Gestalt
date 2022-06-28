use std::collections::HashMap;
use toolbelt::once::InitOnce;
use crate::net::netmsg::NetMsg;

pub(crate) static NETMSG_LOOKUP_TABLE: InitOnce<HashMap<crate::net::netmsg::NetMsgId, crate::net::netmsg::NetMsgType>> = InitOnce::uninitialized();

pub(crate) fn initialize_netmsg_lookup() {
    let mut msgs = HashMap::new();
    
    msgs.insert(crate::message_types::JoinDefaultEntry::net_msg_id(), crate::message_types::JoinDefaultEntry::net_msg_type());
    msgs.insert(crate::message_types::JoinAnnounce::net_msg_id(), crate::message_types::JoinAnnounce::net_msg_type());
    msgs.insert(crate::message_types::voxel::VoxelChangeRequest::net_msg_id(), crate::message_types::voxel::VoxelChangeRequest::net_msg_type());
    msgs.insert(crate::message_types::voxel::VoxelChangeAnnounce::net_msg_id(), crate::message_types::voxel::VoxelChangeAnnounce::net_msg_type());
    msgs.insert(crate::net::DisconnectMsg::net_msg_id(), crate::net::DisconnectMsg::net_msg_type());
    #[cfg(test)]
    msgs.insert(crate::net::test::TestNetMsg::net_msg_id(), crate::net::test::TestNetMsg::net_msg_type());

    NETMSG_LOOKUP_TABLE.initialize(msgs).unwrap();
}
