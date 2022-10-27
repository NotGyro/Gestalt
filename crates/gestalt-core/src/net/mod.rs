use std::collections::VecDeque;
use std::fs;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use std::collections::HashMap;
use laminar::Connection;
use laminar::ConnectionMessenger;
use laminar::VirtualConnection;
use log::error;
use log::info;
use log::trace;
use log::warn;
use serde::Deserialize;
use serde::Serialize;

use snow::StatelessTransportState;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::time::MissedTickBehavior;

use crate::common::FastHashMap;
use crate::common::FastHashSet;
use crate::common::Version;
use crate::common::identity::IdentityKeyPair;
use crate::common::identity::NodeIdentity;
use crate::common::new_fast_hash_map;
use crate::common::new_fast_hash_set;
use crate::message;
use crate::message::MessageReceiver;
use crate::message::MessageSender;
use crate::message::QuitReceiver;
use crate::message::add_domain;
use crate::message::sender_subscribe_domain;
use crate::net::net_channels::net_send_channel;

pub const PREPROTCOL_PORT: u16 = 54134;
pub const GESTALT_PORT: u16 = 54134;

pub mod handshake;
pub mod net_channels;
#[macro_use]
pub mod netmsg;
pub mod preprotocol;
pub mod generated;

pub use netmsg::NetworkRole as NetworkRole; 
pub use netmsg::SelfNetworkRole as SelfNetworkRole; 
pub use netmsg::OuterEnvelope as OuterEnvelope;
pub use netmsg::PacketIntermediary as PacketIntermediary; 
pub use netmsg::NetMsgId as NetMsgId; 
pub use netmsg::NetMsg as NetMsg; 
pub use netmsg::NetMsgDomain as NetMsgDomain;
pub use netmsg::InboundNetMsg as InboundNetMsg; 
pub use netmsg::DISCONNECT_RESERVED as DISCONNECT_RESERVED;

use self::net_channels::INBOUND_NET_MESSAGES;
use self::netmsg::MessageSidedness;

pub const SESSION_ID_LEN: usize = 4;
pub type SessionId = [u8; SESSION_ID_LEN];

pub type MessageCounter = u32;

/// Represents a client who has completed a handshake in the pre-protocol and will now be moving over to the game protocol proper
#[derive(Debug)]
pub struct SuccessfulConnect {
    pub session_id: SessionId,
    pub peer_identity: NodeIdentity,
    pub peer_address: SocketAddr,
    pub peer_role: NetworkRole, 
    pub peer_engine_version: Version,
    pub transport_cryptography: StatelessTransportState,
    pub transport_counter: u32,
}

impl SuccessfulConnect { 
    pub fn get_full_session_name(&self) -> FullSessionName { 
        FullSessionName {
            peer_address: self.peer_address,
            session_id: self.session_id,
        }
    }
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

pub type LaminarConfig = laminar::Config;

/// Partial reimplementation of a Laminar::ConnectionManager with somewhat different logic since we're using async here, and there will be one of these per peer per node.
pub struct LaminarConnectionManager {
    peer_address: SocketAddr,
    connection_state: VirtualConnection,
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
    pub fn process_inbound<T: IntoIterator< Item: AsRef<[u8]> >>(&mut self, inbound_messages: T, time: Instant) -> Result<(), LaminarWrapperError> {
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
    pub fn process_outbound<T: IntoIterator< Item=laminar::Packet >>(&mut self, outbound_messages: T, time: Instant)  -> Result<(), LaminarWrapperError> { 
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
    #[error("Laminar asked to send a packet to {0:?} but this session is a communicating with {1:?}")]
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
    #[error("Peer {0:?} sent a Laminar \"connect\" message after the session was already started!")]
    ConnectAfterStarted(SocketAddr),
    #[error("Variable-length integer could not be decoded: {0:?}")]
    VarIntError(#[from] vu64::Error),
    #[error("A NetMessage of type {0} has been receved from {1}, but no type has been associated with this ID in the engine. \n It's possible this peer is using a newer version of Gestalt.")]
    UnrecognizedMsg(NetMsgId, String),
    #[error("A NetMessage of type {0} has been receved from {1}, but we are a {2:?} and this message's sidedness is a {3:?}.")]
    WrongSidedness(NetMsgId, String, SelfNetworkRole, MessageSidedness),
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
    pub local_counter: u32,
    pub transport_cryptography: snow::StatelessTransportState, 
    
    /// Channel the Session uses to send packets to the UDP socket
    push_channel: PushSender,

    /// Cached sender handles so we don't have to lock the mutex every time we want to send a message.
    inbound_channels: FastHashMap<NetMsgDomain, MessageSender<InboundNetMsg>>,

    pub disconnect_deliberate: bool,

    /// Valid NetMsg types for our network role.
    valid_incoming_messages: FastHashSet<NetMsgId>,
}

impl Session {
    /// Get a message-passing sender for the given NetMsgDomain, caching so we don't have to lock the mutex constantly. 
    fn get_or_susbscribe_inbound_sender(&mut self, domain: NetMsgDomain) -> &mut MessageSender<InboundNetMsg> {
        self.inbound_channels.entry(domain).or_insert_with(|| {
            //add_domain(&INBOUND_NET_MESSAGES, &domain);
            sender_subscribe_domain(&INBOUND_NET_MESSAGES, &domain).unwrap()
        })
    }

    pub fn new(local_identity: IdentityKeyPair, local_role: SelfNetworkRole, peer_address: SocketAddr, connection: SuccessfulConnect, laminar_config: LaminarConfig, 
                push_channel: PushSender, time: Instant) -> Self {
        let mut laminar_layer = LaminarConnectionManager::new(connection.peer_address, &laminar_config, time);
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
    fn encrypt_packet<T: AsRef<[u8]>>(&mut self, plaintext: T) -> Result<OuterEnvelope, SessionLayerError> {
        self.local_counter += 1;
        let mut buffer = vec![0u8; ( (plaintext.as_ref().len() as usize) * 3) + 64 ];
        let len_written = self.transport_cryptography.write_message(self.local_counter as u64, plaintext.as_ref(), &mut buffer)?;
        buffer.truncate(len_written);
        let full_session_name = self.get_session_name();
        Ok(
            OuterEnvelope {
                session_id: full_session_name,
                counter: self.local_counter,
                ciphertext: buffer,
            }
        )
    }

    /// Called inside process_inbound()
    fn decrypt_outer_envelope(&mut self, envelope: OuterEnvelope) -> Result<Vec<u8>, SessionLayerError> {
        let OuterEnvelope{ session_id: _session_id, counter, ciphertext } = envelope;

        let mut buf = vec![0u8; (ciphertext.len() * 3)/2];
        let len_read = self.transport_cryptography.read_message(counter as u64, &ciphertext, &mut buf)?;
        buf.truncate(len_read);
        Ok(buf)
    }

    /// Ingests a batch of packets coming off the wire.
    pub fn ingest_packets<T: IntoIterator< Item=OuterEnvelope >>(&mut self, inbound_messages: T, time: Instant) -> Vec<SessionLayerError> {
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
                    match vu64::decode_with_length(message_type_len, &pkt.payload()[0..message_type_len as usize]) {
                        Ok(message_type_id) => {
                            let message_type_id = message_type_id as NetMsgId;
                            trace!("Decoding a NetMsg from {} with message_type_id {}", self.peer_identity.to_base64(), message_type_id); 
                            let message = InboundNetMsg {
                                message_type_id,
                                payload: pkt.payload()[message_type_len as usize..].to_vec(),
                                peer_identity: self.peer_identity.clone(),
                            };
                            if finished_packets.get(&message_type_id).is_none() { 
                                finished_packets.insert(message_type_id, Vec::default());
                            }
                            finished_packets.get_mut(&message_type_id).unwrap().push(message);
                        },
                        Err(e) => errors.push(e.into()),
                    }
                },
                laminar::SocketEvent::Timeout(addr) => errors.push(SessionLayerError::LaminarTimeout(addr.clone())),
                laminar::SocketEvent::Disconnect(addr) => errors.push(SessionLayerError::LaminarDisconnect(addr.clone())),
                laminar::SocketEvent::Connect(addr) => {
                    //self.laminar.connection_state.last_heard = time;
                    trace!("Connection marked established with {:?}", addr);
                },
            }
        };
        // Push our messages out to the rest of the application.
        for (message_type, message_buf) in finished_packets { 
            if self.valid_incoming_messages.contains(&message_type) { 
                match message_type {
                    // Handle network-subsystem builtin messages
                    DISCONNECT_RESERVED => { 
                        info!("Peer {} has disconnected (deliberately - this is not an error)", self.peer_identity.to_base64()); 
                        self.disconnect_deliberate = true;
                    }
                    // Handle messages meant to go out into the rest of the engine. 
                    _ => {
                        //Non-reserved, game-defined net msg IDs.  
                        let channel = self.get_or_susbscribe_inbound_sender(message_type);
                        match channel.send(message_buf)
                                .map_err(|e| SessionLayerError::SendBroadcastError(e)) {
                            Ok(_x) => {
                                trace!("Successfully just sent a NetMsg from {} of type {} from the session to the rest of the engine.", self.peer_identity.to_base64(), message_type); 
                            },
                            Err(e) => errors.push(e),
                        }
                    }
                }
            }
            else { 
                errors.push(match generated::get_netmsg_table().get(&message_type) {
                    Some(info) => {
                        SessionLayerError::WrongSidedness(message_type, self.peer_identity.to_base64(), self.local_role, info.sidedness.clone())
                    },
                    None => {
                        SessionLayerError::UnrecognizedMsg(message_type, self.peer_identity.to_base64())
                    },
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
            Ok(()) => {},
            Err(e) => errors.push(e.into()),
        }

        errors
    }

    pub fn process_update(&mut self, time: Instant) -> Result<(), SessionLayerError> {
        let mut errors: Vec<SessionLayerError> = Vec::default();
        match self.laminar.process_update(time) {
            Ok(()) => {},
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
    pub fn process_outbound<T: IntoIterator< Item=laminar::Packet >>(&mut self, outbound_messages: T, time: Instant)  -> Result<(), SessionLayerError> {
        let mut errors: Vec<SessionLayerError> = Vec::default();
        match self.laminar.process_outbound(outbound_messages, time) {
            Ok(()) => {},
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

    /// Network connection CPR.
    pub fn force_heartbeat(&mut self) -> Result<(), laminar::error::ErrorKind> {
        let packets = self.laminar.connection_state.process_outgoing(laminar::packet::PacketInfo::heartbeat_packet(&[]), None, Instant::now())?;
        for packet in packets {
            self.laminar.messenger.send_packet(&self.peer_address, &packet.contents());
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
pub async fn handle_session(mut session_manager: Session,
                        mut incoming_packets: mpsc::UnboundedReceiver<Vec<OuterEnvelope>>,
                        mut from_game: MessageReceiver<PacketIntermediary>,
                        session_tick: Duration,
                        kill_from_inside: mpsc::UnboundedSender<(FullSessionName, Vec<SessionLayerError>)>,
                        mut kill_from_outside: tokio::sync::oneshot::Receiver<()>) { 
    let mut ticker = tokio::time::interval(session_tick);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    info!("Handling session for peer {}...", session_manager.peer_identity.to_base64());

    let peer_address = session_manager.peer_address.clone();
    loop {
        tokio::select!{
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

// Each packet on the wire:
// [- 4 bytes session ID -------------------------------]  
// [- 4 bytes message counter --------------------------]
// [- 1-9 bytes vu64 bytes encoding ciphertext size, n -]
// [- n bytes ciphertext -------------------------------]

const MAX_MESSAGE_SIZE: usize = 8192;

pub fn encode_outer_envelope(message: &OuterEnvelope, send_buf: &mut [u8]) -> usize {
    const SESSION_ID_LEN: usize = std::mem::size_of::<SessionId>();
    const COUNTER_LEN: usize = std::mem::size_of::<MessageCounter>();
    let message_len = message.ciphertext.len();
    let encoded_len = vu64::encode(message_len as u64);

    let len_tag_bytes: &[u8] = encoded_len.as_ref();
    
    //let header_len = SESSION_ID_LEN + COUNTER_LEN + len_tag_bytes.len();

    let mut cursor = 0;
    let session_id = message.session_id.session_id.clone();

    send_buf[cursor..cursor+SESSION_ID_LEN].copy_from_slice(&session_id);
    cursor += SESSION_ID_LEN;
    
    send_buf[cursor..cursor+COUNTER_LEN].copy_from_slice(&message.counter.to_le_bytes());
    cursor += COUNTER_LEN;
    
    send_buf[cursor..cursor+len_tag_bytes.len()].copy_from_slice(len_tag_bytes);
    cursor += len_tag_bytes.len();

    //Header done, now write the data.
    send_buf[cursor..cursor+message_len].copy_from_slice(&message.ciphertext);
    cursor += message_len;
    cursor
}

pub async fn run_network_system(our_role: SelfNetworkRole, address: SocketAddr, 
            mut new_connections: mpsc::UnboundedReceiver<SuccessfulConnect>,
            local_identity: IdentityKeyPair,
            laminar_config: LaminarConfig,
            session_tick_interval: Duration) {
    trace!("Initializing network subsystem for {:?}, which is a {:?}. Attempting to bind to socket on {:?}", local_identity.public.to_base64(), our_role, address);
    let socket = if our_role == SelfNetworkRole::Client {
        let socket = UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await.unwrap();
        socket.connect(address).await.unwrap();
        socket
    }
    else {
        UdpSocket::bind(address).await.unwrap()
    };
    trace!("Bound network subsystem to a socket at: {:?}. We are a {:?}", socket.local_addr().unwrap(), our_role);

    // Register all valid NetMsgs. 
    let netmsg_table = generated::get_netmsg_table(); 
    info!("Registering {} NetMsgIds.", netmsg_table.len());
    for (id, msg_type) in netmsg_table.iter() {
        if our_role.should_we_ingest(&msg_type.sidedness) {
            message::add_domain(&INBOUND_NET_MESSAGES, id);
        }
    }

    const SESSION_ID_LEN: usize = std::mem::size_of::<SessionId>();
    const COUNTER_LEN: usize = std::mem::size_of::<MessageCounter>();

    let mut recv_buf = vec![0u8; MAX_MESSAGE_SIZE];
    let mut send_buf = vec![0u8; MAX_MESSAGE_SIZE];

    // Used by servers to hold on to client info until we can ascertain their new port number (the TCP port number from preprotocol/handshake got dropped) 
    let mut anticipated_clients: HashMap<PartialSessionName, SuccessfulConnect> = HashMap::new();

    // One receiver for each session. Messages come into this UDP handler from sessions, and we have to send them.
    // Remember, "Multiple producer single receiver." This is the single receiver.
    let (push_sender, mut push_receiver): (PushSender, PushReceiver) = mpsc::unbounded_channel(); 
    // Per-session channels for routing incoming UDP packets to sessions.
    let mut inbound_channels: HashMap<FullSessionName, mpsc::UnboundedSender<Vec<OuterEnvelope>> > = HashMap::new();

    // Sessions' way of letting us know its their time to go.
    let (kill_from_inside_sender, mut kill_from_inside_receiver) = mpsc::unbounded_channel::<(FullSessionName, Vec<SessionLayerError>)>();
    // This is how we shoot the other task in the head.
    let mut session_kill_from_outside: HashMap<FullSessionName, tokio::sync::oneshot::Sender<()>> = HashMap::new();

    let mut session_to_identity: HashMap<FullSessionName, NodeIdentity> = HashMap::new();
    
    info!("Network system initialized.");
    trace!("Network system init - our role is {:?}, our address is {:?}, and our identity is {}", &our_role, &socket.local_addr(), local_identity.public.to_base64());
    let mut join_handles = Vec::new();

    let mut quit_reciever = QuitReceiver::new(); 

    loop {
        tokio::select!{
            // A packet has been received. 
            received_maybe = (&socket).recv_from(&mut recv_buf) => {
                // TODO: Better error handling later.
                match received_maybe {
                    Ok((len_read, peer_address)) => {
                        assert!(len_read >= SESSION_ID_LEN + COUNTER_LEN + 1);
                        let mut session_id = [0u8; SESSION_ID_LEN];
                        let mut counter_bytes = [0u8; COUNTER_LEN];
        
                        let mut cursor = 0;
                        session_id.copy_from_slice(&recv_buf[cursor..cursor+SESSION_ID_LEN]);
                        cursor += SESSION_ID_LEN;
        
                        counter_bytes.copy_from_slice(&recv_buf[cursor..cursor+COUNTER_LEN]);
                        cursor += COUNTER_LEN;
                        
                        let counter = MessageCounter::from_le_bytes(counter_bytes);
        
                        let first_length_tag_byte: u8 = recv_buf[cursor];
                        //Get the length of the vu64 length tag from the first byte.
                        let lenlen = vu64::decoded_len(first_length_tag_byte) as usize;
                        let message_length = vu64::decode(&recv_buf[cursor..cursor+lenlen]).unwrap(); //TODO: Error handling. 
                        cursor += lenlen;

                        let session_name = FullSessionName {
                            peer_address,
                            session_id,
                        };
                        match inbound_channels.get(&session_name) { 
                            Some(sender) => {
                                let ciphertext = if message_length > 0 {
                                    (&recv_buf[cursor..cursor+message_length as usize]).to_vec()
                                } else { 
                                    warn!("Zero-length message on session {:?}", &session_name);
                                    Vec::new()
                                };
                                sender.send(vec![OuterEnvelope {
                                    session_id: FullSessionName { 
                                        session_id, 
                                        peer_address,
                                    },
                                    counter,
                                    ciphertext,
                                }]).unwrap()
                            },
                            None => {
                                if our_role == SelfNetworkRole::Server {
                                    let partial_session_name = PartialSessionName {
                                        peer_address: peer_address.ip(),
                                        session_id,
                                    };
                                    match anticipated_clients.remove(&partial_session_name) {
                                        Some(connection) => {
                                            trace!("Popping anticipated client entry for session {:?} and establishing a session.", &base64::encode(connection.session_id));
                                            //Communication with the rest of the engine.
                                            net_channels::register_peer(&connection.peer_identity);
                                            match net_send_channel::subscribe_receiver(&connection.peer_identity) { 
                                                Ok(receiver) => {
                                                    trace!("Sender channel successfully registered for {}", connection.peer_identity.to_base64());
                                                    let peer_identity = connection.peer_identity.clone();
                                                    let mut session = Session::new(local_identity.clone(), our_role, peer_address, connection, laminar_config.clone(), push_sender.clone(), Instant::now());
                                                    session.laminar.connection_state.record_recv();
                                                    //Make a channel 
                                                    let (from_net_sender, from_net_receiver) = mpsc::unbounded_channel();
                                                    inbound_channels.insert(session_name, from_net_sender);

                                                    let (kill_from_outside_sender, kill_from_outside_receiver) = tokio::sync::oneshot::channel::<()>();
                                                    session_kill_from_outside.insert(session.get_session_name(), kill_from_outside_sender);
                                    
                                                    let killer_clone = kill_from_inside_sender.clone();
                                                    session_to_identity.insert(session.get_session_name(), session.peer_identity.clone());
                                                    let jh = tokio::spawn( 
                                                        handle_session(session, from_net_receiver, receiver, session_tick_interval.clone(), killer_clone, kill_from_outside_receiver)
                                                    );

                                                    join_handles.push(jh);

                                                    if let Some(sender) = inbound_channels.get(&session_name) {                                                    
                                                        let ciphertext = if message_length > 0 {
                                                            (&recv_buf[cursor..cursor+message_length as usize]).to_vec()
                                                        } else { 
                                                            warn!("Zero-length message on session {:?}", &session_name);
                                                            Vec::new()
                                                        };
                                                        sender.send(vec![OuterEnvelope {
                                                            session_id: FullSessionName { 
                                                                session_id, 
                                                                peer_address,
                                                            },
                                                            counter,
                                                            ciphertext,
                                                        }]).unwrap()
                                                    }
                                                    else {
                                                        error!("Could not send message to newly-connected peer {}", peer_identity.to_base64());
                                                    }
                                                },
                                                Err(e) => { 
                                                    error!("Error initializing new session: {:?}", e);
                                                    println!("Game-to-session-sender already registered for {}", connection.peer_identity.to_base64());
                                                }
                                            }
                                        },
                                        None => {
                                            error!("No session established yet for {:?}", &session_name);
                                        },
                                    }
                                }
                                else {
                                    // TODO: Retain messages in case we run into problems.
                                    error!("No session established yet for {:?}", &session_name);
                                }
                            }
                        }
                    }
                    Err(e) => { 
                        if e.raw_os_error() == Some(10054) {
                            //An existing connection was forcibly closed by the remote host.
                            //ignore - timeout will catch it.
                            warn!("Bad disconnect, an existing connection was forcibly closed by the remote host.");
                        }
                        else { 
                            // Do this twice to ensure it gets added to logs, as well as showing up in the panic message.
                            error!("Error while polling for UDP packets: {:?}", e); 
                            panic!("Error while polling for UDP packets: {:?}", e); 
                        }
                    }
                }
            }
            send_maybe = (&mut push_receiver).recv() => {
                let to_send = send_maybe.unwrap();
                for message in to_send {

                    let encoded_len = encode_outer_envelope(&message, &mut send_buf);

                    //println!("Buffer is {} bytes long and we got to {}. Sending to {:?}", send_buf.len(), cursor+message_len, &message.session_id.peer_address);
                    //Push
                    match our_role {
                        SelfNetworkRole::Client => socket.send(&send_buf[0..encoded_len]).await.unwrap(),
                        _ => socket.send_to(&send_buf[0..encoded_len], message.session_id.peer_address).await.unwrap()
                    };
                     //TODO: Error handling here.
                }
            }
            new_connection_maybe = (&mut new_connections).recv() => {
                let connection = match new_connection_maybe { 
                    Some(conn) => conn, 
                    None => {
                        error!("Channel for new connections closed.");
                        break; // Return to loop head i.e. try a new tokio::select.
                    }, 
                };
                
                info!("Setting up reliability-over-UDP and cryptographic session for peer {}, connecting from Gestalt engine version v{}", connection.peer_identity.to_base64(), &connection.peer_engine_version);

                let session_name = connection.get_full_session_name();
                
                //local_identity: IdentityKeyPair, connection: SuccessfulConnect, laminar_config: &LaminarConfig, 
                //push_channel: PushSender, received_message_channels: HashMap<NetMsgId, NetMsgSender>, time: Instant
                //Todo: Senders.

                if our_role == SelfNetworkRole::Server {
                    trace!("Adding anticipated client entry for session {:?}", &base64::encode(connection.session_id));
                    net_channels::register_peer(&connection.peer_identity);
                    anticipated_clients.insert( PartialSessionName{
                        session_id: connection.session_id.clone(), 
                        peer_address: connection.peer_address.ip(),
                    }, connection);
                }
                else {
                    //Communication with the rest of the engine.
                    net_channels::register_peer(&connection.peer_identity);
                    match net_send_channel::subscribe_receiver(&connection.peer_identity) { 
                        Ok(receiver) => {
                            trace!("Sender channel successfully registered for {}", connection.peer_identity.to_base64());
                            let mut session = Session::new(local_identity.clone(), our_role, connection.peer_address, connection, laminar_config.clone(), push_sender.clone(), Instant::now());
                            //session.laminar.connection_state.record_recv();
                            //Make a channel 
                            let (from_net_sender, from_net_receiver) = mpsc::unbounded_channel();
                            inbound_channels.insert(session_name, from_net_sender);

                            let (kill_from_outside_sender, kill_from_outside_receiver) = tokio::sync::oneshot::channel::<()>();
                            session_kill_from_outside.insert(session.get_session_name(), kill_from_outside_sender);
            
                            let killer_clone = kill_from_inside_sender.clone();
                            session_to_identity.insert(session.get_session_name(), session.peer_identity.clone());
                            let jh = tokio::spawn( async move {
                                session.force_heartbeat().unwrap();
                                handle_session(session, from_net_receiver, receiver, session_tick_interval.clone(), killer_clone, kill_from_outside_receiver).await
                            });

                            join_handles.push(jh);
                        },
                        Err(e) => { 
                            error!("Error initializing new session: {:?}", e);
                            println!("Game-to-session-sender already registered for {}", connection.peer_identity.to_base64());
                        }
                    }
                }
            }
            // Has one of our sessions failed or disconnected? 
            kill_maybe = (&mut kill_from_inside_receiver).recv() => { 
                if let Some((session_kill, errors)) = kill_maybe { 
                    let ident = session_to_identity.get(&session_kill).unwrap().clone(); 
                    if errors.is_empty() {
                        info!("Closing connection for a session with {:?}.", &ident); 
                    }
                    else {
                        info!("Closing connection for a session with {:?}, due to errors: {:?}", &ident, errors); 
                    }
                    inbound_channels.remove(&session_kill);
                    session_kill_from_outside.remove(&session_kill);
                    let _ = session_to_identity.remove(&session_kill);
                    net_channels::drop_peer(&ident);
                }
            }
            quit_ready_indicator = quit_reciever.wait_for_quit() => {
                info!("Shutting down network system.");
                // Notify sessions we're done.
                for (peer_address, _) in inbound_channels.iter() { 
                    let peer_ident = session_to_identity.get(&peer_address).unwrap();
                    net_send_channel::send_to(DisconnectMsg{}, &peer_ident).unwrap();
                }
                tokio::time::sleep(Duration::from_millis(10)).await; 
                // Clear out remaining messages.
                while let Ok(messages) = (&mut push_receiver).try_recv() {
                    for message in messages {
                        let encoded_len = encode_outer_envelope(&message, &mut send_buf);

                        //Push
                        match our_role {
                            SelfNetworkRole::Client => socket.send(&send_buf[0..encoded_len]).await.unwrap(),
                            _ => socket.send_to(&send_buf[0..encoded_len], message.session_id.peer_address).await.unwrap()
                        };
                    }
                }
                // Notify sessions we're done.
                for (session, channel) in session_kill_from_outside {
                    info!("Terminating session with peer {}", session_to_identity.get(&session).unwrap().to_base64());
                    channel.send(()).unwrap();
                }
                tokio::time::sleep(Duration::from_millis(10)).await; 
                for jh in join_handles { 
                    jh.abort();
                    let _ = jh.await;
                }
                quit_ready_indicator.notify_ready();
                break;
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::message;
    use crate::message::SenderAccepts;
    use crate::message_types::JoinDefaultEntry;
    use crate::net::net_channels::net_recv_channel;
    use crate::net::net_channels::net_recv_channel::NetMsgReceiver;

    use super::*;
    use log::LevelFilter;
    use serde::Serialize;
    use serde::Deserialize;
    use simplelog::TermLogger;
    use super::net_channels::NetSendChannel;
    use super::preprotocol::launch_preprotocol_listener;
    use super::preprotocol::preprotocol_connect_to_server;
    use lazy_static::lazy_static;
 
    #[derive(Clone, Serialize, Deserialize, Debug)]
    #[netmsg(1337, Common, ReliableOrdered)]
    pub(crate) struct TestNetMsg {
        pub message: String, 
    }
    lazy_static! {
        /// Used to keep tests which use real network i/o from clobbering eachother. 
        pub static ref NET_TEST_MUTEX: tokio::sync::Mutex<()> = tokio::sync::Mutex::new(());
    }

    #[tokio::test]
    async fn session_with_localhost() {
        let mutex_guard = NET_TEST_MUTEX.lock().await;
        let _log = TermLogger::init(LevelFilter::Debug, simplelog::Config::default(), simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto );

        let server_key_pair = IdentityKeyPair::generate_for_tests();
        let client_key_pair = IdentityKeyPair::generate_for_tests();
        let (serv_completed_sender, serv_completed_receiver) = mpsc::unbounded_channel();
        let (client_completed_sender, client_completed_receiver) = mpsc::unbounded_channel();

        let server_addr = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let server_socket_addr = SocketAddr::new(server_addr, GESTALT_PORT);

        let test_table = tokio::task::spawn_blocking(|| { 
            generated::get_netmsg_table()
        }).await.unwrap();
        println!("Counted {} registered NetMsg types.", test_table.len());
        
        //Launch server
        let join_handle_s = tokio::spawn(
            run_network_system(SelfNetworkRole::Server,
                server_socket_addr,
                serv_completed_receiver,
                server_key_pair.clone(),
                LaminarConfig::default(),
                Duration::from_millis(50))
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _join_handle_handshake_listener = tokio::spawn(launch_preprotocol_listener(server_key_pair.clone(), Some(server_socket_addr), serv_completed_sender ));
        tokio::time::sleep(Duration::from_millis(10)).await;

        //Launch client
        let join_handle_c = tokio::spawn(
            run_network_system( SelfNetworkRole::Client,  server_socket_addr, 
            client_completed_receiver,
                client_key_pair.clone(),
                LaminarConfig::default(),
                Duration::from_millis(50))
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        let client_completed_connection = preprotocol_connect_to_server(client_key_pair.clone(),
                server_socket_addr,
                Duration::new(5, 0) ).await.unwrap();
        client_completed_sender.send(client_completed_connection).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        net_send_channel::send_to(JoinDefaultEntry{ display_name: "test".to_string()}, &server_key_pair.public ).unwrap();

        let mut test_receiver: NetMsgReceiver<TestNetMsg> = net_recv_channel::subscribe().unwrap();

        let test = TestNetMsg { 
            message: String::from("Boop!"),
        };
        let client_to_server_sender: NetSendChannel<TestNetMsg> = net_send_channel::subscribe_sender(&server_key_pair.public).unwrap();
        client_to_server_sender.send_one(test.clone()).unwrap();
        info!("Attempting to send a message to {}", client_key_pair.public.to_base64());

        {
            let out = tokio::time::timeout(Duration::from_secs(5), test_receiver.recv_wait()).await.unwrap().unwrap();
            let (peer_ident, out) = out.first().unwrap().clone();
            assert_eq!(&peer_ident, &client_key_pair.public);

            info!("Got {:?} from {}", out, peer_ident.to_base64());

            assert_eq!(out.message, test.message);
        }

        let test_reply = TestNetMsg { 
            message: String::from("Beep!"), 
        };
        let message_sender: NetSendChannel<TestNetMsg> = net_send_channel::subscribe_sender(&client_key_pair.public).unwrap();
        info!("Attempting to send a message to {}", server_key_pair.public.to_base64());
        message_sender.send_one(test_reply.clone()).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        {
            let out = tokio::time::timeout(Duration::from_secs(5), test_receiver.recv_wait()).await.unwrap().unwrap();
            let (peer_ident, out) = out.first().unwrap().clone();
            assert_eq!(&peer_ident, &server_key_pair.public);

            info!("Got {:?} from {}", out, peer_ident.to_base64());

            assert_eq!(out.message, test_reply.message);
        }

        message::send_one((), &message::START_QUIT).unwrap(); 
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = join_handle_s.abort();
        let _ = join_handle_c.abort();
        let _ = join_handle_s.await;
        let _ = join_handle_c.await;

        drop(mutex_guard);
    }
}