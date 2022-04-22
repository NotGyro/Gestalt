use std::collections::VecDeque;
use std::net::SocketAddr;

use laminar::DatagramSocket;
use serde::{Serialize, Deserialize};

use tokio::sync::mpsc::Sender as TokioSender; 
use tokio::sync::mpsc::Receiver as TokioReceiver;
//use tokio::sync::mpsc::error::TryRecvError; 

use crossbeam_channel::{bounded, Sender, Receiver, TryRecvError};

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

pub mod preprotocol;

// A chunk has to be requested by a client (or peer server) before it is sent. So, a typical flow would go like this:
// 1. Client: My revision number on chunk (15, -8, 24) is 732. Can you give me the new stuff if there is any?
// 2. Server: Mine is 738, here is a buffer of 6 new voxel event logs to get you up to date.

/// Describes what kind of ordering guarantees are made about a packet.
/// Directly inspired by (and currently maps to!) Laminar's reliability types.
#[derive(Debug, Copy, Clone, PartialEq)]
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

/// Runtime information specifying what kind of connection we are looking at.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ConnectionRole {
    /// We are the server and we are connected to a client.
    ServerToClient,
    /// We are the client and we are connected to a server. 
    ClientToServer,
}

pub type StreamId = u8;

/// Which "stream" is this on?
/// A stream in this context must be a u8-identified separate channel of packets
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

/// Thin wrapper used to pretend, from the perspective of Laminar, 
/// that Noise protocol encryption and async UDP are a transparent synchronous UDP socket.
struct TransportWrapper {
    pub local_address: SocketAddr,
    pub outbox: VecDeque<(SocketAddr, Vec<u8>)>, 
    pub inbox: VecDeque<(SocketAddr, Vec<u8>)>,
}

impl DatagramSocket for TransportWrapper {
    fn send_packet(&mut self, addr: &SocketAddr, payload: &[u8]) -> std::io::Result<usize> {
        self.outbox.push((addr, payload.to_vec()));
        Ok(payload.len())
    }

    fn receive_packet<'a>(&mut self, buffer: &'a mut [u8]) -> std::io::Result<(&'a [u8], SocketAddr)> {
        match self.inbox.pop_front() {
            Some((addr, payload)) => Ok((&payload, addr)),
            // Laminar treats "would block" as a "no new messages" signal.
            None => Err(std::io::Error::new(std::io::ErrorKind::WouldBlock , "WouldBlock - Laminar should ignore this and continue.")),
        }
    }

    fn local_addr(&self) -> std::io::Result<SocketAddr> {
        Ok(self.local_address)
    }

    fn is_blocking_mode(&self) -> bool {
        false
    }
}

/// Encrypted top-level envelope containing the counter and the ciphertext
pub struct OuterEnvelope {
    pub counter: preprotocol::current_protocol::MessageCounter,
    /// Noise-protocol-encrypted ciphertext, which decrypts to a Laminar packet containing InnerEnvelope bytes.
    pub ciphertext: Vec<u8>,
}

/// Decrypted envelope, contains  
pub struct InnerEnvelope { 
    
}

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
