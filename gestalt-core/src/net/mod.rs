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
