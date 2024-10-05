use std::marker::PhantomData;

use gestalt_proc_macros::ChannelSet;

use crate::{
	common::identity::NodeIdentity, message::{MessageSender, MpscSender, SendError}, BroadcastChannel, BroadcastReceiver, BroadcastSender, ChannelCapacityConf, ChannelInit, DomainMessageSender, DomainMultiChannel, DomainSenderSubscribe, DomainSubscribeErr, DomainTakeReceiver, MessageReceiver, MessageReceiverAsync, MpscChannel, MpscReceiver, MultiDomainSender, NewDomainErr, ReceiverChannel, SenderChannel, StaticChannelAtom
};

use super::{netmsg::{CiphertextEnvelope, NetMsgRecvError}, ConnectAnnounce, FullSessionName, InboundNetMsg, NetMsg, NetMsgDomain, NetMsgId, OuterEnvelope, PacketIntermediary, SessionLayerError, SuccessfulConnect};

pub type OutboundNetMsgs = Vec<PacketIntermediary>;
pub(super) type NetInnerSender = MpscSender<OutboundNetMsgs>;

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
	pub fn send_many_untyped(&self, packets: Vec<PacketIntermediary>) -> Result<(), SendError> {
		self.inner
			.send(packets)
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

	pub fn resubscribe(&self) -> NetMsgSender {
		NetMsgSender::new(self.inner.clone())
	}
}

impl MessageSender<PacketIntermediary> for NetMsgSender {
	fn send(&self, message: PacketIntermediary) -> Result<(), SendError> {
		self.send_untyped(message)
	}
}
impl MessageSender<Vec<PacketIntermediary>> for NetMsgSender {
	fn send(&self, message: Vec<PacketIntermediary>) -> Result<(), SendError> {
		self.send_many_untyped(message)
	}
}

pub type OutboundNetMsgReceiver = MpscReceiver<OutboundNetMsgs>;

/// Channel for sending packets out from this node to connected peers. 
#[derive(Clone)]
pub struct NetSendChannel { 
	inner: DomainMultiChannel<OutboundNetMsgs, NodeIdentity, MpscChannel<OutboundNetMsgs>>
}

impl ChannelInit for NetSendChannel {
	fn new(capacity: usize) -> Self {
		Self { 
			inner: DomainMultiChannel::new(capacity),
		}
	}
}

impl NetSendChannel { 
	pub fn register_peer(&self, peer: NodeIdentity) -> Result<OutboundNetMsgReceiver, NewDomainErr<NodeIdentity>> {
		let new_channel = MpscChannel::new(self.inner.get_capacity());
		let recv = new_channel.take_receiver()
			.expect("Should be impossible to lack a retained_receiver for a newly-created MpscChannel");
		self.inner.add_channel(peer.clone(), new_channel)?;

		return Ok(recv);
	}
	pub fn init_peer(&self, peer: NodeIdentity) {
		let new_channel = MpscChannel::new(self.inner.get_capacity());
		// Get-or-init pattern i.e. if this already existed it's fine.
		let _ = self.inner.add_channel(peer.clone(), new_channel);
	}

	pub fn drop_peer(&self, peer: &NodeIdentity) {
		self.inner.drop_domain(peer);
	}

	pub fn get_capacity(&self) -> usize { 
		self.inner.get_capacity()
	}

	pub fn sender_subscribe_all(&self) -> MultiDomainSender<OutboundNetMsgs, NodeIdentity, MpscChannel<OutboundNetMsgs>> { 
		self.inner.sender_subscribe_all()
	}

	//pub fn sender_subscribe_all(&self) -> BroadcastSender<MessageIgnoreEndpoint<OutboundNetMsgs, NodeIdentity>> {
	//	self.inner.sender_subscribe_all()
	//}
}

impl ReceiverChannel<OutboundNetMsgs> for NetSendChannel {
	type Receiver = OutboundNetMsgReceiver;
}

impl SenderChannel<PacketIntermediary> for NetSendChannel {
	type Sender = NetMsgSender;
}

impl DomainSenderSubscribe<PacketIntermediary, NodeIdentity> for NetSendChannel {
	fn sender_subscribe_domain(&self, domain: &NodeIdentity) -> Result<Self::Sender, crate::DomainSubscribeErr<NodeIdentity>> {
		Ok(NetMsgSender::new(self.inner.sender_subscribe(domain)?))
	}
}

impl DomainTakeReceiver<OutboundNetMsgs, NodeIdentity> for NetSendChannel {
	fn take_receiver_domain(&self, domain: &NodeIdentity) -> Result<Self::Receiver, DomainSubscribeErr<NodeIdentity>> {
		self.inner.take_receiver_domain(domain)
	}
}

impl DomainMessageSender<OutboundNetMsgs, NodeIdentity> for NetSendChannel {
	fn send_to(&self, message: OutboundNetMsgs, domain: &NodeIdentity) -> Result<(), SendError> {
		self.inner.send_to(message, domain)
	}

	fn send_to_all(&self, message: OutboundNetMsgs) -> Result<(), SendError> {
		self.inner.send_to_all(message)
	}

	fn send_to_all_except(&self, message: OutboundNetMsgs, exclude: &NodeIdentity) -> Result<(), SendError> {
		self.inner.send_to_all_except(message, exclude)
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
	pub connect_internal: <ConnectInternal as StaticChannelAtom>::Channel,
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
			connect_internal: MpscChannel::new(conf.get_or_default::<ConnectInternal>()),
			peer_connected: BroadcastChannel::new(conf.get_or_default::<ConnectionReady>()),
			key_mismatch_reporter: BroadcastChannel::new(conf.get_or_default::<ProtocolKeyMismatchReporter>()),
			key_mismatch_approver: BroadcastChannel::new(conf.get_or_default::<ProtocolKeyMismatchApprover>()),
		}
	}
}

pub type PacketsForSession = Vec<CiphertextEnvelope>;
static_channel_atom!(SocketToSession, DomainMultiChannel<PacketsForSession, FullSessionName, MpscChannel<PacketsForSession>>, PacketsForSession, FullSessionName, 4096);

static_channel_atom!(KillFromSession, MpscChannel<(FullSessionName, Vec<SessionLayerError>)>, (FullSessionName, Vec<SessionLayerError>), 128);
static_channel_atom!(SystemKillSession, DomainMultiChannel<(), FullSessionName, MpscChannel<()>>, (), FullSessionName, 16);

/// Net-system-sided channels, intended to subset EngineNetChannels. 
#[derive(ChannelSet)]
pub struct NetSystemChannels {
	#[channel(SocketToSession, new_channel)]
	pub raw_to_session: <SocketToSession as StaticChannelAtom>::Channel,
	/// Game-to-network. Outbound i.e. outbound from game
	#[channel(NetMsgOutbound)]
	pub net_msg_outbound: <NetMsgOutbound as StaticChannelAtom>::Channel,
	/// Network-to-game. Inbound i.e. inbound from net.
	#[channel(NetMsgInbound)]
	pub net_msg_inbound: <NetMsgInbound as StaticChannelAtom>::Channel,
	/// Successful connections after protocol negotiation & handshake.
	#[take_receiver(ConnectInternal)]
	pub connect_internal: MpscReceiver<SuccessfulConnect>,
	#[channel(ConnectionReady)]
	pub announce_connection: BroadcastChannel<ConnectAnnounce>,
	/// Net-system-internal, used to push OuterEnvelopes from session to socket.
	#[channel(PacketPush, new_channel)]
	pub session_to_socket: <PacketPush as StaticChannelAtom>::Channel,
	/// Net-system-internal, used by sessions to notify the net system it's good to shut this session down.
	#[channel(KillFromSession, new_channel)]
	pub kill_from_session: MpscChannel<(FullSessionName, Vec<SessionLayerError>)>,
	/// Net-system-internal, used by the network system to notify sessions it's time for them to die.
	#[channel(SystemKillSession, new_channel)]
	pub system_kill_session: <SystemKillSession as StaticChannelAtom>::Channel,
}

impl NetSystemChannels { 
	pub fn init_peer(&self, session: FullSessionName, ident: NodeIdentity) {
		self.net_msg_outbound.init_peer(ident);
		self.raw_to_session.init_domain(session.clone());
		self.system_kill_session.init_domain(session);
	}
	pub fn drop_peer(&self, session: &FullSessionName, ident: &NodeIdentity) {
		self.net_msg_outbound.drop_peer(ident);
		self.raw_to_session.drop_domain(session);
		self.system_kill_session.drop_domain(session);
	}
}

/// Channels used by one session object, intended to subset NetSystemChannels.
#[derive(ChannelSet)]
pub struct SessionChannels {
	/// Sent by game, received by session.
	#[take_receiver(SocketToSession, domain: "session")]
	pub socket_to_session: MpscReceiver<PacketsForSession>,
	/// Network-to-game. Inbound i.e. inbound from net.
	#[channel(NetMsgInbound)]
	pub to_engine: InboundNetChannel,
	#[take_receiver(NetMsgOutbound, domain: "peer_identity")]
	pub from_engine: OutboundNetMsgReceiver,
	/// ConnectionReady is sent as soon as our session object has decided that it's safe
	/// to tell the rest of the engine that this connection has occurred.
	#[sender(ConnectionReady)]
	pub announce_connection: BroadcastSender<ConnectAnnounce>,
	/// Net-system-internal, used by sessions to give ready packets to the packet handler.
	#[sender(PacketPush)]
	pub push_sender: MpscSender<OutboundRawPackets>,
	/// Net-system-internal, used by sessions to notify the net system it's good to shut this session down.
	#[sender(KillFromSession)]
	pub kill_session: MpscSender<(FullSessionName, Vec<SessionLayerError>)>,
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
