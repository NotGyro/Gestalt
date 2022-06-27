use std::net::SocketAddr;

use log::warn;
use serde::{Serialize, Deserialize, de::DeserializeOwned};

use crate::{message::{ChannelDomain, MessageWithDomain, RecvError}, common::{identity::NodeIdentity, growable_buffer::GrowableBuf}};

use super::{FullSessionName, MessageCounter};

pub const UNKNOWN_ROLE: u8 = 0;
pub const SERVER_ROLE: u8 = 1;
pub const CLIENT_ROLE: u8 = 2;

#[repr(u8)]
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum NetworkRole { 
    Unknown = UNKNOWN_ROLE,
    Server = SERVER_ROLE,
    Client = CLIENT_ROLE,
    //Later, roles will be added for things like CDNs, sharding, mirrors, backup-servers, etc.
}

impl From<u8> for NetworkRole {
    fn from(value: u8) -> Self {
        match value { 
            SERVER_ROLE => NetworkRole::Server,
            CLIENT_ROLE => NetworkRole::Client,
            _ => NetworkRole::Unknown,
        }
    }
}

impl From<NetworkRole> for u8 {
    fn from(role: NetworkRole) -> Self {
        match role {
            NetworkRole::Unknown => { 
                warn!("Serializing a NetworkRole::Unknown. This shouldn't happen - consider this a bug. Unknown Role's value is {}", UNKNOWN_ROLE);
                UNKNOWN_ROLE
            },
            NetworkRole::Server => SERVER_ROLE,
            NetworkRole::Client => CLIENT_ROLE,
        }
    }
}

#[repr(u8)]
#[derive(Debug, Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
pub enum SelfNetworkRole {
    Server = SERVER_ROLE,
    Client = CLIENT_ROLE,
    //Later, roles will be added for things like CDNs, sharding, mirrors, backup-servers, etc.
}

impl From<SelfNetworkRole> for u8 {
    fn from(role: SelfNetworkRole) -> Self {
        match role {
            SelfNetworkRole::Server => SERVER_ROLE,
            SelfNetworkRole::Client => CLIENT_ROLE,
        }
    }
}
impl From<SelfNetworkRole> for NetworkRole {
    fn from(role: SelfNetworkRole) -> Self {
        match role {
            SelfNetworkRole::Server => NetworkRole::Server,
            SelfNetworkRole::Client => NetworkRole::Client,
        }
    }
}

#[derive(Clone, Debug)]
pub enum MessageSidedness { 
    ClientToServer, 
    ServerToClient, 
    Common,
}

impl SelfNetworkRole { 
    /// On a node with this role, should we ingest a message with that sidedness?
    /// This is checked, even in release builds, as an extra security measure. 
    /// Certain net message IDs are just *not allowed* to be sent to servers.
    pub fn should_we_ingest(&self, message_sidedness: MessageSidedness) -> bool { 
        match message_sidedness {
            MessageSidedness::ClientToServer => match self {
                SelfNetworkRole::Server => true,
                SelfNetworkRole::Client => false,
            },
            MessageSidedness::ServerToClient => match self {
                SelfNetworkRole::Server => false,
                SelfNetworkRole::Client => true,
            },
            MessageSidedness::Common => true,
        }
    }
}

/// Describes what kind of ordering guarantees are made about a packet.
/// Directly inspired by (and currently maps to!) Laminar's reliability types.
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Hash)]
pub enum PacketGuarantees {
    /// No guarantees - it'll get there when it gets there.
    UnreliableUnordered,
    /// Not guaranteed that it'll get there, and if an older packet arrives after a newer one it will be discarded.
    UnreliableSequenced,
    /// Guaranteed it will get there (resend if we don't get ack), but no guarantees about the order.
    ReliableUnordered,
    /// It is guaranteed it will get there, and in the right order. Do not send next packet before getting ack.
    /// TCP-like.
    ReliableOrdered,
    /// Guaranteed it will get there (resend if we don't get ack),
    /// and if an older packet arrives after a newer one it will be discarded.
    ReliableSequenced,
}

pub type StreamId = u8;

/// Which "stream" is this on?
/// A stream in this context must be a u8-identified separate channel of packets
#[derive(Debug, Copy, Clone, PartialOrd, PartialEq, Hash)]
pub enum StreamSelector {
    Any,
    Specific(StreamId),
}

impl From<Option<StreamId>> for StreamSelector {
    fn from(value: Option<StreamId>) -> Self {
        match value { 
            None => StreamSelector::Any,
            Some(val) => StreamSelector::Specific(val),
        }
    }
}
impl From<StreamSelector> for Option<StreamId> {
    fn from(value: StreamSelector) -> Self {
        match value {
            StreamSelector::Any => None,
            StreamSelector::Specific(val) => Some(val),
        }
    }
}



/// Decoded top-level envelope containing the session id, the counter, and the ciphertext, to send to the session layer.
#[derive(Debug, Clone)]
pub struct OuterEnvelope {
    pub session_id: FullSessionName,
    /// Counter, monotonically increasing, encoded as 4 little endian bytes on the wire. 
    pub counter: MessageCounter,
    /// Noise-protocol-encrypted ciphertext, which decrypts to a Laminar packet containing EncodedNetMsg bytes.
    pub ciphertext: Vec<u8>,
}

/// What type of packet are we sending/receiving? Should 1-to-1 correspond with a type of NetMessage.
/// On the wire, this will be Vu64's variable-length encoding.
pub type NetMsgId = u32;

impl ChannelDomain for NetMsgId {}

pub const DISCONNECT_RESERVED: NetMsgId = 0;

/// Information required to interconvert between raw packets and structured Rust types.
#[derive(Debug, Clone)]
pub struct NetMsgType {
    pub id: NetMsgId,
    pub name: &'static str,
    pub sidedness: MessageSidedness,
    pub guarantees: PacketGuarantees,
    pub stream: StreamSelector,
}

/// A NetMsg coming in off the wire 
#[derive(Debug, Clone)]
pub struct InboundNetMsg {
    pub peer_identity: NodeIdentity, 
    pub message_type_id: NetMsgId,
    // Our MsgPack-encoded actual NetMsg.
    pub payload: Vec<u8>,
}

/*
impl MessageWithDomain<NodeIdentity> for InboundNetMsg {
    fn get_domain(&self) -> &NodeIdentity {
        &self.peer_identity
    }
}

impl MessageWithDomain<NetMsgId> for InboundNetMsg {
    fn get_domain(&self) -> &NetMsgId {
        &self.message_type_id
    }
}*/

pub type NetMsgDomain = NetMsgId;

impl<'a> MessageWithDomain<NetMsgDomain> for InboundNetMsg {
    fn get_domain(&self) -> &NetMsgDomain {
        &self.message_type_id
    }
}

/// A NetMsg to send to one of our currently-connected peers. 
#[derive(Debug, Clone)]
pub struct OutboundNetMsg {
    pub message_type: NetMsgType,
    // Our MsgPack-encoded actual NetMsg.
    pub payload: Vec<u8>,
    pub destination: NodeIdentity,
}

#[derive(thiserror::Error, Debug)]
pub enum NetMsgDecodeErr {
    #[error("Attempted to decode a NetMessage into type {0}, but it was a NetMessage of type {1}")]
    WrongType(NetMsgId, NetMsgId),
    #[error("Could not decode a NetMessage: {0:?}")]
    CouldNotDecode(#[from] rmp_serde::decode::Error),
    #[error("Could not send an inbound NetMsg over to the appropriate part of the program.")]
    SendToChannel,
    #[error("Could get an inbound NetMsg off of a channel from the network subsystem.")]
    ReceiveFromChannel,
}

#[derive(thiserror::Error, Debug)]
pub enum NetMsgRecvError {
    #[error("Attempted to decode a NetMessage into type {0} (which is {1}), but it was a NetMessage of type {2}")]
    WrongType(NetMsgId, &'static str, NetMsgId),
    #[error("Could not decode a NetMessage: {0:?}")]
    CouldNotDecode(#[from] rmp_serde::decode::Error),
    #[error("Could not get an inbound NetMsg off of a channel from the network subsystem: {0:?}")]
    ReceiveFromChannel(#[from] RecvError),
}

pub type NetMsgBroadcast = Vec<(InboundNetMsg, NodeIdentity)>;

//Packet with no destination.
#[derive(Clone, Debug)]
pub struct PacketIntermediary { 
    pub guarantees: PacketGuarantees, 
    pub stream: StreamSelector, 
    pub payload: Vec<u8>,
}

impl PacketIntermediary { 
    pub fn make_full_packet(self, send_to: SocketAddr) -> laminar::Packet { 
        use laminar::Packet;
        // Branch on our message properties to figure out what kind of packet to construct.
        match self.guarantees {
            PacketGuarantees::UnreliableUnordered => {
                // Unordered packets have no concept of a "stream"
                Packet::unreliable(send_to, self.payload)
            },
            PacketGuarantees::UnreliableSequenced => {
                match self.stream {
                    StreamSelector::Any => Packet::unreliable_sequenced(send_to, self.payload, None),
                    StreamSelector::Specific(id) => Packet::unreliable_sequenced(send_to, self.payload, Some(id)),
                }
            },
            PacketGuarantees::ReliableUnordered => {
                // Unordered packets have no concept of a "stream"
                Packet::reliable_unordered(send_to, self.payload)
            },
            PacketGuarantees::ReliableOrdered => {
                match self.stream {
                    StreamSelector::Any => Packet::reliable_ordered(send_to, self.payload, None),
                    StreamSelector::Specific(id) => Packet::reliable_ordered(send_to, self.payload, Some(id)),
                }
            },
            PacketGuarantees::ReliableSequenced => {
                match self.stream {
                    StreamSelector::Any => Packet::reliable_sequenced(send_to, self.payload, None),
                    StreamSelector::Specific(id) => Packet::reliable_sequenced(send_to, self.payload, Some(id)),
                }
            },
        }
    }
}

pub const PACKET_ENCODE_MAX: usize = 1024 * 1024 * 512;
pub const RECEIVED_PACKET_BROADCASTER_MAX: usize = 2048;

/// Any type which can be encoded as a NetMessage to be sent out over the wire.
pub trait NetMsg: Serialize + DeserializeOwned + Clone {
    fn net_msg_id() -> NetMsgId;
    fn net_msg_guarantees() -> PacketGuarantees;
    fn net_msg_stream() -> StreamSelector;
    /// Used with the `stringify!()` macro
    fn net_msg_name() -> &'static str;
    fn net_msg_sidedness() -> MessageSidedness;
    fn net_msg_type() -> NetMsgType {
        NetMsgType {
            id: Self::net_msg_id(), 
            name: Self::net_msg_name(), 
            sidedness: Self::net_msg_sidedness(),
            guarantees: Self::net_msg_guarantees(), 
            stream: Self::net_msg_stream(),
        }
    }
    
    fn construct_packet(&self) -> Result<PacketIntermediary, Box<dyn std::error::Error>> {
        // Start by writing our tag.
        let encode_start: Vec<u8> = vu64::encode(Self::net_msg_id() as u64).as_ref().to_vec();
        // Write our data.
        let mut buffer = GrowableBuf::new(encode_start, PACKET_ENCODE_MAX);
        rmp_serde::encode::write(&mut buffer, self)?;
        let encoded = buffer.into_inner();

        Ok(PacketIntermediary{ guarantees: Self::net_msg_guarantees(), stream: Self::net_msg_stream(), payload: encoded})
    }
    
    fn decode_from(message: InboundNetMsg) -> Result<(Self, NodeIdentity), NetMsgRecvError> {
        if Self::net_msg_id() != message.message_type_id {
            Err(NetMsgRecvError::WrongType(Self::net_msg_id(), Self::net_msg_name(), message.message_type_id))
        }
        else {
            let InboundNetMsg{peer_identity, message_type_id: _, payload } = message;
            let payload: Self = rmp_serde::from_read(&payload[..])?;
            Ok((payload, peer_identity))
        }
    }
}

macro_rules! impl_netmsg {
    ($message:ident, $id:expr, $sidedness:ident, $guarantee:ident) => {
        impl crate::net::netmsg::NetMsg for $message {
            #[inline(always)]
            fn net_msg_id() -> crate::net::netmsg::NetMsgId { $id }
            #[inline(always)]
            fn net_msg_guarantees() -> crate::net::netmsg::PacketGuarantees { crate::net::netmsg::PacketGuarantees::$guarantee }
            #[inline(always)]
            fn net_msg_stream() -> crate::net::netmsg::StreamSelector { crate::net::netmsg::StreamSelector::Any }
            #[inline(always)]
            fn net_msg_name() -> &'static str { stringify!($message) }
            #[inline(always)]
            fn net_msg_sidedness() -> crate::net::netmsg::MessageSidedness { 
                crate::net::netmsg::MessageSidedness::$sidedness
            }
        }
        impl Into<crate::net::netmsg::PacketIntermediary> for &$message {
            fn into(self) -> crate::net::netmsg::PacketIntermediary {
                use crate::net::netmsg::NetMsg;
                self.construct_packet().unwrap()
            }
        }
    };
    ($message:ident, $id:expr, $sidedness:ident, $guarantee:ident, $stream:expr) => {
        impl crate::net::netmsg::NetMsg for $message {
            #[inline(always)]
            fn net_msg_id() -> crate::net::netmsg::NetMsgId { $id }
            #[inline(always)]
            fn net_msg_guarantees() -> crate::net::netmsg::PacketGuarantees { crate::net::netmsg::PacketGuarantees::$guarantee }
            #[inline(always)]
            fn net_msg_stream() -> crate::net::netmsg::StreamSelector { crate::net::netmsg::StreamSelector::Specific($stream) }
            #[inline(always)]
            fn net_msg_name() -> &'static str { stringify!($message) }
            #[inline(always)]
            fn net_msg_sidedness() -> crate::net::netmsg::MessageSidedness { 
                crate::net::netmsg::MessageSidedness::$sidedness
            }
        }
        impl Into<crate::net::netmsg::PacketIntermediary> for &$message {
            fn into(self) -> crate::net::netmsg::PacketIntermediary {
                use crate::net::netmsg::NetMsg;
                self.construct_packet().unwrap()
            }
        }
    };
}
