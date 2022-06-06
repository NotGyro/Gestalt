use std::any::Any;
use std::collections::VecDeque;
use std::fs;
use std::io::BufReader;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use futures::SinkExt;
use hashbrown::HashMap;
use laminar::Connection;
use laminar::DatagramSocket;
use laminar::VirtualConnection;
use log::error;
use log::info;
use serde::de::DeserializeOwned;
use serde::{Serialize, Deserialize};

use snow::StatelessTransportState;
use tokio::sync::mpsc;

use crate::common::Version;
use crate::common::identity::IdentityKeyPair;
use crate::common::identity::NodeIdentity;

use self::preprotocol::NetworkRole;

pub const PREPROTCOL_PORT: u16 = 54134;
pub const GESTALT_PORT: u16 = 54135;
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
pub mod preprotocol;

pub const SESSION_ID_LEN: usize = 4;
pub type SessionId = [u8; SESSION_ID_LEN];

pub type MessageCounter = u32;

/// Represents a client who has completed a handshake in the pre-protocol and will now be moving over to the game protocol proper
pub struct SuccessfulConnect {
    pub session_id: SessionId,
    pub peer_identity: NodeIdentity,
    pub peer_address: SocketAddr,
    pub peer_role: NetworkRole, 
    pub peer_engine_version: Version,
    pub transport_cryptography: StatelessTransportState,
    pub transport_counter: u32,
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

/// Partial reimplementation of a Laminar::ConnectionManager with somewhat different logic since we're using async here, and there will be one of these per peer per node.
pub struct LaminarConnectionManager {
    peer_address: SocketAddr,
    connection_state: VirtualConnection,
    messenger: TransportWrapper,
}

impl LaminarConnectionManager {
    /// Ingests a batch of packets coming off the wire.
    pub fn process_inbound<T: IntoIterator< Item: AsRef<[u8]> >>(&mut self, inbound_messages: T, time: Instant) -> Result<(), LaminarWrapperError> {
        let messenger = &mut self.messenger;
        for payload in inbound_messages.into_iter() { 
            let was_est = self.connection_state.is_established();
            //Processing inbound
            self.connection_state.process_packet(messenger, payload.as_ref(), time);
            if !was_est && self.connection_state.is_established() {
                info!("Connection established with {:?}", self.peer_address);
            }
        }

        self.connection_state.update(messenger, time);
        
        match self.connection_state.should_drop(messenger, time) { 
            false => Ok(()),
            true => Err(LaminarWrapperError::Disconnect(self.peer_address))
        }
    }
    pub fn process_update(&mut self, time: Instant) -> Result<(), LaminarWrapperError> {
        let messenger = &mut self.messenger;
        self.connection_state.update(messenger, time);
        
        match self.connection_state.should_drop(messenger, time) {
            false => Ok(()),
            true => Err(LaminarWrapperError::Disconnect(self.peer_address))
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
            true => Err(LaminarWrapperError::Disconnect(self.peer_address))
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


// Each packet on the wire:
// [- 4 bytes session ID ------------------]  
// [- 4 bytes message counter -------------]
// [- 4 bytes encoding ciphertext size, n -]
// [- n bytes ciphertext ------------------]

/// Decoded top-level envelope containing the session id, the counter, and the ciphertext, to send to the session layer.
#[derive(Debug, Clone)]
pub struct OuterEnvelope {
    pub session_id: SessionId,
    /// Counter, monotonically increasing, encoded as 4 little endian bytes on the wire. 
    pub counter: MessageCounter,
    /// Noise-protocol-encrypted ciphertext, which decrypts to a Laminar packet containing EncodedNetMsg bytes.
    pub ciphertext: Vec<u8>,
}

/// What type of packet are we sending/receiving? Should 1-to-1 correspond with a type of NetMessage.
pub type NetMsgId = u16;

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
    pub message_type_id: NetMsgId,
    // Our MsgPack-encoded actual NetMsg.
    pub payload: Vec<u8>,
    pub source: NodeIdentity,
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
}

/// Any type which can be encoded as a NetMessage to be sent out over the wire.
pub trait NetMsgOut: Serialize + DeserializeOwned + Clone {
    /// "_constant" was added here to disambiguate from the similarly-named but slightly distinct NetMsgIn::net_msg_id()
    fn net_msg_id_constant() -> NetMsgId;
    fn net_msg_guarantees() -> PacketGuarantees;
    fn net_msg_stream() -> StreamSelector;
    fn net_msg_type() -> NetMsgType { 
        NetMsgType { 
            id: Self::net_msg_id_constant(), 
            guarantees: Self::net_msg_guarantees(), 
            stream: Self::net_msg_stream(),
        }
    }
}

/// Any type which can be decoded from an inbound packet into a NetMessage.
/// This type will be used to make a trait object, and as such it must be object-safe.
/// See: https://doc.rust-lang.org/reference/items/traits.html#object-safety
pub trait NetMsgDyn: Send + Sync + Any {
    fn net_msg_id(&self) -> NetMsgId;
}

pub struct NetMsgIn { 
    pub payload: Box<dyn NetMsgDyn>,
    pub source: NodeIdentity,
}

#[derive(Clone)]
pub struct NetMsgTableEntry {
    pub type_info: NetMsgType,
    pub decoder_closure: fn(InboundNetMsg) -> Result<NetMsgIn, NetMsgDecodeErr>,
}

impl NetMsgTableEntry {
    pub fn register<T: NetMsgOut + NetMsgDyn>() -> Self { 
        NetMsgTableEntry {
            type_info: T::net_msg_type(),
            //Make a closure which can be used to decode this type. 
            decoder_closure: |inbound: InboundNetMsg| -> Result<NetMsgIn, NetMsgDecodeErr> { 
                if T::net_msg_id_constant() != inbound.message_type_id { 
                    Err(NetMsgDecodeErr::WrongType(T::net_msg_id_constant(), inbound.message_type_id))
                }
                else {
                    let InboundNetMsg{message_type_id: _, payload, source } = inbound;
                    let payload: T = rmp_serde::from_read(&payload[..])?;
                    Ok(NetMsgIn {
                        payload: Box::new(payload),
                        source,
                    })
                }
            }
            //And that's it for now
        }
    }
}

pub struct NetMsgTable { 
    table: Vec<Option<NetMsgTableEntry>>
}
impl Default for NetMsgTable {
    fn default() -> Self {
        Self { table: vec![None; NetMsgId::MAX as usize] }
    }
}

impl NetMsgTable {
    pub fn register<T: NetMsgOut + NetMsgDyn>(&mut self) {
        self.table[T::net_msg_id_constant() as usize] = Some(NetMsgTableEntry::register::<T>());
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Hash)]
pub struct FullSessionName { 
    pub peer_address: SocketAddr, 
    pub session_id: SessionId,
}

pub type EnvelopeBuf = (FullSessionName, Vec<OuterEnvelope>);

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
    SendChannelError(#[from] tokio::sync::mpsc::error::SendError<EnvelopeBuf>),
    #[error("Connection with {0:?} timed out.")]
    LaminarTimeout(SocketAddr),
    #[error("Peer {0:?} disconnected.")]
    LaminarDisconnect(SocketAddr),
    #[error("Peer {0:?} sent a Laminar \"connect\" message after the session was already started!")]
    ConnectAfterStarted(SocketAddr),
}

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
    /// Channel of packets to send out on the UDP socket
    pub push_channel: mpsc::Sender<EnvelopeBuf>,
}

impl Session {
    /// Encrypts the raw byte blobs produced by Laminar and encloses them in an OuterEnvelope,  
    fn encrypt_packet<T: AsRef<[u8]>>(&mut self, plaintext: T) -> Result<OuterEnvelope, SessionLayerError> {
        self.local_counter += 1;
        let mut buffer = vec![0u8; ( (plaintext.as_ref().len() as usize) * 3)/2 ];
        let len_written = self.transport_cryptography.write_message(self.local_counter as u64, plaintext.as_ref(), &mut buffer)?;
        buffer.truncate(len_written);
        Ok( 
            OuterEnvelope {
                session_id: self.session_id,
                counter: self.local_counter,
                ciphertext: buffer,
            }
        )
    }

    /// Called inside process_inbound()
    fn decrypt_outer_envelope(&mut self, envelope: OuterEnvelope) -> Result<Vec<u8>, SessionLayerError> { 
        let OuterEnvelope{ session_id, counter, ciphertext } = envelope;

        #[cfg(debug_assertions)]
        {
            // Only check this in debug, because this should never happen unless the layer below this one is bugged.
            if self.session_id != session_id {
                return Err(SessionLayerError::WrongChannel(
                    base64::encode(&self.session_id),
                    base64::encode(&session_id),
                ));
            }
        }

        let mut buf = vec![0u8; (ciphertext.len() * 3)/2];
        let len_read = self.transport_cryptography.read_message(counter as u64, &ciphertext, &mut buf)?;
        buf.truncate(len_read);
        Ok(buf)
    }

    /// Ingests a batch of packets coming off the wire.
    pub async fn ingest_packets<T: IntoIterator< Item=OuterEnvelope >>(&mut self, inbound_messages: T, time: Instant) -> (Vec<InboundNetMsg>, Vec<SessionLayerError>) {
        let mut errors: Vec<SessionLayerError> = Vec::default();

        let mut batch: Vec<Vec<u8>> = Vec::default();
        for envelope in inbound_messages.into_iter() {
            match self.decrypt_outer_envelope(envelope) {
                Ok(packet_contents) => batch.push(packet_contents),
                Err(e) => errors.push(e),
            }
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
                laminar::SocketEvent::Connect(addr) => errors.push(SessionLayerError::ConnectAfterStarted(addr.clone())),
                laminar::SocketEvent::Packet(_) => unreachable!(), 
            }
        }
        //Now that we've handled those, convert.
        let processed_packets: Vec<InboundNetMsg> = processed_packets.drain(..).map(|evt| { 
            match evt {
                laminar::SocketEvent::Packet(pkt) => {
                    InboundNetMsg {
                        message_type_id: {
                            let mut bytes = [0u8;2];
                            bytes.copy_from_slice(&pkt.payload()[0..2]);
                            u16::from_le_bytes(bytes)
                        },
                        payload: pkt.payload()[2..].to_vec(),
                        //Where it came from. 
                        source: self.peer_identity.clone(),
                    }
                    
                },
                _ => unreachable!("We already filtered out all non-packet Laminar SocketEvents!"),
            }
        }).collect();

        //Our possible replies to the inbound packets.
        let reply_packets: Vec<(SocketAddr, Vec<u8>)> = self.laminar.empty_outbox();

        let mut processed_reply_buf: Vec<OuterEnvelope> = Vec::with_capacity(reply_packets.len());

        for (ip, packet) in reply_packets { 
            #[cfg(debug_assertions)]
            {
                if ip == self.peer_address {
                    //IP matches, no mistakes were made.
                    match self.encrypt_packet(&packet) {
                        Ok(envelope) => processed_reply_buf.push(envelope),
                        Err(e) => errors.push(e),
                    }
                }
                else {
                    //What is Laminar doing?
                    errors.push(SessionLayerError::WrongIpSend(ip, self.peer_address))
                }
            }
            #[cfg(not(debug_assertions))]
            {
                match self.encrypt_packet(&packet) {
                    Ok(envelope) => processed_reply_buf.push(envelope),
                    Err(e) => errors.push(e),
                }
            }
        }

        //Send to UDP socket.
        match self.push_channel.send(( FullSessionName{peer_address: self.peer_address, session_id: self.session_id}, processed_reply_buf)).await { 
            Ok(()) => {},
            Err(e) => errors.push(e.into()),
        }

        (processed_packets, errors)
    }

    pub async fn process_update(&mut self, time: Instant) -> Result<(), SessionLayerError> {
        self.laminar.process_update(time)?;

        let mut errors: Vec<SessionLayerError> = Vec::default();

        // Check to see if we need to send anything. 
        let to_send: Vec<(SocketAddr, Vec<u8>)> = self.laminar.empty_outbox();
        let mut processed_send: Vec<OuterEnvelope> = Vec::with_capacity(to_send.len());
        
        for (ip, packet) in to_send { 
            #[cfg(debug_assertions)]
            {
                if ip == self.peer_address {
                    //IP matches, no mistakes were made.
                    match self.encrypt_packet(&packet) {
                        Ok(envelope) => processed_send.push(envelope),
                        Err(e) => errors.push(e),
                    }
                }
                else {
                    //What is Laminar doing?
                    errors.push(SessionLayerError::WrongIpSend(ip, self.peer_address))
                }
            }
            #[cfg(not(debug_assertions))]
            {
                match self.encrypt_packet(&packet) {
                    Ok(envelope) => processed_send.push(envelope),
                    Err(e) => errors.push(e),
                }
            }
        }

        //Send to UDP socket.
        match self.push_channel.send(( FullSessionName{peer_address: self.peer_address, session_id: self.session_id}, processed_send)).await { 
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
        
        for (ip, packet) in to_send { 
            #[cfg(debug_assertions)]
            {
                if ip == self.peer_address {
                    //IP matches, no mistakes were made.
                    match self.encrypt_packet(&packet) {
                        Ok(envelope) => processed_send.push(envelope),
                        Err(e) => errors.push(e),
                    }
                }
                else {
                    //What is Laminar doing?
                    errors.push(SessionLayerError::WrongIpSend(ip, self.peer_address))
                }
            }
            #[cfg(not(debug_assertions))]
            {
                match self.encrypt_packet(&packet) {
                    Ok(envelope) => processed_send.push(envelope),
                    Err(e) => errors.push(e),
                }
            }
        }

        //Send to UDP socket.
        match self.push_channel.send(( FullSessionName{peer_address: self.peer_address, session_id: self.session_id}, processed_send)).await { 
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
}

/// Meant to be run inside a Tokio runtime - this will loop infinitely.
/// 
/// # Arguments
///
/// * `incoming_packets` - Packets coming in off the UDP socket, routed to this session 
/// * `outgoing_packets` - Packets this session-object has encoded, to send on the UDP socket. 
///
pub async fn handle_session(incoming_packets: mpsc::Receiver<Vec<OuterEnvelope>>, 
                            outgoing_packets: mpsc::Sender<EnvelopeBuf>,) { 
    //tokio::select!{
    //
    //}
}


/*
/// A NetMsg is a trait with enough information in its impl to send the
/// struct it's implemented on over the network, no other details required.
/// The idea is to declaratively describe which kind of packets need which
pub trait NetMsg<'a>: Sized + Serialize + Deserialize<'a> + Clone {
    fn packet_type_id() -> u8;
    fn required_guarantees() -> PacketGuarantees;
    fn which_stream() -> StreamSelector;

    fn construct_packet(&self, send_to: SocketAddr) -> Result<Packet, Box<dyn Error>> {
        // Start by writing our tag.
        let mut encoded: Vec<u8> = Self::packet_type_id().to_le_bytes().to_vec();

        // Then, write our data.
        {
            encoded.append(&mut bincode::serialize(&self)?);
        }

        // Branch on our message properties to figure out what kind of packet to construct.
        Ok(match Self::required_guarantees() {
            PacketGuarantees::UnreliableUnordered => {
                Packet::unreliable(send_to, encoded)
            },
            PacketGuarantees::UnreliableSequenced => {
                match Self::which_stream() {
                    StreamSelector::Any => Packet::unreliable_sequenced(send_to, encoded, None),
                    StreamSelector::Specific(id) => Packet::unreliable_sequenced(send_to, encoded, Some(id)),
                }
            },
            PacketGuarantees::ReliableUnordered => {
                Packet::reliable_unordered(send_to, encoded)
            },
            PacketGuarantees::ReliableOrdered => {
                match Self::which_stream() {
                    StreamSelector::Any => Packet::reliable_ordered(send_to, encoded, None),
                    StreamSelector::Specific(id) => Packet::reliable_ordered(send_to, encoded, Some(id)),
                }
            },
            PacketGuarantees::ReliableSequenced => {
                match Self::which_stream() {
                    StreamSelector::Any => Packet::reliable_sequenced(send_to, encoded, None),
                    StreamSelector::Specific(id) => Packet::reliable_sequenced(send_to, encoded, Some(id)),
                }
            },
        })
    }
}

macro_rules! impl_netmsg {
    ($message:ident, $id:expr, $guarantee:ident) => {
        impl NetMsg for $message {
            #[inline(always)]
            fn packet_type_id() -> u8 { $id }
            #[inline(always)]
            fn required_guarantees() -> PacketGuarantees { PacketGuarantees::$guarantee }
            #[inline(always)]
            fn which_stream() -> StreamSelector { StreamSelector::Any }
        }
    };
    ($message:ident, $id:expr, $guarantee:ident, $stream:expr) => {
        impl NetMsg for $message {
            #[inline(always)]
            fn packet_type_id() -> u8 { $id }
            #[inline(always)]
            fn required_guarantees() -> PacketGuarantees { PacketGuarantees::$guarantee }
            #[inline(always)]
            fn which_stream() -> StreamSelector { StreamSelector::Specific($stream) }
        }
    };
}
*/