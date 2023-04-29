use std::net::SocketAddr;

use log::warn;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::{
	common::{growable_buffer::GrowableBuf, identity::NodeIdentity},
	message::{ChannelDomain, MessageWithDomain, RecvError},
	net::{session::SessionId, FullSessionName, MessageCounter},
};

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
			}
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

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum MessageSidedness {
	ClientToServer,
	ServerToClient,
	Common,
}

impl SelfNetworkRole {
	/// On a node with this role, should we ingest a message with that sidedness?
	/// This is checked, even in release builds, as an extra security measure.
	/// Certain net message IDs are just *not allowed* to be sent to servers.
	#[inline(always)]
	pub fn should_we_ingest(&self, message_sidedness: &MessageSidedness) -> bool {
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

/// Encrypted message, used for all normal gameplay network traffic. These will be way more common than ProtocolMessages.
#[derive(Debug, Clone)]
pub struct CiphertextMessage {
	/// Counter, monotonically increasing, encoded as 4 little endian bytes on the wire.
	pub counter: MessageCounter,
	/// Noise-protocol-encrypted ciphertext, which decrypts to a Laminar packet containing EncodedNetMsg bytes.
	pub ciphertext: Vec<u8>,
}

#[derive(thiserror::Error, Debug)]
pub enum ProtocolMessageError {
	#[error("Unrecognized protocol message type: {0}")]
	UnrecognizedType(u8),
	#[error("Buffer not large enough to contain a protocol message- {0} bytes were provided and we need {1}.")]
	NotEnoughBuffer(usize, usize),
	#[error("Attempted to read a zero-length slice as a protocol message.")]
	CannotReadZeroLength,
}

/// Any message internal to the network system (not used by / propagatated through to the game engine).
#[derive(Debug, Clone)]
pub enum ProtocolMessage {
	Todo = 0,
}

impl ProtocolMessage {
	pub fn encode(&self, send_buf: &mut [u8]) -> Result<usize, ProtocolMessageError> {
		match self {
			ProtocolMessage::Todo => {
				if send_buf.len() == 0 {
					return Err(ProtocolMessageError::NotEnoughBuffer(0, 1));
				}
				send_buf[0] = 0;
				Ok(1)
			}
		}
	}
	pub fn decode(recv_buf: &[u8]) -> Result<(Self, usize), ProtocolMessageError> {
		if recv_buf.len() == 0 {
			return Err(ProtocolMessageError::NotEnoughBuffer(0, 1));
		}
		let variant = recv_buf[0];
		match variant {
			0 => Ok((Self::Todo, 1)),
			_ => Err(ProtocolMessageError::UnrecognizedType(variant)),
		}
	}
}

#[derive(thiserror::Error, Debug)]
pub enum OuterEnvelopeError {
	#[error("Attempted to encode an OuterEnvelope to a buffer not large enough to contain it - {0} bytes were provided and we need {1}.")]
	NotEnoughBuffer(usize, usize),
	#[error("Attempted to encode or decode an OuterEnvelope to/from a buffer {0} bytes in size. The minimum outer envelope size is {1} bytes.")]
	NotEnoughForHeader(usize, usize),
	#[error("Attempted to read a zero-length slice as an OuterEnvelope.")]
	CannotReadZeroLength,
	#[error("Ciphertext in an OuterEnvelope from {0:?} was zero-length.")]
	ZeroLengthCiphertext(SocketAddr),
	#[error("Outer Envelope Vu64 decoding error: {0:?}.")]
	Vu64Decode(#[from] vu64::Error),
}

/// Decoded top-level envelope containing the session id, the counter, and the ciphertext, to send to the session layer.
#[derive(Debug, Clone)]
pub struct OuterEnvelope {
	pub session: FullSessionName,
	pub body: CiphertextMessage,
}

impl OuterEnvelope {
	// Each ordinary game ciphertext message on the wire:
	// [- 4 bytes session ID ---------------------------------]
	// [- 4 bytes message counter, nonzero -------------------]
	// [- 1-9 bytes vu64 bytes encoding ciphertext size, n ---]
	// [- n bytes ciphertext ---------------------------------]

	// In the case of a protocol message, it will instead look like this:
	// [- 4 bytes session ID ---------------------------------]
	// [- 4 bytes, every bit will be 0 (u32::min) ------------]
	// [- 1 byte protocol message variant --------------------]
	// [- protocol-message-type-dependent stuff follows here.-]

	pub fn encode(&self, send_buf: &mut [u8]) -> Result<usize, OuterEnvelopeError> {
		const SESSION_ID_LEN: usize = std::mem::size_of::<SessionId>();
		const COUNTER_LEN: usize = std::mem::size_of::<MessageCounter>();

		if send_buf.len() < SESSION_ID_LEN + COUNTER_LEN + 1 {
			return Err(OuterEnvelopeError::NotEnoughForHeader(
				send_buf.len(),
				SESSION_ID_LEN + COUNTER_LEN + 1,
			));
		}

		let mut cursor = 0;

		//Write session ID.
		let session_id = self.session.session_id.clone();
		send_buf[cursor..cursor + SESSION_ID_LEN].copy_from_slice(&session_id);
		cursor += SESSION_ID_LEN;

		//Write counter
		send_buf[cursor..cursor + COUNTER_LEN].copy_from_slice(&self.body.counter.to_le_bytes());
		cursor += COUNTER_LEN;

		//Write ciphertext len.
		let message_len = self.body.ciphertext.len();
		let encoded_len = vu64::encode(message_len as u64);
		let len_tag_bytes: &[u8] = encoded_len.as_ref();
		let len_len_tag = len_tag_bytes.len();

		let body_len = len_len_tag + message_len;

		// Sanity-check now that we know how much we have to write
		let remaining_len = send_buf[cursor..].len();
		if remaining_len < body_len {
			return Err(OuterEnvelopeError::NotEnoughBuffer(remaining_len, body_len));
		}

		//Write the cursor
		send_buf[cursor..cursor + len_len_tag].copy_from_slice(len_tag_bytes);
		cursor += len_len_tag;

		//Header done, now write the ciphertext.
		send_buf[cursor..cursor + message_len].copy_from_slice(&self.body.ciphertext);
		cursor += message_len;
		Ok(cursor)
	}

	pub fn decode_packet(
		recv_buf: &[u8],
		peer_address: SocketAddr,
	) -> Result<(OuterEnvelope, usize), OuterEnvelopeError> {
		const SESSION_ID_LEN: usize = std::mem::size_of::<SessionId>();
		const COUNTER_LEN: usize = std::mem::size_of::<MessageCounter>();
		// Header will include at least a session ID, a counter, an outer-envelope type variant byte,
		// and at least 1 byte (potentially more) of message length tag (vu64 encoded)
		if recv_buf.len() < SESSION_ID_LEN + COUNTER_LEN + 1 {
			return Err(OuterEnvelopeError::NotEnoughForHeader(
				recv_buf.len(),
				SESSION_ID_LEN + COUNTER_LEN + 1,
			));
		}
		let mut session_id = [0u8; SESSION_ID_LEN];
		let mut counter_bytes = [0u8; COUNTER_LEN];

		// Start by reading the session ID
		let mut cursor = 0;
		session_id.copy_from_slice(&recv_buf[cursor..cursor + SESSION_ID_LEN]);
		cursor += SESSION_ID_LEN;

		// Now, read our sequence number (counter / Noise protocol nonce)
		counter_bytes.copy_from_slice(&recv_buf[cursor..cursor + COUNTER_LEN]);
		cursor += COUNTER_LEN;

		let counter = MessageCounter::from_le_bytes(counter_bytes);

		let session_name = FullSessionName {
			peer_address,
			session_id,
		};

		let first_length_tag_byte: u8 = recv_buf[cursor];
		//Get the length of the vu64 length tag from the first byte.
		let lenlen = vu64::decoded_len(first_length_tag_byte) as usize;
		let message_length = vu64::decode(&recv_buf[cursor..cursor + lenlen])?;
		cursor += lenlen;

		let ciphertext = if message_length > 0 {
			(&recv_buf[cursor..cursor + message_length as usize]).to_vec()
		} else {
			// I intend to write it such that the network layer throws a warning and not an error in response to this.
			return Err(OuterEnvelopeError::ZeroLengthCiphertext(peer_address));
		};
		Ok((
			OuterEnvelope {
				session: session_name.clone(),
				body: CiphertextMessage {
					counter,
					ciphertext,
				},
			},
			cursor + message_length as usize,
		))
	}
}

/// Subset of an OuterEnvelope which cannot be a ProtocolMessage.
#[derive(Debug, Clone)]
pub struct CiphertextEnvelope {
	pub session: FullSessionName,
	pub body: CiphertextMessage,
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
			}
			PacketGuarantees::UnreliableSequenced => match self.stream {
				StreamSelector::Any => Packet::unreliable_sequenced(send_to, self.payload, None),
				StreamSelector::Specific(id) => {
					Packet::unreliable_sequenced(send_to, self.payload, Some(id))
				}
			},
			PacketGuarantees::ReliableUnordered => {
				// Unordered packets have no concept of a "stream"
				Packet::reliable_unordered(send_to, self.payload)
			}
			PacketGuarantees::ReliableOrdered => match self.stream {
				StreamSelector::Any => Packet::reliable_ordered(send_to, self.payload, None),
				StreamSelector::Specific(id) => {
					Packet::reliable_ordered(send_to, self.payload, Some(id))
				}
			},
			PacketGuarantees::ReliableSequenced => match self.stream {
				StreamSelector::Any => Packet::reliable_sequenced(send_to, self.payload, None),
				StreamSelector::Specific(id) => {
					Packet::reliable_sequenced(send_to, self.payload, Some(id))
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

		Ok(PacketIntermediary {
			guarantees: Self::net_msg_guarantees(),
			stream: Self::net_msg_stream(),
			payload: encoded,
		})
	}

	fn decode_from(message: InboundNetMsg) -> Result<(Self, NodeIdentity), NetMsgRecvError> {
		if Self::net_msg_id() != message.message_type_id {
			Err(NetMsgRecvError::WrongType(
				Self::net_msg_id(),
				Self::net_msg_name(),
				message.message_type_id,
			))
		} else {
			let InboundNetMsg {
				peer_identity,
				message_type_id: _,
				payload,
			} = message;
			let payload: Self = rmp_serde::from_read(&payload[..])?;
			Ok((payload, peer_identity))
		}
	}
}
