use std::marker::PhantomData;

use gestalt_proc_macros::ChannelSet;

use crate::{
	common::identity::NodeIdentity, message::{MessageSender, MpscSender, SendError}, AllAndOneReceiver, AllAndOneSender, BroadcastChannel, BroadcastReceiver, BroadcastSender, ChannelCapacityConf, ChannelInit, DomainBroadcastChannel, DomainMultiChannel, DomainSenderSubscribe, DomainSubscribeErr, MessageReceiver, MessageReceiverAsync, MpscChannel, MpscReceiver, NewDomainErr, SenderChannel, StaticChannelAtom
};

use super::{netmsg::NetMsgRecvError, ConnectAnnounce, InboundNetMsg, NetMsg, NetMsgDomain, NetMsgId, OuterEnvelope, PacketIntermediary, SuccessfulConnect};

pub type OutboundNetMsgs = Vec<PacketIntermediary>;
pub(super) type NetInnerSender = AllAndOneSender<OutboundNetMsgs, NodeIdentity, MpscSender<OutboundNetMsgs>>;

pub struct NetMsgSender {
	pub(in crate::net::net_channels) inner: NetInnerSender,
}
impl NetMsgSender {
	pub fn new(sender: NetInnerSender) -> Self {
		NetMsgSender {
			inner: sender,
		}
	}
	pub fn send_untyped(&self, packet: PacketIntermediary) -> Result<(), SendError> {
		self.inner
			.send(vec![packet])
			.map(|_v| ())
			.map_err(|_e| SendError::NoReceivers)
	}
	fn encode_packet<R>(message: R) -> Result<PacketIntermediary, crate::message::SendError> where R: NetMsg { 
		message.construct_packet().map_err(|e| {
			SendError::Encode(format!(
				"Could not convert packet of type {} into a packet intermediary: {:?}",
				R::net_msg_name(),
				e
			))
		})
	}

	pub fn send_one<R>(&self, message: R) -> Result<(), crate::message::SendError> where R: NetMsg {
		let packet = Self::encode_packet(message)?;
		self.send_untyped(packet)
	}

	fn many_encode<R, V>(messages: V) -> Result<Vec<PacketIntermediary>, crate::message::SendError> 
	where
		V: IntoIterator<Item = R>,
		R: NetMsg {
		let mut encoded = Vec::new();

		for message in messages {
			let packet = Self::encode_packet(message)?;
			encoded.push(packet);
		}
		Ok(encoded)
	}

	pub fn send_many<R, V>(&self, messages: V) -> Result<(), crate::message::SendError>
	where
		V: IntoIterator<Item = R>,
		R: NetMsg
	{
		self.inner
			.send(Self::many_encode(messages)?)
			.map(|_v| ())
	}

	pub fn send_to_all<R>(&self, message: R) -> Result<(), crate::message::SendError> where R: NetMsg {
		let packet = Self::encode_packet(message)?;
		self.inner.send_to_all(vec![packet])
			.map(|_| ())
	}
	pub fn send_to_all_except<R>(&self, message: R, exclude: &NodeIdentity) -> Result<(), crate::message::SendError> where R: NetMsg {
		let packet = Self::encode_packet(message)?;
		self.inner.send_to_all_except(vec![packet], exclude)
			.map(|_| ())
	}

	pub fn send_many_to_all<R, V>(&self, messages: V) -> Result<(), crate::message::SendError> 
	where 
		R: NetMsg, 
		V: IntoIterator<Item = R>, { 
		let packets = Self::many_encode(messages)?;
		self.inner.send_to_all(packets)
			.map(|_| ())
	}
	pub fn send_many_to_all_except<R, V>(&self, messages: V) -> Result<(), crate::message::SendError>
	where 
		R: NetMsg, 
		V: IntoIterator<Item = R>, {
		let packets = Self::many_encode(messages)?;
		self.inner.send_to_all(packets)
			.map(|_| ())
	}

	pub fn resubscribe(&self) -> NetMsgSender {
		NetMsgSender::new(self.inner.clone())
	}
}

impl<T> MessageSender<T> for NetMsgSender where T: NetMsg + std::fmt::Debug {
	fn send(&self, message: T) -> Result<(), SendError> {
		self.send_one(message)
	}
}

pub type OutboundNetMsgReceiver = AllAndOneReceiver<OutboundNetMsgs, NodeIdentity, MpscReceiver<OutboundNetMsgs>>;

/// Channel for sending packets out from this node to connected peers. 
#[derive(Clone)]
pub struct NetSendChannel { 
	inner: DomainBroadcastChannel<OutboundNetMsgs, NodeIdentity, MpscChannel<OutboundNetMsgs>>
}

impl ChannelInit for NetSendChannel {
	fn new(capacity: usize) -> Self {
		Self { 
			inner: DomainBroadcastChannel::new(capacity),
		}
	}
}

impl NetSendChannel { 
	pub fn register_peer(&self, peer: NodeIdentity) -> Result<OutboundNetMsgReceiver, NewDomainErr<NodeIdentity>> {
		let new_channel = MpscChannel::new(self.inner.get_capacity());
		let recv = new_channel.take_receiver()
			.expect("Should be impossible to lack a retained_receiver for a newly-created MpscChannel");
		self.inner.add_channel(peer.clone(), new_channel)?;

		return Ok(AllAndOneReceiver::new(recv, self.inner.receiver_subscribe_all(), peer));
	}

	pub fn drop_peer(&self, peer: &NodeIdentity) {
		self.inner.drop_domain(peer);
	}

	pub fn get_capacity(&self) -> usize { 
		self.inner.get_capacity()
	}
}

impl<T> SenderChannel<T> for NetSendChannel where T: NetMsg + std::fmt::Debug {
	type Sender = NetMsgSender;
}

impl<T> DomainSenderSubscribe<T, NodeIdentity> for NetSendChannel where T: NetMsg + std::fmt::Debug {
	fn sender_subscribe_domain(&self, domain: &NodeIdentity) -> Result<Self::Sender, crate::DomainSubscribeErr<NodeIdentity>> {
		Ok(NetMsgSender::new(self.inner.sender_subscribe(domain)?))
	}
}

pub type InboundNetMsgs = Vec<InboundNetMsg>;
pub type InboundNetChannel = DomainMultiChannel<InboundNetMsgs, NetMsgDomain, BroadcastChannel<InboundNetMsgs>>;

impl InboundNetChannel {
	pub fn receiver_typed<T: NetMsg>(&self) -> Result<NetMsgReceiver<T>, DomainSubscribeErr<NetMsgId>> {
		NetMsgReceiver::subscribe(self)
	}
}

pub struct NetMsgReceiver<T> where T: NetMsg { 
	inner: BroadcastReceiver<InboundNetMsgs>,
	_marker: PhantomData<T>,
}

impl<T: NetMsg> NetMsgReceiver<T> {
	pub fn subscribe(channel: &InboundNetChannel) -> Result<Self, DomainSubscribeErr<NetMsgId>> {
		Ok(
			Self { 
				inner: channel.receiver_subscribe(&T::net_msg_id())?,
				_marker: PhantomData,
			}
		)
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

	pub async fn recv_wait(&mut self) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> {
		Self::decode(self.inner.recv_wait().await?)
	}

	pub fn resubscribe<U>(&self) -> NetMsgReceiver<U>
	where
		U: NetMsg,
	{
		NetMsgReceiver {
			inner: self.inner.resubscribe(),
			_marker: PhantomData::default(),
		}
	}
}

impl<T> MessageReceiver<Vec<(NodeIdentity, T)>> for NetMsgReceiver<T> where T: NetMsg + std::fmt::Debug {
	fn recv_poll(&mut self) -> Result<Option<Vec<(NodeIdentity, T)>>, crate::RecvError> {
		let resl = self.inner.recv_poll()?;
		let messages = if let Some(messages) = resl { messages } else { return Ok(None) };
		Self::decode(messages)
			.map(|v| Some(v))
			.map_err(|e| crate::RecvError::Other(format!("Unable to decode netmessages: {e}")))
	}
}

static_channel_atom!(NetMsgOutbound, NetSendChannel, OutboundNetMsgs, NodeIdentity, 4096);
static_channel_atom!(NetMsgInbound, InboundNetChannel, InboundNetMsgs, NetMsgDomain, 4096);

static_channel_atom!(ConnectInternal, MpscChannel<SuccessfulConnect>, SuccessfulConnect, 4096);

static_channel_atom!(ConnectionReady, BroadcastChannel<ConnectAnnounce>, ConnectAnnounce, 4096);
static_channel_atom!(DisconnectAnnounce, BroadcastChannel<ConnectAnnounce>, ConnectAnnounce, 4096);

pub type OutboundRawPackets = Vec<OuterEnvelope>;
pub type OutboundPacketChannel = MpscChannel<OutboundRawPackets>;
// Session-to-packethandler, used for session objects to push fully-encoded outer envelopes to send over the socket.
static_channel_atom!(PacketPush, OutboundPacketChannel, OutboundRawPackets, 4096);

static_channel_atom!(ProtocolKeyMismatchReporter, BroadcastChannel<NodeIdentity>, NodeIdentity, 4096);
static_channel_atom!(ProtocolKeyMismatchApprover, BroadcastChannel<(NodeIdentity, bool)>, (NodeIdentity, bool), 4096);

/// What Main needs to init for engine <-> net communication. 
#[derive(ChannelSet, Clone)]
pub struct EngineNetChannels {
	/// Game-to-network. Outbound i.e. outbound from game
	#[channel(NetMsgOutbound)]
	pub net_msg_outbound: <NetMsgOutbound as StaticChannelAtom>::Channel,
	/// Network-to-game. Inbound i.e. inbound from net.
	#[channel(NetMsgInbound)]
	pub net_msg_inbound: <NetMsgInbound as StaticChannelAtom>::Channel,
	#[channel(ConnectInternal)]
	pub internal_connect: <ConnectInternal as StaticChannelAtom>::Channel,
	#[channel(ConnectionReady)]
	pub peer_connected: <ConnectionReady as StaticChannelAtom>::Channel,
	#[channel(ProtocolKeyMismatchReporter)]
	pub key_mismatch_reporter: <ProtocolKeyMismatchReporter as StaticChannelAtom>::Channel,
	#[channel(ProtocolKeyMismatchApprover)]
	pub key_mismatch_approver: <ProtocolKeyMismatchApprover as StaticChannelAtom>::Channel,
}
// TODO: Do some more proc macro nonsense but around init this time so this isn't so boilerplatey.
impl EngineNetChannels {
	pub fn new(conf: &ChannelCapacityConf) -> Self {
		Self {
			net_msg_outbound: NetSendChannel::new(conf.get_or_default::<NetMsgOutbound>()),
			net_msg_inbound: InboundNetChannel::new(conf.get_or_default::<NetMsgInbound>()),
			internal_connect: MpscChannel::new(conf.get_or_default::<ConnectInternal>()),
			peer_connected: BroadcastChannel::new(conf.get_or_default::<ConnectionReady>()),
			key_mismatch_reporter: BroadcastChannel::new(conf.get_or_default::<ProtocolKeyMismatchReporter>()),
			key_mismatch_approver: BroadcastChannel::new(conf.get_or_default::<ProtocolKeyMismatchApprover>()),
		}
	}
}

/// Net-system-sided channels, intended to subset EngineNetChannels. 
#[derive(ChannelSet)]
pub struct NetSystemChannels {
	//pub raw_to_session:
	/// Game-to-network. Outbound i.e. outbound from game
	#[channel(NetMsgOutbound)]
	pub net_msg_outbound: <NetMsgOutbound as StaticChannelAtom>::Channel,
	/// Network-to-game. Inbound i.e. inbound from net.
	#[channel(NetMsgInbound)]
	pub net_msg_inbound: <NetMsgInbound as StaticChannelAtom>::Channel,
	/// Successful connections after protocol negotiation & handshake.
	pub recv_internal_connections: MpscReceiver<SuccessfulConnect>,
	#[channel(ConnectionReady)]
	pub announce_connection: BroadcastChannel<ConnectAnnounce>,
	/// Net-system-internal, used to push OuterEnvelopes from session to socket.
	#[channel(PacketPush, new_channel)]
	pub from_session: <PacketPush as StaticChannelAtom>::Channel,
}

/// Channels used by one session object, intended to subset NetSystemChannels.
#[derive(ChannelSet)]
pub struct SessionChannels {
	/// Sent by game, received by session.
	/// Can't be auto-subset'd because MpscChannels are particular
	pub net_msg_outbound: OutboundNetMsgReceiver,
	/// Network-to-game. Inbound i.e. inbound from net.
	#[channel(NetMsgInbound)]
	pub to_engine_sender: <NetMsgInbound as StaticChannelAtom>::Channel,
	/// ConnectionReady is sent as soon as our session object has decided that it's safe
	/// to tell the rest of the engine that this connection has occurred.
	#[sender(ConnectionReady)]
	pub announce_connection: BroadcastSender<ConnectAnnounce>,
	/// Net-system-internal, used by sessions to give ready packets to the packet handler.
	#[sender(PacketPush)]
	pub push_sender: MpscSender<OutboundRawPackets>,
}

/// Channels required to do protocol negotiation and handshake,
/// intended to subset EngineNetChannels.
#[derive(ChannelSet, Clone)]
pub struct PreprotocolChannels {
	#[channel(ConnectInternal)]
	pub internal_connect: <ConnectInternal as StaticChannelAtom>::Channel,
	#[channel(ProtocolKeyMismatchReporter)]
	pub key_mismatch_reporter: <ProtocolKeyMismatchReporter as StaticChannelAtom>::Channel,
	#[channel(ProtocolKeyMismatchApprover)]
	pub key_mismatch_approver: <ProtocolKeyMismatchApprover as StaticChannelAtom>::Channel,
}

/// Channels required to do protocol negotiation and handshake,
/// intended to subset PreprotocolChannels.
#[derive(ChannelSet)]
pub struct PreprotocolSessionChannels {
	#[sender(ConnectInternal)]
	pub internal_connect: MpscSender<SuccessfulConnect>,
	#[sender(ProtocolKeyMismatchReporter)]
	pub key_mismatch_reporter: BroadcastSender<NodeIdentity>,
	#[receiver(ProtocolKeyMismatchApprover)]
	pub key_mismatch_approver: BroadcastReceiver<(NodeIdentity, bool)>,
}
