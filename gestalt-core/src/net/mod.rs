use std::collections::VecDeque;
use std::fs;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use futures::FutureExt;
use hashbrown::HashMap;
use laminar::Connection;
use laminar::ConnectionMessenger;
use laminar::VirtualConnection;
use log::error;
use log::info;
use log::warn;
use serde::Deserialize;
use serde::{Serialize, de::DeserializeOwned};

use snow::StatelessTransportState;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use crate::common::Version;
use crate::common::growable_buffer::GrowableBuf;
use crate::common::identity::IdentityKeyPair;
use crate::common::identity::NodeIdentity;

use self::preprotocol::NetworkRole;

pub const PREPROTCOL_PORT: u16 = 54134;
pub const GESTALT_PORT: u16 = 54134;
//use tokio::sync::mpsc::error::TryRecvError; 

//use crossbeam_channel::{bounded, Sender, Receiver, TryRecvError};

/*use std::boxed::Box;
use std::error::Error;
use std::marker::PhantomData;
use std::net::{IpAddr, SocketAddr};
use std::result::Result;
use std::thread;

use hashbrown::{HashSet, HashMap};
use log::error;
use log::info;
use log::warn;
use parking_lot::Mutex;

use laminar::{SocketEvent, Socket, Packet};

use crossbeam_channel::{bounded, Sender, Receiver, TryRecvError};

use serde::{Serialize, Deserialize, de::DeserializeOwned};

use lazy_static::lazy_static;

use crate::common::Version;
use crate::common::identity::IdentityKeyPair;
use crate::common::identity::NodeIdentity;*/

pub mod handshake;
pub mod net_channel;
pub mod preprotocol;

pub const SESSION_ID_LEN: usize = 4;
pub type SessionId = [u8; SESSION_ID_LEN];

pub type MessageCounter = u32;

/// Represents a client who has completed a handshake in the pre-protocol and will now be moving over to the game protocol proper
#[derive(Debug)]
pub struct SuccessfulConnect {
    pub session_id: SessionId,
    pub peer_identity: NodeIdentity,
    pub peer_address: SocketAddr,
    pub peer_role: NetworkRole, 
    pub peer_engine_version: Version,
    pub transport_cryptography: StatelessTransportState,
    pub transport_counter: u32,
}

impl SuccessfulConnect { 
    pub fn get_full_session_name(&self) -> FullSessionName { 
        FullSessionName {
            peer_address: self.peer_address,
            session_id: self.session_id,
        }
    }
}

/// Which directory holds temporary network protocol data? 
/// I.e. Noise protocol keys, cached knowledge of "this identity is at this IP," etc. 
pub fn protocol_store_dir() -> PathBuf { 
    const PROTOCOL_STORE_DIR: &str = "protocol/"; 
    let path = PathBuf::from(PROTOCOL_STORE_DIR);
    if !path.exists() { 
        fs::create_dir(&path).unwrap();
    }
    path
}

/// Runtime information specifying what kind of connection we are looking at.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ConnectionRole {
    /// We are the server and we are connected to a client.
    ServerToClient,
    /// We are the client and we are connected to a server. 
    ClientToServer,
}

// A chunk has to be requested by a client (or peer server) before it is sent. So, a typical flow would go like this:
// 1. Client: My revision number on chunk (15, -8, 24) is 732. Can you give me the new stuff if there is any?
// 2. Server: Mine is 738, here is a buffer of 6 new voxel event logs to get you up to date.

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
// One Tokio task for polling the socket for inbound messages and routing them to and sending on channels per peer, and for polling from channels to send over the network. 
// --
// n Tokio tasks per peer for polling inbound channel from the above UDP socket task, decrypting, Laminar logic per connection, etc, and then pushing to the socket task.
// These also poll off of channels sending user messages for outbound messages to turn into Laminar messages and then encrypt them, pushing to the socket task. 
// This is where ConnectionManager and the Noise protocol transport cryptography lives, along with its cryptographic counter.
// --
// Somewhere we associate inbound messages with a NodeIdentity and correspond outgoing NetMsg's with the right session per NodeIdentity.
// Also, message serialziation has to happen at some point. 
// Perhaps the middle layer gets channels moved to the top layer and channels moved to the bottom layer, at the same time, upon successful connection start?
// Do I need a "bottom layer"? 

// Tokio channel "blocking_send()" can be used outside of a Tokio runtime to send a message inside a Tokio runtime. 
// blocking_recv() also can be used to get messages from inside a Tokio runtime and pull them inside a Tokio runtime. 
// try_recv() also appears to work outside of an async context! 
// And sending on an unbounded channel never requires any kind of waiting - send() on an unbounded channel is sync... 
// This has lots of implications! Including and especially ways we can make the message bus work, low low latency.


#[derive(Clone, Debug)]
pub struct NetConfig { 
    //How often should we try to resend dropped packets / send heartbeats?
    pub update_interval: Duration,
    //Drop connection after this long with no message.
    pub timeout: Duration,
    //Configuration for Laminar
    pub laminar_config: laminar::Config,
}
impl Default for NetConfig {
    fn default() -> Self {
        Self { 
            update_interval: Duration::from_millis(50),
            timeout: Duration::from_secs(3),
            laminar_config: Default::default(),
        }
    }
}

/// Thin wrapper used to pretend, from the perspective of Laminar, 
/// that Noise protocol encryption and async UDP are a transparent synchronous UDP socket.
#[derive(Default)]
struct TransportWrapper {
    pub laminar_config: laminar::Config,
    // Packets to send
    pub outbox: VecDeque<(SocketAddr, Vec<u8>)>, 
    // Packets received
    pub inbox: VecDeque<laminar::SocketEvent>,

}

impl laminar::ConnectionMessenger<laminar::SocketEvent> for TransportWrapper {
    fn config(&self) -> &laminar::Config {
        &self.laminar_config
    }

    #[allow(unused_variables)]
    fn send_event(&mut self, address: &SocketAddr, event: laminar::SocketEvent) {
        // This is for this node recieving messages from a remote peer, pushing them along to the rest of the program. 
        self.inbox.push_back(event);
    }

    fn send_packet(&mut self, address: &SocketAddr, payload: &[u8]) {
        //This is for outgoing packets.
        self.outbox.push_back((*address, payload.to_vec()));
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LaminarWrapperError {
    #[error("Peer {0:?} disconnected.")]
    Disconnect(SocketAddr),
}

pub type LaminarConfig = laminar::Config;

/// Partial reimplementation of a Laminar::ConnectionManager with somewhat different logic since we're using async here, and there will be one of these per peer per node.
pub struct LaminarConnectionManager {
    peer_address: SocketAddr,
    connection_state: VirtualConnection,
    pub(in crate::net) messenger: TransportWrapper,
}

impl LaminarConnectionManager {

    pub fn new(peer_address: SocketAddr, laminar_config: &LaminarConfig, time: Instant) -> Self {
        let mut messenger = TransportWrapper {
            laminar_config: laminar_config.clone(),
            outbox: VecDeque::default(),
            inbox: VecDeque::default(),
        };
        let connection_state = VirtualConnection::create_connection(&mut messenger, peer_address, time);

        LaminarConnectionManager {
            peer_address,
            connection_state,
            messenger,
        }
    }

    /// Ingests a batch of packets coming off the wire.
    pub fn process_inbound<T: IntoIterator< Item: AsRef<[u8]> >>(&mut self, inbound_messages: T, time: Instant) -> Result<(), LaminarWrapperError> {
        info!("Processing inbound packets.");
        let mut at_least_one = false; 
        let messenger = &mut self.messenger;
        for payload in inbound_messages.into_iter() {
            at_least_one = true;
            let was_est = self.connection_state.is_established();
            //Processing inbound
            self.connection_state.process_packet(messenger, payload.as_ref(), time);
            if !was_est && self.connection_state.is_established() {
                info!("Connection established with {:?}", self.peer_address);
            }
        }
        if at_least_one {
            self.connection_state.last_heard = time.clone(); 
        }

        self.connection_state.update(messenger, time);
        
        match self.connection_state.should_drop(messenger, time) { 
            false => Ok(()),
            true => {
                info!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", self.connection_state.packets_in_flight(), self.connection_state.last_heard(Instant::now()), self.connection_state.is_established()); 
                Err(LaminarWrapperError::Disconnect(self.peer_address))
            }
        }
    }
    pub fn process_update(&mut self, time: Instant) -> Result<(), LaminarWrapperError> {
        let messenger = &mut self.messenger;
        self.connection_state.update(messenger, time);
        
        match self.connection_state.should_drop(messenger, time) {
            false => Ok(()),
            true => {
                info!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", self.connection_state.packets_in_flight(), self.connection_state.last_heard(Instant::now()), self.connection_state.is_established()); 
                Err(LaminarWrapperError::Disconnect(self.peer_address))
            }
        }
    }
    /// Adds Laminar connection logic to messages that we are sending. 
    pub fn process_outbound<T: IntoIterator< Item=laminar::Packet >>(&mut self, outbound_messages: T, time: Instant)  -> Result<(), LaminarWrapperError> { 
        // Return before attempting to send. 
        if self.connection_state.should_drop(&mut self.messenger, time) { 
            return Err(LaminarWrapperError::Disconnect(self.peer_address));
        }
        
        // To send:
        for packet in outbound_messages.into_iter() {
            self.connection_state.process_event(&mut self.messenger, packet, time);
        }
        self.connection_state.update(&mut self.messenger, time);

        // Check again!
        match self.connection_state.should_drop(&mut self.messenger, time) { 
            false => Ok(()),
            true => {
                info!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", self.connection_state.packets_in_flight(), self.connection_state.last_heard(Instant::now()), self.connection_state.is_established()); 
                Err(LaminarWrapperError::Disconnect(self.peer_address))
            }
        }
    }
    // Take all of the messages to send - used by the network system to poll this object for messages to send. 
    pub fn empty_outbox<T: FromIterator<(SocketAddr, Vec<u8>)>>(&mut self) -> T { 
        self.messenger.outbox.drain(0..).collect()
    }
    pub fn empty_inbox<T: FromIterator<laminar::SocketEvent>>(&mut self) -> T { 
        self.messenger.inbox.drain(0..).collect()
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

pub const DISCONNECT_RESERVED: NetMsgId = 0;

/// Information required to interconvert between raw packets and structured Rust types.
#[derive(Debug, Copy, Clone)]
pub struct NetMsgType {
    pub id: NetMsgId,
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
    // This used to have a `pub source: NodeIdentity,` line, but these are implicitly per-session and that is validated at the session layer. 
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
    #[error("Could not get an inbound NetMsg of type {0} off of a channel from the network subsystem.")]
    ReceiveFromChannel(&'static str),
}

pub type NetMsgSender = tokio::sync::broadcast::Sender<Vec<InboundNetMsg>>; 
pub type NetMsgReceiver = tokio::sync::broadcast::Receiver<Vec<InboundNetMsg>>;

pub type NetMsgBroadcast = Vec<(InboundNetMsg, NodeIdentity)>;

pub struct TypedNetMsgReceiver<T> { 
    pub inner: NetMsgReceiver,
    _t: PhantomData<T>,
}
impl<T: NetMsg> TypedNetMsgReceiver<T> { 
    pub fn new(inner: NetMsgReceiver) -> Self { 
        TypedNetMsgReceiver { 
            inner, 
            _t: PhantomData::default(),
        }
    }
    pub fn subscribe_on(sender: NetMsgSender) -> Self { 
        TypedNetMsgReceiver { 
            inner: sender.subscribe(), 
            _t: PhantomData::default(),
        }
    }
    pub fn len(&self) -> usize { 
        self.inner.len()
    }
    pub fn is_empty(&self) -> bool { 
        self.inner.is_empty()
    }
    pub(crate) fn decode(inbound: Vec<InboundNetMsg>) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> {
        let mut output = Vec::with_capacity(inbound.len());
        for message in inbound { 
            if T::net_msg_id() != message.message_type_id { 
                return Err(NetMsgRecvError::WrongType(T::net_msg_id(), T::net_msg_name(), message.message_type_id));
            }
            else {
                let InboundNetMsg{peer_identity, message_type_id: _, payload } = message;
                let payload: T = rmp_serde::from_read(&payload[..])?;
                output.push((peer_identity, payload));
            }
        }
        Ok(output)
    }

    pub fn resubscribe(&self) -> Self { 
        TypedNetMsgReceiver { 
            inner: self.inner.resubscribe(), 
            _t: PhantomData::default(),
        }
    }
    pub async fn recv(&mut self) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> { 
        Self::decode(
            self.inner.recv().await
                .map_err(|_e| NetMsgRecvError::ReceiveFromChannel(T::net_msg_name()) )?
        )
    }
    pub fn try_recv(&mut self) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> { 
        match self.inner.try_recv() {
            Ok(buf) => Self::decode(buf),
            Err(e) => match e {
                tokio::sync::broadcast::error::TryRecvError::Empty => Ok(Vec::new()) /* Return an empty vector - nothing went wrong, our mailbox is just empty.*/,
                tokio::sync::broadcast::error::TryRecvError::Closed => Err(NetMsgRecvError::ReceiveFromChannel(T::net_msg_name())),
                tokio::sync::broadcast::error::TryRecvError::Lagged(_x) => Err(NetMsgRecvError::ReceiveFromChannel(T::net_msg_name())),
            },
        }
    }
}

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
    fn net_msg_type() -> NetMsgType {
        NetMsgType {
            id: Self::net_msg_id(), 
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
}

macro_rules! impl_netmsg {
    ($message:ident, $id:expr, $guarantee:ident) => {
        impl NetMsg for $message {
            #[inline(always)]
            fn net_msg_id() -> u32 { $id }
            #[inline(always)]
            fn net_msg_guarantees() -> PacketGuarantees { PacketGuarantees::$guarantee }
            #[inline(always)]
            fn net_msg_stream() -> StreamSelector { StreamSelector::Any }
            #[inline(always)]
            fn net_msg_name() -> &'static str { stringify!($message) }
        }
    };
    ($message:ident, $id:expr, $guarantee:ident, $stream:expr) => {
        impl NetMsg for $message {
            #[inline(always)]
            fn net_msg_id() -> u32 { $id }
            #[inline(always)]
            fn net_msg_guarantees() -> PacketGuarantees { PacketGuarantees::$guarantee }
            #[inline(always)]
            fn net_msg_stream() -> StreamSelector { StreamSelector::Specific($stream) }
            #[inline(always)]
            fn net_msg_name() -> &'static str { stringify!($message) }
        }
    };
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Hash, Eq)]
pub struct FullSessionName { 
    pub peer_address: SocketAddr, 
    pub session_id: SessionId,
}
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Hash, Eq)]
pub struct PartialSessionName { 
    pub peer_address: IpAddr, 
    pub session_id: SessionId,
}

impl FullSessionName { 
    pub fn get_partial(&self) -> PartialSessionName { 
        PartialSessionName { 
            peer_address: self.peer_address.ip(), 
            session_id: self.session_id.clone(),
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum SessionLayerError {
    #[error("Reliable-UDP error: {0:?}")]
    LaminarWrapper(#[from] LaminarWrapperError),
    #[error("Cryptographic error decrypting/encrypting packet: {0:?}")]
    CryptographicError(#[from] snow::Error),
    #[error("A packet was given to the wrong session state to decrypt! Our session is {0} and the session ID on the packet is {1}")]
    WrongChannel(String, String),
    #[error("Laminar asked to send a packet to {0:?} but this session is a communicating with {1:?}")]
    WrongIpSend(SocketAddr, SocketAddr),
    #[error("Mutliple errors were detected while handling inbound packets: {0:?}")]
    ErrorBatch(Vec<SessionLayerError>),
    #[error("Could not send OuterEnvelope to packet layer: {0:?}")]
    SendChannelError(#[from] tokio::sync::mpsc::error::SendError<Vec<OuterEnvelope>>),
    #[error("Could not send decoded message to the rest of the engine: {0:?}")]
    SendBroadcastError(#[from] tokio::sync::broadcast::error::SendError<Vec<InboundNetMsg>>),
    #[error("Connection with {0:?} timed out.")]
    LaminarTimeout(SocketAddr),
    #[error("Peer {0:?} disconnected.")]
    LaminarDisconnect(SocketAddr),
    #[error("Peer {0:?} sent a Laminar \"connect\" message after the session was already started!")]
    ConnectAfterStarted(SocketAddr),
    #[error("Variable-length integer could not be decoded: {0:?}")]
    VarIntError(#[from] vu64::Error),
    #[error("A NetMessage of type {0} has been receved from {1}, but we have no handlers associated with that type of message. \n It's possible this peer is using a newer version of Gestalt.")]
    NoHandler(NetMsgId, String),
}

pub type PushSender = mpsc::UnboundedSender<Vec<OuterEnvelope>>;
pub type PushReceiver = mpsc::UnboundedReceiver<Vec<OuterEnvelope>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DisconnectMsg {}

impl_netmsg!(DisconnectMsg, DISCONNECT_RESERVED, ReliableUnordered);

/// One per session, handles both cryptography and Laminar reliable-UDP logic.
pub struct Session {
    /// Handles reliability-over-UDP.
    pub laminar: LaminarConnectionManager,
    pub local_identity: IdentityKeyPair,
    pub peer_identity: NodeIdentity,
    pub peer_address: SocketAddr, 
    
    pub session_id: SessionId,
    /// Counter we put on outgoing `OuterEnvelope`s, should increase monotonically.
    pub local_counter: u32,
    pub transport_cryptography: snow::StatelessTransportState, 
    
    /// Channel the Session uses to send packets to the UDP socket
    push_channel: PushSender,

    /// Channels to distribute out inbound packets to the rest of the engine on. 
    received_channels: HashMap<NetMsgId, NetMsgSender>,

    pub disconnect_deliberate: bool,
}

impl Session {
    pub fn new(local_identity: IdentityKeyPair, peer_address: SocketAddr, connection: SuccessfulConnect, laminar_config: LaminarConfig, 
                push_channel: PushSender, received_message_channels: HashMap<NetMsgId, NetMsgSender>, time: Instant) -> Self {
        Session {
            laminar: LaminarConnectionManager::new(connection.peer_address, &laminar_config, time),
            local_identity,
            peer_identity: connection.peer_identity,
            peer_address,
            session_id: connection.session_id,
            local_counter: connection.transport_counter,
            transport_cryptography: connection.transport_cryptography,
            push_channel,
            received_channels: received_message_channels,
            disconnect_deliberate: false,
        }
    }
    pub fn get_session_name(&self) -> FullSessionName { 
        FullSessionName {
            peer_address: self.peer_address.clone(),
            session_id: self.session_id.clone(),
        }
    }

    /// Encrypts the raw byte blobs produced by Laminar and encloses them in an OuterEnvelope,  
    fn encrypt_packet<T: AsRef<[u8]>>(&mut self, plaintext: T) -> Result<OuterEnvelope, SessionLayerError> {
        self.local_counter += 1;
        let mut buffer = vec![0u8; ( (plaintext.as_ref().len() as usize) * 3) + 64 ];
        let len_written = self.transport_cryptography.write_message(self.local_counter as u64, plaintext.as_ref(), &mut buffer)?;
        buffer.truncate(len_written);
        let full_session_name = self.get_session_name();
        Ok(
            OuterEnvelope {
                session_id: full_session_name,
                counter: self.local_counter,
                ciphertext: buffer,
            }
        )
    }

    /// Called inside process_inbound()
    fn decrypt_outer_envelope(&mut self, envelope: OuterEnvelope) -> Result<Vec<u8>, SessionLayerError> { 
        let OuterEnvelope{ session_id: _session_id, counter, ciphertext } = envelope;

        let mut buf = vec![0u8; (ciphertext.len() * 3)/2];
        let len_read = self.transport_cryptography.read_message(counter as u64, &ciphertext, &mut buf)?;
        buf.truncate(len_read);
        Ok(buf)
    }

    /// Ingests a batch of packets coming off the wire.
    pub async fn ingest_packets<T: IntoIterator< Item=OuterEnvelope >>(&mut self, inbound_messages: T, time: Instant) -> Vec<SessionLayerError> {
        let mut errors: Vec<SessionLayerError> = Vec::default();

        let mut batch: Vec<Vec<u8>> = Vec::default();
        for envelope in inbound_messages.into_iter() {
            match self.decrypt_outer_envelope(envelope) {
                Ok(packet_contents) => batch.push(packet_contents),
                Err(e) => errors.push(e),
            }
        }

        if batch.len() > 0 { 
            self.laminar.connection_state.last_heard = time;
        }

        match self.laminar.process_inbound(batch, time) {
            Ok(_) => {},
            Err(e) => errors.push(e.into()),
        }

        //Packets to send to the rest of the Gestalt application, having been decoded.
        let mut processed_packets: Vec<laminar::SocketEvent> = self.laminar.empty_inbox();

        //Are any of these types of Laminar packets which should close the channel? 
        let drop_packets: Vec<laminar::SocketEvent> = processed_packets.drain_filter(|packet| { 
            match packet {
                laminar::SocketEvent::Packet(_) => false,
                _ => true,
            }
        }).collect();
        if !drop_packets.is_empty() { 
            match drop_packets.first().unwrap() {
                laminar::SocketEvent::Timeout(addr) => errors.push(SessionLayerError::LaminarTimeout(addr.clone())),
                laminar::SocketEvent::Disconnect(addr) => errors.push(SessionLayerError::LaminarDisconnect(addr.clone())),
                laminar::SocketEvent::Connect(_addr) => {
                    self.laminar.connection_state.last_heard = time;
                },
                laminar::SocketEvent::Packet(_) => unreachable!(), 
            }
        }
        //Now that we've handled those, convert.
        //Batch them according to ID.
        let mut finished_packets: HashMap<NetMsgId, Vec<InboundNetMsg>> = HashMap::new();
        for evt in processed_packets {
            match evt {
                laminar::SocketEvent::Packet(pkt) => {
                    // How long is our varint?
                    let message_type_first_byte = pkt.payload()[0];
                    let message_type_len = vu64::decoded_len(message_type_first_byte);
                    match vu64::decode_with_length(message_type_len, &pkt.payload()[0..message_type_len as usize]) {
                        Ok(message_type_id) => {
                            let message_type_id = message_type_id as NetMsgId;
                            let message = InboundNetMsg {
                                message_type_id,
                                payload: pkt.payload()[message_type_len as usize..].to_vec(),
                                peer_identity: self.peer_identity.clone(),
                            };
                            if finished_packets.get(&message_type_id).is_none() { 
                                finished_packets.insert(message_type_id, Vec::default());
                            }
                            finished_packets.get_mut(&message_type_id).unwrap().push(message);
                        },
                        Err(e) => errors.push(e.into()),
                    }
                },
                _ => unreachable!("We already filtered out all non-packet Laminar SocketEvents!"),
            }
        };
        // Push our messages out to the rest of the application.
        for (message_type, message_buf) in finished_packets { 
            match message_type {
                DISCONNECT_RESERVED => { 
                    info!("Peer {} has disconnected (deliberately - this is not an error)", self.peer_identity.to_base64()); 
                    self.disconnect_deliberate = true;
                }
                _ => {
                    //Non-reserved, game-defined net msg IDs.  
                    if let Some(channel) = self.received_channels.get_mut(&(message_type as NetMsgId)) { 
                        match channel.send(message_buf)
                            .map_err(|e| SessionLayerError::SendBroadcastError(e)) {
                            Ok(_x) => {},
                            Err(e) => errors.push(e),
                        }
                    }
                    else {
                        error!("A NetMessage of type {} has been receved from {}, but we have no handlers associated with that type of message. \n It's possible this peer is using a newer version of Gestalt.", 
                            message_type, self.peer_identity.to_base64());
                        errors.push(SessionLayerError::NoHandler(message_type, self.peer_identity.to_base64()));
                    }
                }
            }
        }

        //Our possible replies to the inbound packets.
        let reply_packets: Vec<(SocketAddr, Vec<u8>)> = self.laminar.empty_outbox();

        let mut processed_reply_buf: Vec<OuterEnvelope> = Vec::with_capacity(reply_packets.len());

        for (_, packet) in reply_packets {
            match self.encrypt_packet(&packet) {
                Ok(envelope) => processed_reply_buf.push(envelope),
                Err(e) => errors.push(e),
            }
        }

        if !processed_reply_buf.is_empty() {
            self.laminar.connection_state.record_send();
        }

        //Send to UDP socket.
        match self.push_channel.send(processed_reply_buf) {
            Ok(()) => {},
            Err(e) => errors.push(e.into()),
        }

        errors
    }

    pub async fn process_update(&mut self, time: Instant) -> Result<(), SessionLayerError> {
        let mut errors: Vec<SessionLayerError> = Vec::default();
        match self.laminar.process_update(time) {
            Ok(()) => {},
            Err(e) => errors.push(e.into()),
        }

        // Check to see if we need to send anything.
        let to_send: Vec<(SocketAddr, Vec<u8>)> = self.laminar.empty_outbox();
        let mut processed_send: Vec<OuterEnvelope> = Vec::with_capacity(to_send.len());
        
        for (_, packet) in to_send {
            match self.encrypt_packet(&packet) {
                Ok(envelope) => processed_send.push(envelope),
                Err(e) => errors.push(e),
            }
        }

        if !processed_send.is_empty() {
            self.laminar.connection_state.record_send();
        }

        //Send to UDP socket.
        match self.push_channel.send(processed_send) { 
            Ok(()) => {},
            Err(e) => errors.push(e.into()),
        }

        // Result / output
        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.pop().unwrap()), 
            _ => Err(SessionLayerError::ErrorBatch(errors))
        }
    }

    /// Adds Laminar connection logic to messages that we are sending. 
    pub async fn process_outbound<T: IntoIterator< Item=laminar::Packet >>(&mut self, outbound_messages: T, time: Instant)  -> Result<(), SessionLayerError> {
        let mut errors: Vec<SessionLayerError> = Vec::default();
        match self.laminar.process_outbound(outbound_messages, time) {
            Ok(()) => {},
            Err(e) => errors.push(e.into()),
        }

        // Check to see if we need to send anything.
        let to_send: Vec<(SocketAddr, Vec<u8>)> = self.laminar.empty_outbox();
        let mut processed_send: Vec<OuterEnvelope> = Vec::with_capacity(to_send.len());
        
        for (_, packet) in to_send {
            match self.encrypt_packet(&packet) {
                Ok(envelope) => processed_send.push(envelope),
                Err(e) => errors.push(e),
            }
        }

        if !processed_send.is_empty() {
            self.laminar.connection_state.record_send();
        }

        //Send to UDP socket.
        match self.push_channel.send(processed_send) { 
            Ok(()) => {},
            Err(e) => errors.push(e.into()),
        }

        // Result / output
        match errors.len() {
            0 => Ok(()),
            1 => Err(errors.pop().unwrap()), 
            _ => Err(SessionLayerError::ErrorBatch(errors))
        }
    }

    /// Network connection CPR.
    pub async fn force_heartbeat(&mut self) -> Result<(), laminar::error::ErrorKind> { 
        let addr = self.peer_address;
        let packets = self.laminar.connection_state.process_outgoing(laminar::packet::PacketInfo::heartbeat_packet(&[]), None, Instant::now())?;
        for packet in packets { 
            self.laminar.messenger.send_packet(&self.peer_address, &packet.contents());
        }
        Ok(())
    } 
}

/// Meant to be run inside a Tokio runtime - this will loop infinitely.
/// 
/// # Arguments
///
/// * `incoming_packets` - Packets coming in off the UDP socket, routed to this session 
/// * `send_channel` - Channel used by the rest of the engine to send messages out to this peer.  
/// * `session_tick` - Interval between times we examine if we should .  
///
pub async fn handle_session(mut session_manager: Session,
                        mut incoming_packets: mpsc::UnboundedReceiver<Vec<OuterEnvelope>>,
                        mut send_channel: mpsc::UnboundedReceiver<Vec<PacketIntermediary>>,
                        session_tick: Duration,
                        kill_from_inside: mpsc::UnboundedSender<(FullSessionName, Vec<SessionLayerError>)>,
                        mut kill_from_outside: tokio::sync::oneshot::Receiver<()>) { 
    let mut ticker = tokio::time::interval(session_tick);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    info!("Handling session for peer {}...", session_manager.peer_identity.to_base64());

    let peer_address = session_manager.peer_address.clone();
    let mut keep_running = true;
    let mut error_state = false;
    while keep_running {
        tokio::select!{
            // Inbound packets
            // Per tokio documentation - "This method is cancel safe. If recv is used as the event in a tokio::select! statement and some other branch completes first, it is guaranteed that no messages were received on this channel."
            inbound_packets_maybe = (&mut incoming_packets).recv() => { 
                match inbound_packets_maybe { 
                    Some(inbound_packets) => { 
                        let ingest_results = session_manager.ingest_packets(inbound_packets, Instant::now()).await;
                        if !ingest_results.is_empty() { 
                            let mut built_string = String::default();
                            for errorout in ingest_results.iter() { 
                                let to_append = format!("* {:?} \n", errorout);
                                built_string.push_str(to_append.as_str());
                            }
                            error!("Errors encountered parsing inbound packets in a session with {}: \n {}", session_manager.peer_identity.to_base64(), built_string);
                            kill_from_inside.send((session_manager.get_session_name() ,  ingest_results)).unwrap();
                            keep_running = false;
                            error_state = true;
                            break;
                        }
                    }, 
                    None => { 
                        info!("Connection closed for {}, dropping session state.", session_manager.peer_identity.to_base64());
                        kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
                        keep_running = false;
                        error_state = true;
                        break;
                    }
                }
                if session_manager.disconnect_deliberate { 
                    kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
                    keep_running = false;
                    break;
                }
            },
            send_packets_maybe = (&mut send_channel).recv() => {
                match send_packets_maybe {
                    Some(send_packets) => {
                        session_manager.laminar.connection_state.record_send();
                        let serialize_results = session_manager.process_outbound(send_packets.into_iter().map(|intermediary| intermediary.make_full_packet(peer_address)), Instant::now()).await;
                        if let Err(e) = serialize_results {
                            error!("Error encountered attempting to send a packet to peer {}: {:?}", session_manager.peer_identity.to_base64(), e);
                            kill_from_inside.send((session_manager.get_session_name(), vec![e])).unwrap();
                            keep_running = false;
                            error_state = true;
                            break;
                        }
                    }, 
                    None => { 
                        info!("Connection closed for {}, dropping session state.", session_manager.peer_identity.to_base64());
                        kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
                        keep_running = false;
                        error_state = true;
                        break;
                    }
                }
                if session_manager.disconnect_deliberate { 
                    kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
                    keep_running = false;
                    break;
                }
            },
            _ = (&mut ticker).tick() => {
                let update_results = session_manager.process_update(Instant::now()).await;
                if let Err(e) = update_results {
                    info!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", session_manager.laminar.connection_state.packets_in_flight(), session_manager.laminar.connection_state.last_heard(Instant::now()), session_manager.laminar.connection_state.is_established()); 
                    error!("Error encountered while ticking network connection to peer {}: {:?}", session_manager.peer_identity.to_base64(), e);
                    kill_from_inside.send((session_manager.get_session_name(), vec![e])).unwrap();
                    keep_running = false;
                    error_state = true;
                    break;
                }
            }
            kill_maybe = (&mut kill_from_outside) => { 
                info!("Shutting down session with user {}", session_manager.peer_identity.to_base64() );
                keep_running = false;
                break;
            }
        }
    }
    //error!("A session manager for a session between {} (us) and {} (peer) has stopped looping.", session_manager.local_identity.public.to_base64(), session_manager.peer_identity.to_base64());
}

// Each packet on the wire:
// [- 4 bytes session ID -------------------------------]  
// [- 4 bytes message counter --------------------------]
// [- 1-9 bytes vu64 bytes encoding ciphertext size, n -]
// [- n bytes ciphertext -------------------------------]

const MAX_MESSAGE_SIZE: usize = 8192;

pub fn encode_outer_envelope(message: &OuterEnvelope, send_buf: &mut [u8]) -> usize {
    const SESSION_ID_LEN: usize = std::mem::size_of::<SessionId>();
    const COUNTER_LEN: usize = std::mem::size_of::<MessageCounter>();
    let message_len = message.ciphertext.len();
    let encoded_len = vu64::encode(message_len as u64);

    let len_tag_bytes: &[u8] = encoded_len.as_ref();
    
    //let header_len = SESSION_ID_LEN + COUNTER_LEN + len_tag_bytes.len();

    let mut cursor = 0;
    //pub session_id: FullSessionName,
    let session_id = message.session_id.session_id.clone();
    //pub counter: MessageCounter,
    send_buf[cursor..cursor+SESSION_ID_LEN].copy_from_slice(&session_id);
    cursor += SESSION_ID_LEN;
    
    //pub counter: MessageCounter,
    send_buf[cursor..cursor+COUNTER_LEN].copy_from_slice(&message.counter.to_le_bytes());
    cursor += COUNTER_LEN;
    
    send_buf[cursor..cursor+len_tag_bytes.len()].copy_from_slice(len_tag_bytes);
    cursor += len_tag_bytes.len();

    //Header done, now write the data.
    send_buf[cursor..cursor+message_len].copy_from_slice(&message.ciphertext);
    cursor += message_len;
    cursor
}

pub async fn run_network_system(role: NetworkRole, address: SocketAddr, 
            mut new_connections: mpsc::UnboundedReceiver<SuccessfulConnect>,
            local_identity: IdentityKeyPair, 
            received_message_channels: HashMap<NetMsgId, NetMsgSender>,
            laminar_config: LaminarConfig,
            session_tick_interval: Duration,
            mut quit_handler: tokio::sync::oneshot::Receiver<()>) {
    
    info!("Initializing network subsystem for {:?}, which is a {:?}. Attempting to bind to socket on {:?}", local_identity.public.to_base64(), role, address);
    let socket = if role == NetworkRole::Client {
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await.unwrap();
        socket.connect(address).await.unwrap();
        socket
    }
    else {
        UdpSocket::bind(address).await.unwrap()
    };
    info!("Bound network subsystem to a socket at: {:?}. We are a {:?}", socket.local_addr().unwrap(), role);

    const SESSION_ID_LEN: usize = std::mem::size_of::<SessionId>();
    const COUNTER_LEN: usize = std::mem::size_of::<MessageCounter>();

    let mut recv_buf = vec![0u8; MAX_MESSAGE_SIZE];
    let mut send_buf = vec![0u8; MAX_MESSAGE_SIZE];

    // Used by servers to hold on to client info until we can ascertain their new port number (the TCP port number from preprotocol/handshake got dropped) 
    let mut anticipated_clients: HashMap<PartialSessionName, SuccessfulConnect> = HashMap::new();

    // One receiver for each session. Messages come into this UDP handler from sessions, and we have to send them.
    // Remember, "Multiple producer single receiver." This is the single receiver.
    let (push_sender, mut push_receiver): (PushSender, PushReceiver) = mpsc::unbounded_channel(); 
    // Per-session channels for routing incoming UDP packets to sessions.
    let mut inbound_channels: HashMap<FullSessionName, mpsc::UnboundedSender<Vec<OuterEnvelope>> > = HashMap::new();

    // Sessions' way of letting us know its their time to go.
    let (kill_from_inside_sender, mut kill_from_inside_receiver) = mpsc::unbounded_channel::<(FullSessionName, Vec<SessionLayerError>)>();
    // This is how we shoot the other task in the head.
    let mut session_kill_from_outside: HashMap<FullSessionName, tokio::sync::oneshot::Sender<()>> = HashMap::new();

    let mut session_to_identity: HashMap<FullSessionName, NodeIdentity> = HashMap::new();
    
    info!("Network system initialized. Our role is {:?}, our address is {:?}, and our identity is {}", &role, &socket.local_addr(), local_identity.public.to_base64());
    let mut join_handles = Vec::new(); 

    let mut continue_running = true;
    while continue_running {
        tokio::select!{
            // A packet has been received. 
            received_maybe = (&socket).recv_from(&mut recv_buf) => {
                // TODO: Better error handling later.
                match received_maybe {
                    Ok((len_read, peer_address)) => {
                        assert!(len_read >= SESSION_ID_LEN + COUNTER_LEN + 1);
                        let mut session_id = [0u8; SESSION_ID_LEN];
                        let mut counter_bytes = [0u8; COUNTER_LEN];
        
                        let mut cursor = 0;
                        session_id.copy_from_slice(&recv_buf[cursor..cursor+SESSION_ID_LEN]);
                        cursor += SESSION_ID_LEN;
        
                        counter_bytes.copy_from_slice(&recv_buf[cursor..cursor+COUNTER_LEN]);
                        cursor += COUNTER_LEN;
                        
                        let counter = MessageCounter::from_le_bytes(counter_bytes);
        
                        let first_length_tag_byte: u8 = recv_buf[cursor];
                        //Get the length of the vu64 length tag from the first byte.
                        let lenlen = vu64::decoded_len(first_length_tag_byte) as usize;
                        let message_length = vu64::decode(&recv_buf[cursor..cursor+lenlen]).unwrap(); //TODO: Error handling. 
                        cursor += lenlen;

                        let session_name = FullSessionName {
                            peer_address,
                            session_id,
                        };
                        match inbound_channels.get(&session_name) { 
                            Some(sender) => {
                                if message_length > 0 {
                                    let ciphertext = (&recv_buf[cursor..cursor+message_length as usize]).to_vec();
                                    sender.send(vec![OuterEnvelope {
                                        session_id: FullSessionName { 
                                            session_id, 
                                            peer_address,
                                        },
                                        counter,
                                        ciphertext,
                                    }]).unwrap()
                                }
                                else { 
                                    warn!("Zero-length message on session {:?}", &session_name);
                                }
                            },
                            None => {
                                if role == NetworkRole::Server {
                                    let partial_session_name = PartialSessionName {
                                        peer_address: peer_address.ip(),
                                        session_id,
                                    };
                                    match anticipated_clients.remove(&partial_session_name) {
                                        Some(connection) => {
                                            info!("Popping anticipated client entry for session {:?} and establishing a session.", &base64::encode(connection.session_id));
                                            //Communication with the rest of the engine.
                                            let (game_to_session_sender, game_to_session_receiver) = mpsc::unbounded_channel();
                                            match net_channel::register_channel(connection.peer_identity.clone(), game_to_session_sender) { 
                                                Ok(()) => {
                                                    info!("Sender channel successfully registered for {}", connection.peer_identity.to_base64());
                                                    let mut session = Session::new(local_identity.clone(), peer_address, connection, laminar_config.clone(), push_sender.clone(), received_message_channels.clone(), Instant::now());
                                                    session.laminar.connection_state.record_recv();
                                                    //Make a channel 
                                                    let (from_net_sender, from_net_receiver) = mpsc::unbounded_channel();
                                                    inbound_channels.insert(session_name, from_net_sender);

                                                    let (kill_from_outside_sender, kill_from_outside_receiver) = tokio::sync::oneshot::channel::<()>();
                                                    session_kill_from_outside.insert(session.get_session_name(), kill_from_outside_sender);
                                    
                                                    let killer_clone = kill_from_inside_sender.clone();
                                                    session_to_identity.insert(session.get_session_name(), session.peer_identity.clone());
                                                    let jh = tokio::spawn( async move {
                                                        session.force_heartbeat().await.unwrap();
                                                        handle_session(session, from_net_receiver, game_to_session_receiver, session_tick_interval.clone(), killer_clone, kill_from_outside_receiver).await
                                                    });

                                                    join_handles.push(jh);

                                                    if message_length > 0 {
                                                        let ciphertext = (&recv_buf[cursor..cursor+message_length as usize]).to_vec();
                                                        inbound_channels.get(&session_name).unwrap().send(vec![OuterEnvelope {
                                                            session_id: FullSessionName { 
                                                                session_id, 
                                                                peer_address,
                                                            },
                                                            counter,
                                                            ciphertext,
                                                        }]).unwrap()
                                                    }
                                                    else { 
                                                        inbound_channels.get(&session_name).unwrap().send(vec![OuterEnvelope {
                                                            session_id: FullSessionName { 
                                                                session_id, 
                                                                peer_address,
                                                            },
                                                            counter,
                                                            ciphertext: Vec::new(),
                                                        }]).unwrap()
                                                    }
                                                },
                                                Err(e) => { 
                                                    error!("Error initializing new session: {:?}", e);
                                                    println!("Game-to-session-sender already registered for {}", connection.peer_identity.to_base64());
                                                }
                                            }
                                        },
                                        None => {
                                            error!("No session established yet for {:?}", &session_name);
                                        },
                                    }
                                }
                                else {
                                    // TODO: Retain messages in case we run into problems.
                                    error!("No session established yet for {:?}", &session_name);
                                }
                            }
                        }
                    }
                    Err(e) => { 
                        if e.raw_os_error() == Some(10054) { 
                            //An existing connection was forcibly closed by the remote host.
                            //ignore - timeout will catch it.
                            warn!("Bad disconnect, an existing connection was forcibly closed by the remote host.");
                        }
                        else { 
                            error!("Error while polling for UDP packets: {:?}", e); 
                            panic!();
                        }
                    }
                }
            }
            send_maybe = (&mut push_receiver).recv() => {
                let to_send = send_maybe.unwrap();
                for message in to_send {

                    let encoded_len = encode_outer_envelope(&message, &mut send_buf);

                    //println!("Buffer is {} bytes long and we got to {}. Sending to {:?}", send_buf.len(), cursor+message_len, &message.session_id.peer_address);
                    //Push
                    match role {
                        NetworkRole::Client => socket.send(&send_buf[0..encoded_len]).await.unwrap(),
                        _ => socket.send_to(&send_buf[0..encoded_len], message.session_id.peer_address).await.unwrap()
                    };
                     //TODO: Error handling here.
                }
            }
            new_connection_maybe = (&mut new_connections).recv() => {
                let connection = match new_connection_maybe { 
                    Some(conn) => conn, 
                    None => {
                        warn!("Channel for new connections closed (we are a {:?} and our address is {:?}) - most likely this means the engine is shutting down, which is fine.", role, address);
                        break; // Return to loop head i.e. try a new tokio::select.
                    }, 
                };
                
                info!("Setting up reliability-over-UDP and cryptographic session \n for peer {} with address {:?}, role {:?}, \n connecting from Gestalt engine version v{}", connection.peer_identity.to_base64(), &connection.peer_address, &connection.peer_role, &connection.peer_engine_version);

                let session_name = connection.get_full_session_name();
                
                //local_identity: IdentityKeyPair, connection: SuccessfulConnect, laminar_config: &LaminarConfig, 
                //push_channel: PushSender, received_message_channels: HashMap<NetMsgId, NetMsgSender>, time: Instant
                //Todo: Senders.

                if role == NetworkRole::Server {
                    info!("Adding anticipated client entry for session {:?}", &base64::encode(connection.session_id));
                    anticipated_clients.insert( PartialSessionName{
                        session_id: connection.session_id.clone(), 
                        peer_address: connection.peer_address.ip(),
                    }, connection);
                }
                else {
                    //Communication with the rest of the engine. 
                    let (game_to_session_sender, game_to_session_receiver) = mpsc::unbounded_channel();
                    match net_channel::register_channel(connection.peer_identity.clone(), game_to_session_sender) { 
                        Ok(()) => { 
                            info!("Sender channel successfully registered for {}", connection.peer_identity.to_base64());
                            let mut session = Session::new(local_identity.clone(), connection.peer_address, connection, laminar_config.clone(), push_sender.clone(), received_message_channels.clone(), Instant::now());
                            session.laminar.connection_state.record_recv();
                            //Make a channel 
                            let (from_net_sender, from_net_receiver) = mpsc::unbounded_channel();
                            inbound_channels.insert(session_name, from_net_sender);

                            let (kill_from_outside_sender, kill_from_outside_receiver) = tokio::sync::oneshot::channel::<()>();
                            session_kill_from_outside.insert(session.get_session_name(), kill_from_outside_sender);
                                                
                            let killer_clone = kill_from_inside_sender.clone();
                            session_to_identity.insert(session.get_session_name(), session.peer_identity.clone());
                            let jh = tokio::spawn( async move {
                                session.force_heartbeat().await.unwrap();
                                handle_session(session, from_net_receiver, game_to_session_receiver, session_tick_interval.clone(), killer_clone, kill_from_outside_receiver).await
                            });

                            join_handles.push(jh);
                        }, 
                        Err(e) => { 
                            error!("Error initializing new session: {:?}", e);
                            println!("Game-to-session-sender already registered for {}", connection.peer_identity.to_base64());
                        }
                    }
                }
            }
            // Has one of our sessions failed or disconnected? 
            kill_maybe = (&mut kill_from_inside_receiver).recv() => { 
                if let Some((session_kill, errors)) = kill_maybe { 
                    if errors.is_empty() { 
                        info!("Closing connection for a session with {:?}.", session_kill.peer_address); 
                    }
                    else { 
                        info!("Closing connection for a session with {:?}, due to errors: {:?}", session_kill.peer_address, errors); 
                    }
                    let session = session_to_identity.get(&session_kill).unwrap(); 
                    net_channel::drop_channel(session).unwrap();
                    inbound_channels.remove(&session_kill); 
                    session_kill_from_outside.remove(&session_kill);
                }
            }
            quit_maybe = (&mut quit_handler) => {
                info!("Shutting down network system.");
                // Notify sessions we're done.
                for (peer_address, _) in inbound_channels.iter() { 
                    let peer_ident = session_to_identity.get(&peer_address).unwrap();
                    net_channel::send_to(&DisconnectMsg{}, &peer_ident).unwrap();
                }
                tokio::time::sleep(Duration::from_millis(10)).await; 
                // Clear out remaining messages.
                while let Ok(messages) = (&mut push_receiver).try_recv() {
                    for message in messages {
                        let encoded_len = encode_outer_envelope(&message, &mut send_buf);

                        //Push
                        match role {
                            NetworkRole::Client => socket.send(&send_buf[0..encoded_len]).await.unwrap(),
                            _ => socket.send_to(&send_buf[0..encoded_len], message.session_id.peer_address).await.unwrap()
                        };
                    }
                }
                // Notify sessions we're done.
                for (session, channel) in session_kill_from_outside { 
                    info!("Terminating session {:?}", session);
                    channel.send(()).unwrap();
                }
                for jh in join_handles { 
                    jh.abort();
                }
                continue_running = false; 
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv6Addr;

    use super::*;
    use log::LevelFilter;
    use parking_lot::Mutex;
    use serde::Serialize;
    use serde::Deserialize;
    use simplelog::TermLogger;
    use super::net_channel::NetSendChannel;
    use super::preprotocol::launch_preprotocol_listener;
    use super::preprotocol::preprotocol_connect_to_server;
    use lazy_static::lazy_static;

    lazy_static! {
        /// Used to keep tests which use real network i/o from clobbering eachother. 
        pub static ref NET_TEST_MUTEX: Mutex<()> = Mutex::new(());
    }
 
    #[derive(Clone, Serialize, Deserialize, Debug)]
    struct TestNetMsg {
        pub message: String, 
    }
    impl_netmsg!(TestNetMsg, 0, ReliableOrdered);

    #[tokio::test(flavor = "multi_thread")]
    async fn session_with_localhost() {
        let mutex_guard = NET_TEST_MUTEX.lock();
        let _log = TermLogger::init(LevelFilter::Debug, simplelog::Config::default(), simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto );

        let server_key_pair = IdentityKeyPair::generate_for_tests();
        let client_key_pair = IdentityKeyPair::generate_for_tests();
        let (serv_completed_sender, serv_completed_receiver) = mpsc::unbounded_channel();
        let (client_completed_sender, client_completed_receiver) = mpsc::unbounded_channel();

        let server_addr = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let server_socket_addr = SocketAddr::new(server_addr, GESTALT_PORT);
        let client_addr = IpAddr::V4(Ipv4Addr::LOCALHOST);

        let (serv_message_inbound_sender, serv_message_inbound_receiver) = tokio::sync::broadcast::channel(4096);
        let (client_message_inbound_sender, client_message_inbound_receiver) = tokio::sync::broadcast::channel(4096);

        let server_channels = HashMap::from([(TestNetMsg::net_msg_id(), serv_message_inbound_sender.clone())]);
        let client_channels = HashMap::from([(TestNetMsg::net_msg_id(), client_message_inbound_sender.clone())]);
        
        let (quit_server_s, quit_server_r) = tokio::sync::oneshot::channel();
        let (quit_client_s, quit_client_r) = tokio::sync::oneshot::channel();
        //Launch server
        let join_handle_s = tokio::spawn(
            run_network_system(NetworkRole::Server,
                server_socket_addr,
                serv_completed_receiver,
                server_key_pair.clone(), 
                server_channels,
                LaminarConfig::default(),
                Duration::from_millis(50),
                quit_server_r)
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        let join_handle_handshake_listener = tokio::spawn(launch_preprotocol_listener(server_key_pair.clone(), Some(server_socket_addr), serv_completed_sender ));
        tokio::time::sleep(Duration::from_millis(10)).await;

        //Launch client
        let join_handle_c = tokio::spawn(
            run_network_system( NetworkRole::Client,  server_socket_addr, 
            client_completed_receiver,
                client_key_pair.clone(),
                client_channels,
                LaminarConfig::default(),
                Duration::from_millis(50),
                quit_client_r)
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        let client_completed_connection = preprotocol_connect_to_server(client_key_pair.clone(),
                server_socket_addr,
                Duration::new(5, 0) ).await.unwrap();
        client_completed_sender.send(client_completed_connection).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        let test = TestNetMsg { 
            message: String::from("Beep!"), 
        };
        let message_sender: NetSendChannel<TestNetMsg> = net_channel::subscribe_typed(&server_key_pair.public).unwrap();
        info!("Attempting to send a message to {}", server_key_pair.public.to_base64());
        message_sender.send(&test).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        let mut test_receiver: TypedNetMsgReceiver<TestNetMsg> = TypedNetMsgReceiver::new(serv_message_inbound_receiver);

        {
            let out = test_receiver.recv().await.unwrap();
            let (peer_ident, out) = out.first().unwrap().clone();

            println!("Got {:?} from {}", out, peer_ident.to_base64());

            assert_eq!(out.message, test.message);
        }

        let test_reply = TestNetMsg { 
            message: String::from("Boop!"), 
        };
        let server_to_client_sender: NetSendChannel<TestNetMsg> = net_channel::subscribe_typed(&client_key_pair.public).unwrap();
        server_to_client_sender.send(&test_reply).unwrap();
        info!("Attempting to send a message to {}", client_key_pair.public.to_base64());
        let mut client_receiver: TypedNetMsgReceiver<TestNetMsg> = TypedNetMsgReceiver::new(client_message_inbound_receiver);

        {
            let out = client_receiver.recv().await.unwrap();
            let (peer_ident, out) = out.first().unwrap().clone();

            println!("Got {:?} from {}", out, peer_ident.to_base64());

            assert_eq!(out.message, test_reply.message);
        }
        quit_client_s.send(()).unwrap();
        let _ = join_handle_c.await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        quit_server_s.send(()).unwrap();
        let _ = join_handle_s.await;
        tokio::time::sleep(Duration::from_millis(100)).await;
        drop(mutex_guard);
    }
}