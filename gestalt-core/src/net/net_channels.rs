use std::marker::PhantomData;

use crate::{
	common::identity::NodeIdentity,
	message::{BroadcastChannel, BroadcastSender, Message, MessageSender, SendError},
	net::{InboundNetMsg, NetMsgDomain},
};

use self::net_send_channel::PACKET_TO_SESSION;

use super::{NetMsg, PacketIntermediary};

pub struct NetSendChannel<T>
where
	T: Send + NetMsg,
{
	pub(in crate::net::net_channels) inner: BroadcastSender<Vec<PacketIntermediary>>,
	//pub(in crate::net::net_channel) peer_addr: SocketAddr,
	_t: PhantomData<T>,
}

impl<T> NetSendChannel<T>
where
	T: Send + NetMsg,
{
	pub fn new(sender: BroadcastSender<Vec<PacketIntermediary>>) -> Self {
		NetSendChannel {
			inner: sender,
			//peer_addr,
			_t: PhantomData::default(),
		}
	}
	pub fn send_untyped(&self, packet: PacketIntermediary) -> Result<(), SendError> {
		self.inner
			.send(vec![packet])
			.map(|_v| ())
			.map_err(|_e| SendError::NoReceivers)
	}

	pub fn send_many<R, V>(&self, messages: Vec<R>) -> Result<(), crate::message::SendError>
	where
		R: Message + Into<T>,
		V: IntoIterator<Item = R>,
	{
		for message in messages {
			let packet = message.into().construct_packet().map_err(|e| {
				SendError::Encode(format!(
					"Could not convert packet of type {} into a packet intermediary: {:?}",
					T::net_msg_name(),
					e
				))
			})?;
			self.send_untyped(packet)
				.map_err(|_e| SendError::NoReceivers)?;
		}
		Ok(())
	}

	pub fn resubscribe<U>(&self) -> NetSendChannel<U>
	where
		U: Send + NetMsg,
	{
		NetSendChannel::new(self.inner.clone())
	}
}

impl<T, R> MessageSender<T> for NetSendChannel<R>
where
	T: Message + Into<R>,
	R: Message + Send + NetMsg,
{
	fn send(&self, message: T) -> Result<(), crate::message::SendError> {
		let packet = message.into().construct_packet().map_err(|e| {
			SendError::Encode(format!(
				"Could not convert packet of type {} into a packet intermediary: {:?}",
				R::net_msg_name(),
				e
			))
		})?;
		self.send_untyped(packet)
			.map_err(|_e| SendError::NoReceivers)
	}
}

pub mod net_send_channel {
	use super::*;

	use crate::{
		common::identity::NodeIdentity,
		message::{
			BroadcastChannel, BroadcastReceiver, DomainMessageSender, DomainSubscribeErr, SendError,
		},
	};

	global_domain_channel!(
		BroadcastChannel,
		PACKET_TO_SESSION,
		Vec<PacketIntermediary>,
		NodeIdentity,
		4096
	);

	// Subscribe
	pub fn subscribe_sender<T>(
		peer: &NodeIdentity,
	) -> Result<NetSendChannel<T>, DomainSubscribeErr<NodeIdentity>>
	where
		T: Clone + Send + NetMsg,
	{
		Ok(NetSendChannel::new(PACKET_TO_SESSION.sender_subscribe(peer)?))
	}
	pub(in crate::net) fn subscribe_receiver(
		peer: &NodeIdentity,
	) -> Result<BroadcastReceiver<Vec<PacketIntermediary>>, DomainSubscribeErr<NodeIdentity>> {
		PACKET_TO_SESSION.receiver_subscribe(peer)
	}

	// Send helpers
	pub fn send_to<T>(message: T, peer: &NodeIdentity) -> Result<(), SendError>
	where
		T: NetMsg,
	{
		let packet = message.construct_packet().map_err(|e| {
			SendError::Encode(format!(
				"Could not convert packet of type {} into a packet intermediary: {:?}",
				T::net_msg_name(),
				e
			))
		})?;

		PACKET_TO_SESSION.send_to(vec![packet], peer)
	}

	pub fn send_to_all<T>(message: T) -> Result<(), SendError>
	where
		T: NetMsg,
	{
		let packet = message.construct_packet().map_err(|e| {
			SendError::Encode(format!(
				"Could not convert packet of type {} into a packet intermediary: {:?}",
				T::net_msg_name(),
				e
			))
		})?;
		PACKET_TO_SESSION.send_to_all(vec![packet])
	}

	pub fn send_to_all_except<T>(message: T, exclude: &NodeIdentity) -> Result<(), SendError>
	where
		T: NetMsg,
	{
		let packet = message.construct_packet().map_err(|e| {
			SendError::Encode(format!(
				"Could not convert packet of type {} into a packet intermediary: {:?}",
				T::net_msg_name(),
				e
			))
		})?;
		PACKET_TO_SESSION.send_to_all_except(vec![packet], exclude)
	}
}

pub const NET_MSG_CHANNEL_CAPACITY: usize = 4096;
global_domain_channel!(
	BroadcastChannel,
	INBOUND_NET_MESSAGES,
	Vec<InboundNetMsg>,
	NetMsgDomain,
	NET_MSG_CHANNEL_CAPACITY
);

pub type InboundMsgSender = BroadcastSender<Vec<InboundNetMsg>>;

pub const CONNECTION_READY_CAPACITY: usize = 1024;

global_channel!(BroadcastChannel, CONNECTED, NodeIdentity, CONNECTION_READY_CAPACITY);

pub mod net_recv_channel {
	use std::marker::PhantomData;

	use crate::{
		common::identity::NodeIdentity,
		message::{BroadcastReceiver, DomainSubscribeErr, MessageReceiver, MessageReceiverAsync},
		net::{netmsg::NetMsgRecvError, InboundNetMsg, NetMsg, NetMsgId},
	};

	use super::INBOUND_NET_MESSAGES;

	pub struct NetMsgReceiver<T> {
		pub inner: BroadcastReceiver<Vec<InboundNetMsg>>,
		_t: PhantomData<T>,
	}
	impl<T: NetMsg> NetMsgReceiver<T> {
		pub fn new(inner: BroadcastReceiver<Vec<InboundNetMsg>>) -> Self {
			NetMsgReceiver {
				inner,
				_t: PhantomData::default(),
			}
		}

		pub(crate) fn decode(
			inbound: Vec<InboundNetMsg>,
		) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> {
			let mut output = Vec::with_capacity(inbound.len());
			for message in inbound {
				if T::net_msg_id() != message.message_type_id {
					return Err(NetMsgRecvError::WrongType(
						T::net_msg_id(),
						T::net_msg_name(),
						message.message_type_id,
					));
				} else {
					let InboundNetMsg {
						peer_identity,
						message_type_id: _,
						payload,
					} = message;
					let payload: T = rmp_serde::from_read(&payload[..])?;
					output.push((peer_identity, payload));
				}
			}
			Ok(output)
		}

		/// Returns an empty vec when no new messages are ready.
		pub fn recv_poll(&mut self) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> {
			match self.inner.recv_poll()? {
				Some(buffer) => Self::decode(buffer),
				None => Ok(Vec::new()),
			}
		}

		pub async fn recv_wait(&mut self) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> {
			Self::decode(self.inner.recv_wait().await?)
		}

		pub fn resubscribe<U>(&self) -> NetMsgReceiver<U>
		where
			U: NetMsg,
		{
			NetMsgReceiver {
				inner: self.inner.resubscribe(),
				_t: PhantomData::default(),
			}
		}
	}

	pub fn subscribe<T>() -> Result<NetMsgReceiver<T>, DomainSubscribeErr<NetMsgId>>
	where
		T: NetMsg,
	{
		INBOUND_NET_MESSAGES.add_domain(&T::net_msg_id());
		INBOUND_NET_MESSAGES
			.receiver_subscribe(&T::net_msg_id())
			.map(|inner| NetMsgReceiver::new(inner))
	}
}

pub fn register_peer(peer: &NodeIdentity) {
	PACKET_TO_SESSION.add_domain(peer);
}
pub fn drop_peer(peer: &NodeIdentity) {
	PACKET_TO_SESSION.drop_domain(peer);
}
