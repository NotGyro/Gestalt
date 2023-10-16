use std::{
	collections::HashMap,
	net::{IpAddr, SocketAddr},
	time::{Duration, Instant},
};

use laminar::ConnectionMessenger;
use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc, time::MissedTickBehavior};

use crate::{
	common::{
		identity::{IdentityKeyPair, NodeIdentity},
		new_fast_hash_map, new_fast_hash_set, FastHashMap, FastHashSet,
	},
	message::{BroadcastReceiver, MessageReceiverAsync},
	net::{InboundNetMsg, NetMsgId, DISCONNECT_RESERVED},
};

use super::{
	generated,
	net_channels::{InboundMsgSender, INBOUND_NET_MESSAGES},
	netmsg::{CiphertextEnvelope, CiphertextMessage, MessageSidedness},
	reliable_udp::{LaminarConfig, LaminarConnectionManager, LaminarWrapperError},
	MessageCounter, NetMsgDomain, OuterEnvelope, PacketIntermediary, SelfNetworkRole,
	SuccessfulConnect,
};

pub const SESSION_ID_LEN: usize = 4;
pub type SessionId = [u8; SESSION_ID_LEN];

/// Runtime information specifying what kind of connection we are looking at.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ConnectionRole {
	/// We are the server and we are connected to a client.
	ServerToClient,
	/// We are the client and we are connected to a server.
	ClientToServer,
}

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
	#[error(
		"Laminar asked to send a packet to {0:?} but this session is a communicating with {1:?}"
	)]
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
	#[error(
		"Peer {0:?} sent a Laminar \"connect\" message after the session was already started!"
	)]
	ConnectAfterStarted(SocketAddr),
	#[error("Variable-length integer could not be decoded: {0:?}")]
	VarIntError(#[from] vu64::Error),
	#[error("A NetMessage of type {0} has been receved from {1}, but no type has been associated with this ID in the engine. \n It's possible this peer is using a newer version of Gestalt.")]
	UnrecognizedMsg(NetMsgId, String),
	#[error("A NetMessage of type {0} has been receved from {1}, but we are a {2:?} and this message's sidedness is a {3:?}.")]
	WrongSidedness(NetMsgId, String, SelfNetworkRole, MessageSidedness),
	#[error(
		"Counter for a session with {0:?} is at the maximum value for a 4-byte unsized integer!"
	)]
	ExhaustedCounter(SocketAddr),
}

pub type PushSender = mpsc::UnboundedSender<Vec<OuterEnvelope>>;
pub type PushReceiver = mpsc::UnboundedReceiver<Vec<OuterEnvelope>>;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[netmsg(DISCONNECT_RESERVED, Common, ReliableUnordered)]
pub struct DisconnectMsg {}

/// One per session, handles both cryptography and Laminar reliable-UDP logic.
pub struct Session {
	/// Handles reliability-over-UDP.
	pub laminar: LaminarConnectionManager,
	pub local_role: SelfNetworkRole,
	pub local_identity: IdentityKeyPair,
	pub peer_identity: NodeIdentity,
	pub peer_address: SocketAddr,

	pub session_id: SessionId,
	/// Counter we put on outgoing `OuterEnvelope`s, should increase monotonically.
	pub local_counter: MessageCounter,
	pub transport_cryptography: snow::StatelessTransportState,

	/// Channel the Session uses to send packets to the UDP socket
	push_channel: PushSender,

	/// Cached sender handles so we don't have to lock the mutex every time we want to send a message.
	inbound_channels: FastHashMap<NetMsgDomain, InboundMsgSender>,

	pub disconnect_deliberate: bool,

	/// Valid NetMsg types for our network role.
	valid_incoming_messages: FastHashSet<NetMsgId>,
}

impl Session {
	/// Get a message-passing sender for the given NetMsgDomain, caching so we don't have to lock the mutex constantly.
	fn get_or_susbscribe_inbound_sender(&mut self, domain: NetMsgDomain) -> &mut InboundMsgSender {
		self.inbound_channels.entry(domain).or_insert_with(|| {
			//add_domain(&INBOUND_NET_MESSAGES, &domain);
			INBOUND_NET_MESSAGES.sender_subscribe(&domain).unwrap()
		})
	}

	pub fn new(
		local_identity: IdentityKeyPair,
		local_role: SelfNetworkRole,
		peer_address: SocketAddr,
		connection: SuccessfulConnect,
		laminar_config: LaminarConfig,
		push_channel: PushSender,
		time: Instant,
	) -> Self {
		let mut laminar_layer =
			LaminarConnectionManager::new(connection.peer_address, &laminar_config, time);
		laminar_layer.connection_state.last_heard = time;

		let mut valid_incoming_messages = new_fast_hash_set();
		for id in generated::get_netmsg_table().iter().filter_map(|v| {
			let (id, info) = v;
			if local_role.should_we_ingest(&info.sidedness) {
				Some(*id)
			} else {
				None
			}
		}) {
			valid_incoming_messages.insert(id);
		}

		Session {
			laminar: laminar_layer,
			local_identity,
			local_role,
			peer_identity: connection.peer_identity,
			peer_address,
			session_id: connection.session_id,
			local_counter: connection.transport_counter,
			transport_cryptography: connection.transport_cryptography,
			push_channel,
			inbound_channels: new_fast_hash_map(),
			valid_incoming_messages,
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
	fn encrypt_packet<T: AsRef<[u8]>>(
		&mut self,
		plaintext: T,
	) -> Result<OuterEnvelope, SessionLayerError> {
		self.local_counter
			.checked_add(1)
			.ok_or(SessionLayerError::ExhaustedCounter(self.peer_address.clone()))?;
		let mut buffer = vec![0u8; ((plaintext.as_ref().len() as usize) * 3) + 64];
		let len_written = self.transport_cryptography.write_message(
			self.local_counter as u64,
			plaintext.as_ref(),
			&mut buffer,
		)?;
		buffer.truncate(len_written);
		let full_session_name = self.get_session_name();
		Ok(OuterEnvelope {
			session: full_session_name,
			body: CiphertextMessage {
				counter: self.local_counter,
				ciphertext: buffer.to_vec(),
			},
		})
	}

	/// Called inside process_inbound()
	fn decrypt_envelope(
		&mut self,
		envelope: CiphertextEnvelope,
	) -> Result<Vec<u8>, SessionLayerError> {
		let CiphertextEnvelope {
			session: _session_id,
			body: CiphertextMessage {
				counter,
				ciphertext,
			},
		} = envelope;

		let mut buf = vec![0u8; (ciphertext.len() * 3) / 2];
		let len_read =
			self.transport_cryptography
				.read_message(counter as u64, &ciphertext, &mut buf)?;
		buf.truncate(len_read);
		Ok(buf)
	}

	/// Ingests a batch of packets coming off the wire.
	pub fn ingest_packets<T: IntoIterator<Item = CiphertextEnvelope>>(
		&mut self,
		inbound_messages: T,
		time: Instant,
	) -> Vec<SessionLayerError> {
		let mut errors: Vec<SessionLayerError> = Vec::default();

		let mut batch: Vec<Vec<u8>> = Vec::default();
		for envelope in inbound_messages.into_iter() {
			match self.decrypt_envelope(envelope) {
				Ok(packet_contents) => batch.push(packet_contents),
				Err(e) => errors.push(e),
			}
		}

		match self.laminar.process_inbound(batch, time) {
			Ok(_) => {}
			Err(e) => errors.push(e.into()),
		}

		//Packets to send to the rest of the Gestalt application, having been decoded.
		let processed_packets: Vec<laminar::SocketEvent> = self.laminar.empty_inbox();

		//Now that we've handled those, convert.
		//Batch them according to ID.
		let mut finished_packets: HashMap<NetMsgId, Vec<InboundNetMsg>> = HashMap::new();
		for evt in processed_packets {
			match evt {
				laminar::SocketEvent::Packet(pkt) => {
					// How long is our varint?
					let message_type_first_byte = pkt.payload()[0];
					let message_type_len = vu64::decoded_len(message_type_first_byte);
					match vu64::decode_with_length(
						message_type_len,
						&pkt.payload()[0..message_type_len as usize],
					) {
						Ok(message_type_id) => {
							let message_type_id = message_type_id as NetMsgId;
							trace!(
								"Decoding a NetMsg from {} with message_type_id {}",
								self.peer_identity.to_base64(),
								message_type_id
							);
							let message = InboundNetMsg {
								message_type_id,
								payload: pkt.payload()[message_type_len as usize..].to_vec(),
								peer_identity: self.peer_identity.clone(),
							};
							if finished_packets.get(&message_type_id).is_none() {
								finished_packets.insert(message_type_id, Vec::default());
							}
							finished_packets
								.get_mut(&message_type_id)
								.unwrap()
								.push(message);
						}
						Err(e) => errors.push(e.into()),
					}
				}
				laminar::SocketEvent::Timeout(addr) => {
					errors.push(SessionLayerError::LaminarTimeout(addr.clone()))
				}
				laminar::SocketEvent::Disconnect(addr) => {
					errors.push(SessionLayerError::LaminarDisconnect(addr.clone()))
				}
				laminar::SocketEvent::Connect(addr) => {
					//self.laminar.connection_state.last_heard = time;
					trace!("Connection marked established with {:?}", addr);
				}
			}
		}
		// Push our messages out to the rest of the application.
		for (message_type, message_buf) in finished_packets {
			if self.valid_incoming_messages.contains(&message_type) {
				match message_type {
					// Handle network-subsystem builtin messages
					DISCONNECT_RESERVED => {
						info!(
							"Peer {} has disconnected (deliberately - this is not an error)",
							self.peer_identity.to_base64()
						);
						self.disconnect_deliberate = true;
					}
					// Handle messages meant to go out into the rest of the engine.
					_ => {
						//Non-reserved, game-defined net msg IDs.
						let channel = self.get_or_susbscribe_inbound_sender(message_type);
						match channel
							.send(message_buf)
							.map_err(|e| SessionLayerError::SendBroadcastError(e))
						{
							Ok(_x) => {
								trace!("Successfully just sent a NetMsg from {} of type {} from the session to the rest of the engine.", self.peer_identity.to_base64(), message_type);
							}
							Err(e) => errors.push(e),
						}
					}
				}
			} else {
				errors.push(match generated::get_netmsg_table().get(&message_type) {
					Some(info) => SessionLayerError::WrongSidedness(
						message_type,
						self.peer_identity.to_base64(),
						self.local_role,
						info.sidedness.clone(),
					),
					None => SessionLayerError::UnrecognizedMsg(
						message_type,
						self.peer_identity.to_base64(),
					),
				});
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

		//Send to UDP socket.
		match self.push_channel.send(processed_reply_buf) {
			Ok(()) => {}
			Err(e) => errors.push(e.into()),
		}

		errors
	}

	pub fn process_update(&mut self, time: Instant) -> Result<(), SessionLayerError> {
		let mut errors: Vec<SessionLayerError> = Vec::default();
		match self.laminar.process_update(time) {
			Ok(()) => {}
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

		//Send to UDP socket.
		match self.push_channel.send(processed_send) {
			Ok(()) => {}
			Err(e) => errors.push(e.into()),
		}

		// Result / output
		match errors.len() {
			0 => Ok(()),
			1 => Err(errors.pop().unwrap()),
			_ => Err(SessionLayerError::ErrorBatch(errors)),
		}
	}

	/// Adds Laminar connection logic to messages that we are sending.
	pub fn process_outbound<T: IntoIterator<Item = laminar::Packet>>(
		&mut self,
		outbound_messages: T,
		time: Instant,
	) -> Result<(), SessionLayerError> {
		let mut errors: Vec<SessionLayerError> = Vec::default();
		match self.laminar.process_outbound(outbound_messages, time) {
			Ok(()) => {}
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
			Ok(()) => {}
			Err(e) => errors.push(e.into()),
		}

		// Result / output
		match errors.len() {
			0 => Ok(()),
			1 => Err(errors.pop().unwrap()),
			_ => Err(SessionLayerError::ErrorBatch(errors)),
		}
	}

	/// Network connection CPR.
	pub fn force_heartbeat(&mut self) -> Result<(), laminar::error::ErrorKind> {
		let packets = self.laminar.connection_state.process_outgoing(
			laminar::packet::PacketInfo::heartbeat_packet(&[]),
			None,
			Instant::now(),
		)?;
		for packet in packets {
			self.laminar
				.messenger
				.send_packet(&self.peer_address, &packet.contents());
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
/// * `session_tick` - Interval between times we examine if we should send heartbeat packets, resend lost packets, etc.  
///
pub async fn handle_session(
	mut session_manager: Session,
	mut incoming_packets: mpsc::UnboundedReceiver<Vec<CiphertextEnvelope>>,
	mut from_game: BroadcastReceiver<Vec<PacketIntermediary>>,
	session_tick: Duration,
	kill_from_inside: mpsc::UnboundedSender<(FullSessionName, Vec<SessionLayerError>)>,
	mut kill_from_outside: tokio::sync::oneshot::Receiver<()>,
) {
	let mut ticker = tokio::time::interval(session_tick);
	ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
	info!("Handling session for peer {}...", session_manager.peer_identity.to_base64());

	let peer_address = session_manager.peer_address.clone();
	loop {
		tokio::select! {
			// Inbound packets
			// Per tokio documentation - "This method is cancel safe. If recv is used as the event in a tokio::select! statement and some other branch completes first, it is guaranteed that no messages were received on this channel."
			inbound_packets_maybe = (&mut incoming_packets).recv() => {
				match inbound_packets_maybe {
					Some(inbound_packets) => {
						let ingest_results = session_manager.ingest_packets(inbound_packets, Instant::now());
						if !ingest_results.is_empty() {
							let mut built_string = String::default();
							for errorout in ingest_results.iter() {
								let to_append = format!("* {} \n", errorout);
								built_string.push_str(to_append.as_str());
							}
							error!("Errors encountered parsing inbound packets in a session with {}: \n {}", session_manager.peer_identity.to_base64(), built_string);
							kill_from_inside.send((session_manager.get_session_name() ,  ingest_results)).unwrap();
							break;
						}
					},
					None => {
						info!("Connection closed for {}, dropping session state.", session_manager.peer_identity.to_base64());
						kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
						break;
					}
				}
				if session_manager.disconnect_deliberate {
					kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
					break;
				}
			},
			send_packets_maybe = (&mut from_game).recv_wait() => {
				match send_packets_maybe {
					Ok(send_packets) => {
						session_manager.laminar.connection_state.record_send();
						let serialize_results = session_manager.process_outbound(send_packets.into_iter().map(|intermediary| intermediary.make_full_packet(peer_address)), Instant::now());
						if let Err(e) = serialize_results {
							error!("Error encountered attempting to send a packet to peer {}: {:?}", session_manager.peer_identity.to_base64(), e);
							kill_from_inside.send((session_manager.get_session_name(), vec![e])).unwrap();
							break;
						}
					},
					Err(e) => {
						info!("Connection closed for {} due to {:?}, dropping session state.", session_manager.peer_identity.to_base64(), e);
						kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
						break;
					}
				}
				if session_manager.disconnect_deliberate {
					kill_from_inside.send((session_manager.get_session_name(), vec![])).unwrap();
					break;
				}
			},
			_ = (&mut ticker).tick() => {
				let update_results = session_manager.process_update(Instant::now());
				if let Err(e) = update_results {
					trace!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", session_manager.laminar.connection_state.packets_in_flight(), session_manager.laminar.connection_state.last_heard(Instant::now()), session_manager.laminar.connection_state.is_established());
					error!("Error encountered while ticking network connection to peer {}: {:?}", session_manager.peer_identity.to_base64(), e);
					kill_from_inside.send((session_manager.get_session_name(), vec![e])).unwrap();
					break;
				}
			}
			_ = (&mut kill_from_outside) => {
				info!("Shutting down session with user {}", session_manager.peer_identity.to_base64() );
				break;
			}
		}
	}
	//error!("A session manager for a session between {} (us) and {} (peer) has stopped looping.", session_manager.local_identity.public.to_base64(), session_manager.peer_identity.to_base64());
}
