use std::{collections::VecDeque, net::SocketAddr, time::Instant};

use laminar::{Connection, VirtualConnection};
use log::trace;

/// Thin wrapper used to pretend, from the perspective of Laminar,
/// that Noise protocol encryption and async UDP are a transparent synchronous UDP socket.
#[derive(Default)]
pub(in crate::net) struct TransportWrapper {
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
	pub(in crate::net) connection_state: VirtualConnection,
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
	pub fn process_inbound<T: IntoIterator<Item: AsRef<[u8]>>>(
		&mut self,
		inbound_messages: T,
		time: Instant,
	) -> Result<(), LaminarWrapperError> {
		//let mut at_least_one = false;
		let messenger = &mut self.messenger;
		for payload in inbound_messages.into_iter() {
			//at_least_one = true;
			//let was_est = self.connection_state.is_established();
			//Processing inbound
			self.connection_state.process_packet(messenger, payload.as_ref(), time);
			//if !was_est && self.connection_state.is_established() {
			//    info!("Connection established with {:?}", self.peer_address);
			//}
		}

		self.connection_state.update(messenger, time);

		//if at_least_one {
		//    self.connection_state.last_heard = time.clone();
		//}

		match self.connection_state.should_drop(messenger, time) {
			false => Ok(()),
			true => {
				trace!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", self.connection_state.packets_in_flight(), self.connection_state.last_heard(Instant::now()), self.connection_state.is_established());
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
				trace!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", self.connection_state.packets_in_flight(), self.connection_state.last_heard(Instant::now()), self.connection_state.is_established());
				Err(LaminarWrapperError::Disconnect(self.peer_address))
			}
		}
	}
	/// Adds Laminar connection logic to messages that we are sending.
	pub fn process_outbound<T: IntoIterator<Item = laminar::Packet>>(
		&mut self,
		outbound_messages: T,
		time: Instant,
	) -> Result<(), LaminarWrapperError> {
		let messenger = &mut self.messenger;
		// Return before attempting to send.
		if self.connection_state.should_drop(messenger, time) {
			return Err(LaminarWrapperError::Disconnect(self.peer_address));
		}

		// To send:
		for packet in outbound_messages.into_iter() {
			self.connection_state.process_event(messenger, packet, time);
		}
		self.connection_state.update(messenger, time);

		// Check again!
		match self.connection_state.should_drop(messenger, time) {
			false => Ok(()),
			true => {
				trace!("Connection indicated as should_drop(). packets_in_flight() is {} and last_heard() is {:?}. Established? : {}", self.connection_state.packets_in_flight(), self.connection_state.last_heard(Instant::now()), self.connection_state.is_established());
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
