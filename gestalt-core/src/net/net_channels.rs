use std::marker::PhantomData;

use crate::{
	common::identity::NodeIdentity,
	message::{BroadcastChannel, BroadcastSender, Message, MessageSender, SendError},
	net::{InboundNetMsg, NetMsgDomain},
};

use self::net_send_channel::PACKET_TO_SESSION;

use super::{NetMsg, PacketIntermediary};

pub struct NetSessionSender<T>
where
	T: Send + NetMsg,
{
	pub(in crate::net::net_channels) inner: BroadcastSender<Vec<PacketIntermediary>>,
	//pub(in crate::net::net_channel) peer_addr: SocketAddr,
	_t: PhantomData<T>,
}

impl<T> NetSessionSender<T>
where
	T: Send + NetMsg,
{
	pub fn new(sender: BroadcastSender<Vec<PacketIntermediary>>) -> Self {
		NetSessionSender {
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

	pub fn resubscribe<U>(&self) -> NetSessionSender<U>
	where
		U: Send + NetMsg,
	{
		NetSessionSender::new(self.inner.clone())
	}
}

impl<T, R> MessageSender<T> for NetSessionSender<R>
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
