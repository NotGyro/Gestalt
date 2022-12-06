use std::fs;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::net::Ipv6Addr;
use std::net::SocketAddr;
use std::net::ToSocketAddrs;
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::time::Duration;
use std::time::Instant;

use std::collections::HashMap;
use futures::StreamExt;
use futures::TryFutureExt;
use futures::stream::FuturesUnordered;
use log::error;
use log::info;
use log::trace;
use log::warn;

use snow::StatelessTransportState;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::common::identity::IdentityKeyPair;
use crate::common::identity::NodeIdentity;
use crate::message::QuitReadyNotifier;
use crate::message::QuitReceiver;
use crate::net::net_channels::net_send_channel;

//pub const PREPROTCOL_PORT: u16 = 54134;
//pub const GESTALT_PORT: u16 = 54134;

pub mod handshake;
pub mod net_channels;
#[macro_use]
pub mod netmsg;
pub mod preprotocol;
pub mod generated;
pub mod reliable_udp;
pub mod session;

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
use self::netmsg::CiphertextEnvelope;
use self::netmsg::OuterEnvelopeError;
use self::reliable_udp::*;
use self::session::*;

pub type MessageCounterInit = u32;
pub type MessageCounter = NonZeroU32;

const MAX_MESSAGE_SIZE: usize = 8192;

/// Which directory holds temporary network protocol data? 
/// I.e. Noise protocol keys, cached knowledge of "this identity is at this IP," etc. 
pub fn default_protocol_store_dir() -> PathBuf {
    const PROTOCOL_STORE_DIR: &str = "protocol/";
    let path = PathBuf::from(PROTOCOL_STORE_DIR);
    if !path.exists() {
        fs::create_dir(&path).unwrap();
    }
    path
}

/// Represents a client who has completed a handshake in the pre-protocol and will now be moving over to the game protocol proper
#[derive(Debug)]
pub struct SuccessfulConnect {
    pub session_id: SessionId,
    pub peer_identity: NodeIdentity,
    pub peer_address: SocketAddr,
    pub peer_role: NetworkRole,
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
const SESSION_ID_LEN: usize = std::mem::size_of::<SessionId>();
const COUNTER_LEN: usize = std::mem::size_of::<MessageCounter>();

#[derive(thiserror::Error, Debug)]
pub enum NetworkError {
    #[error("Error encountered encoding or decoding an outer envelope: {0:?}")]
    OuterEnvelope(#[from] OuterEnvelopeError),
    #[error("IO Error: {0}.")]
    IoError(#[from] std::io::Error),
    #[error("Channel for new connections has been closed, cannot receive new connections.")]
    NoNewConnectionsChannel,
}

pub struct NetworkSystem {
    pub our_role: SelfNetworkRole,
    socket: UdpSocket,
    pub new_connections: mpsc::UnboundedReceiver<SuccessfulConnect>,
    pub local_identity: IdentityKeyPair,
    pub laminar_config: LaminarConfig,
    pub session_tick_interval: Duration,
    /// Used by servers to hold on to client info until we can ascertain their new port number (the TCP port number from preprotocol/handshake got dropped) 
    anticipated_clients: HashMap<PartialSessionName, SuccessfulConnect>,
    recv_buf: Vec<u8>,
    send_buf: Vec<u8>,
    push_sender: PushSender,
    push_receiver: PushReceiver,
    /// One receiver for each session. Messages come into this UDP handler from sessions, and we have to send them.
    /// Remember, "Multiple producer single receiver." This is the single receiver.
    /// Per-session channels for routing incoming UDP packets to sessions.
    inbound_channels: HashMap<FullSessionName, mpsc::UnboundedSender<Vec<CiphertextEnvelope>>>,
    /// This is how the session objects let us know it's their time to go. 
    kill_from_inside_session_sender: mpsc::UnboundedSender<(FullSessionName, Vec<SessionLayerError>)>,
    kill_from_inside_session_receiver: mpsc::UnboundedReceiver<(FullSessionName, Vec<SessionLayerError>)>,
    /// This is how we shoot the other task in the head.
    session_kill_from_outside: HashMap<FullSessionName, tokio::sync::oneshot::Sender<()>>,
    session_to_identity: HashMap<FullSessionName, NodeIdentity>,
    join_handles: Vec<JoinHandle<()>>,
}

impl NetworkSystem {
    pub async fn new(our_role: SelfNetworkRole,
            address: SocketAddr,
            new_connections: mpsc::UnboundedReceiver<SuccessfulConnect>,
            local_identity: IdentityKeyPair,
            laminar_config: LaminarConfig,
            session_tick_interval: Duration) -> Result<Self, std::io::Error> { 
        
        let (push_sender, push_receiver): (PushSender, PushReceiver) = mpsc::unbounded_channel(); 
        let (kill_from_inside_session_sender, kill_from_inside_session_receiver) = mpsc::unbounded_channel::<(FullSessionName, Vec<SessionLayerError>)>();

        let socket = match our_role {
            SelfNetworkRole::Server => UdpSocket::bind(address).await?,
            SelfNetworkRole::Client => UdpSocket::bind(SocketAddr::from((Ipv4Addr::UNSPECIFIED, 0))).await?,
        };

        Ok(Self {
            our_role,
            socket,
            new_connections,
            local_identity,
            laminar_config,
            session_tick_interval,
            anticipated_clients: HashMap::default(),
            recv_buf: vec![0u8; MAX_MESSAGE_SIZE],
            send_buf: vec![0u8; MAX_MESSAGE_SIZE],
            push_sender,
            push_receiver,
            inbound_channels: HashMap::default(),
            kill_from_inside_session_sender,
            kill_from_inside_session_receiver,
            session_kill_from_outside: HashMap::default(),
            session_to_identity: HashMap::default(),
            join_handles: Vec::default(),
        })
    }
    pub async fn add_new_session(&mut self, actual_address: FullSessionName, connection: SuccessfulConnect) -> std::io::Result<()> {
        trace!("Attempting to add connection for {:?} with transport counter {}", &actual_address.peer_address, &connection.transport_counter);

        //Communication with the rest of the engine.
        net_channels::register_peer(&connection.peer_identity);
        match net_send_channel::subscribe_receiver(&connection.peer_identity) { 
            Ok(receiver) => {
                trace!("Sender channel successfully registered for {}", connection.peer_identity.to_base64());
                // Construct the session
                let mut session = Session::new(self.local_identity.clone(), 
                    self.our_role,
                    actual_address.peer_address,
                    connection,
                    self.laminar_config.clone(),
                    self.push_sender.clone(),
                    Instant::now());

                // Make a channel 
                let (from_net_sender, from_net_receiver) = mpsc::unbounded_channel();
                self.inbound_channels.insert(actual_address, from_net_sender);

                let (kill_from_outside_sender, kill_from_outside_receiver) = tokio::sync::oneshot::channel::<()>();
                self.session_kill_from_outside.insert(session.get_session_name(), kill_from_outside_sender);

                let killer_clone = self.kill_from_inside_session_sender.clone();
                self.session_to_identity.insert(session.get_session_name(), session.peer_identity.clone());

                let session_tick_interval = self.session_tick_interval.clone();
                let our_role = self.our_role.clone();
                let jh = tokio::spawn( async move {
                    // If this is a server, this may have been in anticipated_clients and so we need to record that we got a packet here, 
                    // because this session is being constructed because we just got a packet from the client. 
                    if our_role == SelfNetworkRole::Server { 
                        session.laminar.connection_state.record_recv();
                    }
                    else if our_role == SelfNetworkRole::Client { 
                        session.force_heartbeat().unwrap();
                    }

                    handle_session(session,
                        from_net_receiver,
                        receiver,
                        session_tick_interval,
                        killer_clone,
                        kill_from_outside_receiver).await
                });

                self.join_handles.push(jh);
            },
            Err(e) => {
                error!("Error initializing new session: {:?}", e);
                println!("Game-to-session-sender already registered for {}", connection.peer_identity.to_base64());
            }
        }
        Ok(())
    }
    pub async fn shutdown(&mut self, quit_ready_indicator: QuitReadyNotifier) { 
        // Notify sessions we're done.
        for (peer_address, _) in self.inbound_channels.iter() { 
            let peer_ident = self.session_to_identity.get(&peer_address).unwrap();
            net_send_channel::send_to(DisconnectMsg{}, &peer_ident).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(10)).await; 
        // Clear out remaining messages.
        while let Ok(messages) = (&mut self.push_receiver).try_recv() {
            for message in messages {
                match message.encode(&mut self.send_buf) {
                    Ok(len_written) => {
                        //Push
                        match self.our_role {
                            SelfNetworkRole::Client => self.socket.send_to(&self.send_buf[0..len_written], message.session.peer_address).await.unwrap(),
                            _ => self.socket.send_to(&self.send_buf[0..len_written], message.session.peer_address).await.unwrap()
                        };
                    },
                    Err(e) => error!("Encountered an encoding error while trying to shut shut down the network system: {:?} \n\
                                                        Since we are shutting down anyway, continuing to flush other remaining messages.", e),
                }
            }
        }
        let nuke: Vec<_> = self.session_kill_from_outside.drain().collect();
        // Notify sessions we're done.
        for (session, channel) in nuke {
            info!("Terminating session with peer {}", self.session_to_identity.get(&session).unwrap().to_base64());
            channel.send(()).unwrap();
        }
        tokio::time::sleep(Duration::from_millis(10)).await; 
        for jh in &self.join_handles { 
            jh.abort();
            let _ = jh;
        }
        quit_ready_indicator.notify_ready();
    }
    pub async fn wait_for_ready(&mut self) -> Result<(), NetworkError> {
        match (self.our_role, self.inbound_channels.len()) { 
            // We're a client (i.e. not listening) and have no connections yet, 
            // make sure we are connected so we don't try to receive from nobody. 
            (SelfNetworkRole::Client, 0) => {
                let connection = match self.new_connections.recv().await { 
                    Some(conn) => conn, 
                    None => {
                        error!("Channel for new connections closed.");
                        return Err(NetworkError::NoNewConnectionsChannel);
                    }, 
                };
                
                info!("Setting up reliability-over-UDP and cryptographic session for peer {}", connection.peer_identity.to_base64());

                let session_name = connection.get_full_session_name();
                
                self.add_new_session(session_name, connection).await.unwrap();

                Ok(())
            }
            // Every other case leads to a normal receive. 
            _ => { 
                Ok(())
            }
        }
    }
    pub async fn run(&mut self) {
        trace!("Initializing network subsystem for {:?}, which is a {:?}.", self.local_identity.public.to_base64(), self.our_role);

        // Register all valid NetMsgs. 
        let netmsg_table = generated::get_netmsg_table(); 
        info!("Registering {} NetMsgIds.", netmsg_table.len());
        for (id, msg_type) in netmsg_table.iter() {
            if self.our_role.should_we_ingest(&msg_type.sidedness) {
                INBOUND_NET_MESSAGES.add_domain(id);
            }
        }

        info!("Network system initialized.");
        trace!("Network system init - our role is {:?}, and our identity is {}", &self.our_role, self.local_identity.public.to_base64());

        let mut quit_reciever = QuitReceiver::new();

        //If we are a client, make sure there's at least one session going before polling for anything. 
        //Otherwise silly things will happen, like attempting to receive on a channel that doesn't exist. 
        self.wait_for_ready().await.unwrap();

        loop {
            tokio::select!{
                new_connection_maybe = (&mut self.new_connections).recv() => {
                    let connection = match new_connection_maybe { 
                        Some(conn) => conn, 
                        None => {
                            error!("Channel for new connections closed.");
                            break; // Return to loop head i.e. try a new tokio::select.
                        }, 
                    };
                    
                    info!("Setting up reliability-over-UDP and cryptographic session for peer {}", connection.peer_identity.to_base64());

                    let session_name = connection.get_full_session_name();
                    
                    if self.our_role == SelfNetworkRole::Server {
                        trace!("Adding anticipated client entry for session {:?}", &base64::encode(connection.session_id));
                        net_channels::register_peer(&connection.peer_identity);
                        self.anticipated_clients.insert( PartialSessionName{
                            session_id: connection.session_id.clone(), 
                            peer_address: connection.peer_address.ip(),
                        }, connection);
                    }
                    else {
                        self.add_new_session(session_name, connection).await.unwrap();
                    }
                }
                // A packet has been received.
                received_maybe = (&mut self.socket).recv_from(&mut self.recv_buf) => {
                    match received_maybe {
                        Ok((len_read, peer_address)) => {
                            match OuterEnvelope::decode_packet(&self.recv_buf[..len_read], peer_address.clone()) {
                                Err(OuterEnvelopeError::ZeroLengthCiphertext(addr)) => { 
                                    warn!("Zero-length ciphertext received on a ciphertext message from {:?}. Possible bug.", peer_address);
                                },
                                Err(e) => { 
                                    error!("Error attempting to decode an OuterEnvelope that just came in off the UDP socket from {:?}: {:?}", peer_address, e);
                                }
                                Ok((message, len_message)) => { 
                                    assert_eq!(len_read, len_message); //TODO: Figure out if the socket will ever act in a way which breaks this assumption. 
                                    let OuterEnvelope{session: session_name, body: message_body} = message; 
                                    match self.inbound_channels.get(&message.session) {
                                        Some(sender) => {
                                            match message_body {
                                                netmsg::EnvelopeBody::Ciphertext(ciphertext_message) => {
                                                    sender.send(vec!(CiphertextEnvelope{ 
                                                        session: session_name, 
                                                        body: ciphertext_message
                                                    })).unwrap()
                                                },
                                                netmsg::EnvelopeBody::Protocol(protocol_message) => {
                                                    //TODO - the switch to having protocol messages at all is pretty new and I'm working on it. 
                                                },
                                            }
                                        },
                                        None => {
                                            if self.our_role == SelfNetworkRole::Server {
                                                // Reconstruct the partial session name so we can do a lookup with it. 
                                                let partial_session_name = PartialSessionName {
                                                    peer_address: peer_address.ip(),
                                                    session_id: session_name.session_id,
                                                };
                                                //Did we have an anticipated client with this partial session name?
                                                match self.anticipated_clients.remove(&partial_session_name) {
                                                    Some(connection) => {
                                                        trace!("Popping anticipated client entry for session {:?} and establishing a session.", &base64::encode(connection.session_id));
                                                        trace!("Addr is {:?}", &session_name.peer_address);
                    
                                                        let peer_identity = connection.peer_identity.clone();
                                                        match self.add_new_session(session_name, connection).await { 
                                                            Ok(()) => {
                                                                // Push the message we just got from the rest of the engine out to the network. 
                                                                if let Some(sender) = self.inbound_channels.get(&session_name) {         
                                                                    match message_body {
                                                                        netmsg::EnvelopeBody::Ciphertext(ciphertext_message) => {
                                                                            sender.send(vec!(CiphertextEnvelope{ 
                                                                                session: session_name, 
                                                                                body: ciphertext_message
                                                                            })).unwrap()
                                                                        },
                                                                        netmsg::EnvelopeBody::Protocol(protocol_message) => {
                                                                            //TODO - the switch to having protocol messages at all is pretty new and I'm working on it. 
                                                                        },
                                                                    }
                                                                }
                                                                else {
                                                                    error!("Could not send message to newly-connected peer {}", peer_identity.to_base64());
                                                                }
                                                            },
                                                            Err(e) => { 
                                                                error!("Error adding a new session incoming from {:?}: {:?}", peer_address, e);
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
                            }
                        }
                        Err(e) => { 
                            if e.raw_os_error() == Some(10054) {
                                //An existing connection was forcibly closed by the remote host.
                                //ignore - timeout will catch it.
                                warn!("Bad disconnect, an existing connection was forcibly closed by the remote host. Our role is: {:?}", &self.our_role);
                            }
                            else {
                                error!("Error attempting to read from UDP socket: {:?}", e);
                            }
                        }
                    }
                }
                send_maybe = (&mut self.push_receiver).recv() => {
                    let to_send = send_maybe.unwrap();
                    for message in to_send {
                        match message.encode(&mut self.send_buf) {
                            Ok(encoded_len) => {
                                trace!("Sending {}-byte packet to {:?}", encoded_len, &message.session);
                                //Push
                                let resl: Result<usize, std::io::Error> = match self.our_role {
                                    SelfNetworkRole::Client => self.socket.send_to(&self.send_buf[0..encoded_len], message.session.peer_address).await,
                                    _ => self.socket.send_to(&self.send_buf[0..encoded_len], message.session.peer_address).await
                                };
                                trace!("{:?}", resl);
                            },
                            Err(e) => error!("Error encountered encoding an outer envelope: {:?}", e),
                        }
                    }
                }
                // Has one of our sessions failed or disconnected? 
                kill_maybe = (&mut self.kill_from_inside_session_receiver).recv() => { 
                    if let Some((session_kill, errors)) = kill_maybe { 
                        let ident = self.session_to_identity.get(&session_kill).unwrap().clone(); 
                        if errors.is_empty() {
                            info!("Closing connection for a session with {:?}.", &ident); 
                        }
                        else {
                            info!("Closing connection for a session with {:?}, due to errors: {:?}", &ident, errors); 
                        }
                        self.inbound_channels.remove(&session_kill);
                        self.session_kill_from_outside.remove(&session_kill);
                        let _ = self.session_to_identity.remove(&session_kill);
                        net_channels::drop_peer(&ident);
                    }
                }
                quit_ready_indicator = quit_reciever.wait_for_quit() => {
                    info!("Shutting down network system.");
                    self.shutdown(quit_ready_indicator.clone()).await;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use std::net::IpAddr;
    use std::net::Ipv6Addr;

    use crate::message;
    use crate::message::BroadcastChannel;
    use crate::message::MessageSender;
    use crate::message::ReceiverChannel;
    use crate::message::SenderChannel;
    use crate::message_types::JoinDefaultEntry;
    use crate::net::handshake::approver_no_mismatch;
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
 
    async fn find_available_udp_port(range: std::ops::Range<u16>) -> Option<u16> { 
        for i in range { 
            match UdpSocket::bind((Ipv6Addr::LOCALHOST, i)).await { 
                Ok(_) => return Some(i),
                Err(_) => {},
            }
        }
        None
    }

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
        // Init stuff
        let mutex_guard = NET_TEST_MUTEX.lock().await;
        let _log = TermLogger::init(LevelFilter::Trace, simplelog::Config::default(), simplelog::TerminalMode::Mixed, simplelog::ColorChoice::Auto );
        
        let protocol_dir = tempfile::tempdir().unwrap();

        let server_key_pair = IdentityKeyPair::generate_for_tests();
        let client_key_pair = IdentityKeyPair::generate_for_tests();
        let (serv_completed_sender, serv_completed_receiver) = mpsc::unbounded_channel();
        let (client_completed_sender, client_completed_receiver) = mpsc::unbounded_channel();
        
        // Mismatch approver stuff.
        let mismatch_report_channel: BroadcastChannel<NodeIdentity> = BroadcastChannel::new(1024); 
        let mismatch_approve_channel: BroadcastChannel<(NodeIdentity, bool)> = BroadcastChannel::new(1024);
        let mismatch_report_receiver = mismatch_report_channel.receiver_subscribe();
        let mismatch_approve_sender = mismatch_approve_channel.sender_subscribe();
        // Spawn our little "explode if the key isn't new" system. 
        tokio::spawn( approver_no_mismatch(mismatch_report_receiver, mismatch_approve_sender) );

        // Port/binding stuff. 
        let port = find_available_udp_port(54134..54534).await.unwrap();
        info!("Binding on port {}", port); 
        
        let server_addr = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let server_socket_addr = SocketAddr::new(server_addr, port);

        let test_table = tokio::task::spawn_blocking(|| { 
            generated::get_netmsg_table()
        }).await.unwrap();
        println!("Counted {} registered NetMsg types.", test_table.len());
        
        //Actually start doing the test here: 
        //Launch server
        let join_handle_s = tokio::spawn(
            async move {
                let mut sys = NetworkSystem::new(SelfNetworkRole::Server,
                    server_socket_addr,
                    serv_completed_receiver,
                    server_key_pair.clone(),
                    LaminarConfig::default(),
                    Duration::from_millis(50)).await.unwrap();
                sys.run().await
            }
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        //Server's preprotocol listener
        let _join_handle_handshake_listener = tokio::spawn(
            launch_preprotocol_listener(server_key_pair.clone(), 
                Some(server_socket_addr), 
                serv_completed_sender, 
                port,
                PathBuf::from(protocol_dir.path()),
                mismatch_report_channel.clone(), 
                mismatch_approve_channel.clone()));
        tokio::time::sleep(Duration::from_millis(10)).await;

        //Launch client
        let join_handle_c = tokio::spawn(
            async move {
                let mut sys = NetworkSystem::new(
                    SelfNetworkRole::Client,  
                    server_socket_addr, 
                    client_completed_receiver,
                    client_key_pair.clone(),
                    LaminarConfig::default(),
                    Duration::from_millis(50)).await.unwrap();
                sys.run().await
            }
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
        let client_completed_connection = preprotocol_connect_to_server(client_key_pair.clone(),
                server_socket_addr,
                Duration::new(5, 0),
                PathBuf::from(protocol_dir.path()),
                mismatch_report_channel.sender_subscribe(), 
                mismatch_approve_channel.receiver_subscribe()).await.unwrap();
        client_completed_sender.send(client_completed_connection).unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;

        net_send_channel::send_to(JoinDefaultEntry{ display_name: "test".to_string()}, &server_key_pair.public ).unwrap();

        let mut test_receiver: NetMsgReceiver<TestNetMsg> = net_recv_channel::subscribe().unwrap();

        let test = TestNetMsg { 
            message: String::from("Boop!"),
        };
        let client_to_server_sender: NetSendChannel<TestNetMsg> = net_send_channel::subscribe_sender(&server_key_pair.public).unwrap();
        client_to_server_sender.send_one(test.clone()).unwrap();
        info!("Attempting to send a message to server {}", server_key_pair.public.to_base64());

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
        let server_to_client_sender: NetSendChannel<TestNetMsg> = net_send_channel::subscribe_sender(&client_key_pair.public).unwrap();
        info!("Attempting to send a message to client {}", client_key_pair.public.to_base64());
        server_to_client_sender.send_one(test_reply.clone()).unwrap();

        tokio::time::sleep(Duration::from_millis(10)).await;

        {
            let out = tokio::time::timeout(Duration::from_secs(5), test_receiver.recv_wait()).await.unwrap().unwrap();
            let (peer_ident, out) = out.first().unwrap().clone();
            assert_eq!(&peer_ident, &server_key_pair.public);

            info!("Got {:?} from {}", out, peer_ident.to_base64());

            assert_eq!(out.message, test_reply.message);
        }

        message::START_QUIT.send_one(()).unwrap(); 
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = join_handle_s.abort();
        let _ = join_handle_c.abort();
        let _ = join_handle_s.await;
        let _ = join_handle_c.await;

        drop(mutex_guard);
    }
}