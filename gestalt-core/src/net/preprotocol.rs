//! The "pre-protocol" is a TCP connection established between two Gestalt nodes to exchange information about 
//! which protocols they supub(crate) pub(crate) pport, which versions of these protocols they support, and other metadata 
//! such as "server name", estimating RTT, cryptographic primitives supported, etc. All of this happens *before*, 
//! not simultaneously with, any gameplay or exchange of content-addressed Gestalt resources.
//! 
//! Every pre-protocol message is 32 bytes, little-endian, providing the length of the string that will follow, and then a json string.
//! 
//! The motivation here is that I plan to use a reliability layer over UDP for the actual Gestalt protocol,
//! but it's possible that the fundamental structure of that reliability layer's packets on the wire could
//! change. TCP is not going to change, and neither is json - nor are any massive new vulnerabilities likely
//! to crop up in the fundamental design of TCP or json (implementation specifics notwithsdanding).
//! So, this is an attempt at some basic future-proofing.
//! The actual codebase is not "already future-proof", but the intent is to communicate a set of supported 
//! protocols on a channel that is very unlikely to break backwards-compatibility. 
//! 
//! tl;dr please do not make breaking changes in this file, thanx

use lazy_static::lazy_static;

use std::collections::HashSet;
use std::path::PathBuf;
use log::{error, info, trace};
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::sync::mpsc;

use crate::common::identity::{IdentityKeyPair, DecodeIdentityError};
use crate::common::{identity::NodeIdentity, Version};
use crate::message::{SenderChannel, ReceiverChannel};
use crate::net::handshake::{PROTOCOL_NAME, PROTOCOL_VERSION};

use std::sync::Arc;
use std::time::Duration;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::{TcpStream, TcpListener};

use super::handshake::{HandshakeNext, load_noise_local_keys, NewProtocolKeyReporter, NewProtocolKeyApprover, noise_protocol_dir};
use super::{SessionId, SuccessfulConnect, handshake::{HandshakeReceiver, HandshakeError, HandshakeInitiator}};

use super::{NetworkRole, SelfNetworkRole, MessageCounterInit};

// TODO/NOTE - Cryptography should behave differently on known long-term static public key and unknown long-term static public key. 

// Types of pre-protocol request / response pairs:
// * What's your name/alias?
// * Is the server currently full? Is it unavailable to join for some other reasons? 
// * I want to connect, what handshake protocols & game protocols do you support? 
// That third one starts a state machine for connection.

/// Represents a supported version of a supported protocol. 
#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Ord, Clone)]
pub struct ProtocolDef {
    /// Name of the protocol, such as "gestalt-laminar"
    pub protocol: String,
    #[serde(with = "crate::common::version_string")] 
    pub version: Version,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportedProtocols {
    pub supported_protocols: HashSet<ProtocolDef>, 
}

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
pub enum ServerStatus { 
    NoResponse,
    Unavailable,
    Starting,
    Ready,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeStepMessage {
    pub data: String,
    pub handshake_step: u8, 
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartHandshakeMsg { 
    pub handshake: HandshakeStepMessage,
    pub initiator_role: NetworkRole, //"I am connecting as an initiator_role in relation to you"
    pub use_protocol: ProtocolDef,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PreProtocolQuery {
    /// Find out which protocols the server supports. 
    SupportedProtocols,
    /// Is the server ready to join? 
    RequestServerStatus,
    /// Initiates a handshake, providing the handshake protocol definition of the handshake we will use.
    StartHandshake(StartHandshakeMsg),
    Handshake(HandshakeStepMessage),
    /// Sent by the party who encountered an error when an error is encountered.
    Err(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PreProtocolReply {
    Status(ServerStatus),
    Handshake(HandshakeStepMessage),
    /// Find out which protocols the server supports. 
    SupportedProtocols(SupportedProtocols),
    /// General or handshake-specific error.
    Err(String),
}

lazy_static! {
    pub static ref SUPPORTED_PROTOCOL_SET: HashSet<ProtocolDef> = { 
        let mut set = HashSet::new(); 
        set.insert(ProtocolDef{ 
            protocol: PROTOCOL_NAME.to_string(), 
            version: PROTOCOL_VERSION,
        });
        set
    };
}

lazy_static! {
    pub static ref SERVER_STATUS: Arc<Mutex<ServerStatus>> = Arc::new(Mutex::new(ServerStatus::Starting));
}

#[derive(thiserror::Error, Debug)]
pub enum PreProtocolError {
    #[error("Bad handshake: {0:?}")]
    HandshakeError(#[from] HandshakeError),
    #[error("Identity parsing error: {0:?}")]
    IdentityError(#[from] DecodeIdentityError),
    #[error("Attempted to start a handshake, but the Initiator has not provided a node identity.")]
    HandshakeNoIdentity,
    #[error("Peer did not provide engine version or expected network role!")]
    NoIntroduction,
    #[error("Received a Handshake message but a handshake was never started.")]
    HandshakeMessageWithoutHandshakeStart,
    #[error("Received a Handshake start message but a handshake was already started.")]
    HandshakeAlreadyStarted,
    #[error("An attempt to start a handshake was made with unsupported protocol: {0:?}")]
    UnsupportedProtocol(ProtocolDef),
    #[error("The Handshake Receiver did not produce a reply message to the start handshake message.")]
    NoReplyToStart,
}

/// Pre-protocol receiver capable of answering questions from one peer.
pub enum PreProtocolReceiverState{
    QueryAnswerer,
    Handshake(HandshakeReceiver),
}

impl Default for PreProtocolReceiverState {
    fn default() -> Self {
        PreProtocolReceiverState::QueryAnswerer
    }
}

impl PreProtocolReceiverState { 
    pub fn is_in_handshake(&self) -> bool { 
        match self {
            PreProtocolReceiverState::QueryAnswerer => false,
            PreProtocolReceiverState::Handshake(_) => true,
        }
    }
    pub fn get_peer_identity(&self) -> Option<&NodeIdentity> { 
        match self { 
            PreProtocolReceiverState::QueryAnswerer => None,
            PreProtocolReceiverState::Handshake(receiver) => receiver.get_peer_identity(), 
        }
    }
}

pub enum PreProtocolOutput { 
    Reply(PreProtocolReply), 
    /// Send none, but keep receiving. 
    NoMessage,
    /// Stop receiving PreProtocol messages. 
    Done,
}

/// Pre-protocol receiver capable of answering questions from one peer.
pub struct PreProtocolReceiver { 
    state: PreProtocolReceiverState,
    protocol_dir: PathBuf,
    our_identity: IdentityKeyPair,
    peer_role: Option<NetworkRole>,
    mismatch_reporter: Option<NewProtocolKeyReporter>,
    mismatch_approver: Option<NewProtocolKeyApprover>,
}

impl PreProtocolReceiver { 
    pub fn new(our_identity: IdentityKeyPair, 
            _role: SelfNetworkRole,
            protocol_dir: PathBuf,
            mismatch_reporter: NewProtocolKeyReporter,
            mismatch_approver: NewProtocolKeyApprover,
        ) -> Self {
        PreProtocolReceiver { 
            state: PreProtocolReceiverState::QueryAnswerer,
            protocol_dir,
            our_identity,
            peer_role: None,
            mismatch_reporter: Some(mismatch_reporter),
            mismatch_approver: Some(mismatch_approver),
        }
    }
    pub fn is_handshake_done(&self) -> bool { 
        match &self.state {
            PreProtocolReceiverState::QueryAnswerer => false,
            PreProtocolReceiverState::Handshake(receiver) => {
                receiver.is_done()
            },
        }
    }
    pub fn complete_handshake(&mut self) -> Result<(snow::StatelessTransportState, MessageCounterInit, NodeIdentity, SessionId), PreProtocolError> { 
        match std::mem::take(&mut self.state) {
            PreProtocolReceiverState::QueryAnswerer => Err(HandshakeError::CompleteBeforeDone.into()),
            PreProtocolReceiverState::Handshake(receiver) => {
                receiver.complete().map_err(|e| e.into())
            },
        }
    }
    pub async fn receive_and_reply(&mut self, incoming: PreProtocolQuery) -> Result<PreProtocolOutput, PreProtocolError>{
        Ok(match incoming {
            PreProtocolQuery::SupportedProtocols => {
                PreProtocolOutput::Reply(
                    PreProtocolReply::SupportedProtocols( SupportedProtocols {
                        supported_protocols: SUPPORTED_PROTOCOL_SET.clone(),
                    })
                )
            },
            PreProtocolQuery::RequestServerStatus => { 
                PreProtocolOutput::Reply( 
                    PreProtocolReply::Status(*SERVER_STATUS.clone().lock())
                )
            },
            PreProtocolQuery::StartHandshake(start_handshake) => {
                self.peer_role = Some(start_handshake.initiator_role);
                if !self.state.is_in_handshake() {
                    // For when noise keys changed. 
                    let mismatch_reporter = self.mismatch_reporter.take().ok_or(HandshakeError::NoMismatchChannels)?; 
                    let mismatch_approver = self.mismatch_approver.take().ok_or(HandshakeError::NoMismatchChannels)?; 
                    let noise_dir = noise_protocol_dir(&self.protocol_dir);
                    // Init the receiver state machine
                    let mut receiver_state = HandshakeReceiver::new(
                        noise_dir,
                        load_noise_local_keys(self.protocol_dir.clone(), self.our_identity.public.clone()).await?, 
                        self.our_identity.clone(), 
                        mismatch_reporter, 
                        mismatch_approver,
                        );
                    match receiver_state.advance(start_handshake.handshake).await { 
                        Ok(HandshakeNext::SendMessage(message)) => { 
                            self.state = PreProtocolReceiverState::Handshake(receiver_state);
                            PreProtocolOutput::Reply(PreProtocolReply::Handshake(message))
                        },
                        Ok(HandshakeNext::Done) => return Err(PreProtocolError::NoReplyToStart),
                        Err(e) => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("Handshake error: {:?}",e))),
                    }
                    //Mutex guard should drop here.
                }
                else {
                    PreProtocolOutput::Reply(PreProtocolReply::Err(format!("Handshake error: {:?}", PreProtocolError::HandshakeAlreadyStarted)))
                }
            },
            PreProtocolQuery::Handshake(msg) => { 
                trace!("Handshake step message received: {}", msg.handshake_step);
                match &mut self.state {
                    PreProtocolReceiverState::Handshake(receiver) => {
                        match receiver.advance(msg).await {
                            Ok(HandshakeNext::SendMessage(message)) => {
                                trace!("Sending handshake step: {}", message.handshake_step);
                                PreProtocolOutput::Reply(PreProtocolReply::Handshake(message))
                            },
                            // Receiver doesn't work this way.
                            Ok(HandshakeNext::Done) => unreachable!(),
                            Err(e) => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("Handshake error: {:?}",e))),
                        }
                    },
                    PreProtocolReceiverState::QueryAnswerer => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("Handshake error: {:?}", PreProtocolError::HandshakeMessageWithoutHandshakeStart)) ),
                }
            },
            PreProtocolQuery::Err(err) => {
                self.state = PreProtocolReceiverState::QueryAnswerer;
                match self.state.get_peer_identity() {
                    Some(ident) => error!("Remote party {:?} reported an error in the handshake process: {}", ident, err),
                    None => error!("Unidentified remote party reported an error in the handshake process: {}", err),
                }
                PreProtocolOutput::NoMessage
            },
        })
    }
}
pub async fn write_preprotocol_message(json: &str, stream: &mut TcpStream) -> Result<(), std::io::Error> { 
    let bytes = json.as_bytes();
    let message_len_bytes = (bytes.len() as u32).to_le_bytes();
    assert_eq!(message_len_bytes.len(), 4);
    stream.write_all(&message_len_bytes).await?;
    stream.write_all(bytes).await?;
    stream.flush().await?;
    Ok(())
}

pub async fn read_preprotocol_message(stream: &mut TcpStream) -> Result<String, std::io::Error> { 
    let mut next_message_size_buf = [0_u8; 4];
    stream.read_exact(&mut next_message_size_buf).await?;
    stream.flush().await?;
    let next_message_size = u32::from_le_bytes(next_message_size_buf);
    let mut message_buf: Vec<u8> = vec![0u8; next_message_size as usize];
    stream.read_exact(&mut message_buf).await?;

    Ok(String::from_utf8_lossy(&message_buf).to_string())
}

// To be clear- this is still handling ONE session with one peer, this is not "the server".
pub async fn preprotocol_receiver_session(our_identity: IdentityKeyPair, 
        our_role: SelfNetworkRole, /* In most cases this will be Server for a receiver, 
            but I want to leave it flexible for possible future NAT hole-punching shenanigans.*/
        peer_address: SocketAddr, 
        mut stream: TcpStream, 
        completed_channel: mpsc::UnboundedSender<SuccessfulConnect>,
        protocol_dir: PathBuf,
        mismatch_reporter: NewProtocolKeyReporter,
        mismatch_approver: NewProtocolKeyApprover,
        ) {
    let mut receiver = PreProtocolReceiver::new(our_identity, our_role, protocol_dir, mismatch_reporter, mismatch_approver);
    while match read_preprotocol_message(&mut stream).await {
        Ok(msg) => match serde_json::from_str::<PreProtocolQuery>(&msg) { 
                Ok(query) => {
                    match receiver.receive_and_reply(query).await {
                        Ok(out) => match out {
                            PreProtocolOutput::Reply(to_send) => {
                                let json_string = serde_json::to_string(&to_send).unwrap();
                                write_preprotocol_message( &json_string, &mut stream).await.unwrap();

                                match receiver.is_handshake_done() {
                                    true => {
                                        let (transport, seq, peer_identity, session_id) = receiver.complete_handshake().unwrap();
                                        info!("Successfully completed handshake with {}!", peer_identity.to_base64());

                                        match receiver.peer_role { 
                                            Some(peer_role) => {
                                                
                                                trace!("Connection to {:?} successful and our cryptographic counter is {}", peer_identity.to_base64(), seq); 
                                            
                                                let completed = SuccessfulConnect {
                                                    session_id,
                                                    peer_identity,
                                                    peer_address,
                                                    peer_role,
                                                    transport_cryptography: transport,
                                                    transport_counter: seq as u32,
                                                };
                                                
                                                info!("A connection to this server was successfully made by client {}", completed.peer_identity.to_base64());
                                                completed_channel.send(completed).unwrap();
                                                // Done with this part, stop sending.
                                                false
                                            },
                                            _ => {
                                                error!("Missed introduction from {} - either no role or no engine version", stream.peer_addr().unwrap());
                                                let reply = PreProtocolReply::Err(
                                                    format!("{:?}", PreProtocolError::NoIntroduction)
                                                );
                                                let json_string = serde_json::to_string(&reply).unwrap();
                                                write_preprotocol_message( &json_string, &mut stream).await.unwrap();

                                                false
                                            }
                                        }
                                    },
                                    false => {
                                        // Keep looping.
                                        true
                                    },
                                }
                            },
                            PreProtocolOutput::NoMessage => { true },
                            PreProtocolOutput::Done => { false },
                        },
                        Err(e) => {
                            error!("Preprotocol loop error communicating with {}: {:?}", stream.peer_addr().unwrap(), e);
                            let reply = PreProtocolReply::Err(
                                format!("Preprotocol loop error: {:?}", e)
                            );
                            let json_string = serde_json::to_string(&reply).unwrap();
                            write_preprotocol_message( &json_string, &mut stream).await.unwrap();

                            false
                        },
                    }
                },
                Err(e) => { 
                    error!("Error parsing PreProtocolQuery from json received from {}: {:?}", stream.peer_addr().unwrap(), e);
                    let reply = PreProtocolReply::Err(
                        format!("Preprotocol parsing error: {:?}", e)
                    );
                    let json_string = serde_json::to_string(&reply).unwrap();
                    write_preprotocol_message( &json_string, &mut stream).await.unwrap();

                    false
                },
            },
        Err(_) => {
            error!("Error getting message length from {}", stream.peer_addr().unwrap());
            let reply = PreProtocolReply::Err(String::from("Handshake error: Error getting message length."));
            let json_string = serde_json::to_string(&reply).unwrap();
            write_preprotocol_message( &json_string, &mut stream).await.unwrap();

            false
        }
    } {}
    stream.shutdown().await.unwrap();
}

/// Spawns a thread which listens for pre-protocol connections on TCP.
pub async fn launch_preprotocol_listener<R, A>(our_identity: IdentityKeyPair, 
        our_address: Option<SocketAddr>, 
        completed_channel: mpsc::UnboundedSender<SuccessfulConnect>, 
        port: u16,
        protocol_dir: PathBuf, 
        mismatch_report_channel: R,
        mismatch_approver_channel: A)
        where R: SenderChannel<NodeIdentity, Sender=NewProtocolKeyReporter>, 
            A: ReceiverChannel<(NodeIdentity, bool), Receiver=NewProtocolKeyApprover>{ 

    let ip = match our_address { 
        Some(value) => value, 
        None => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
    };
    let listener = TcpListener::bind(ip).await.unwrap();

    loop {
        match listener.accept().await {
            Ok((stream, peer_address)) => {
                trace!("New PreProtocol connection: {}", peer_address);
                let completed_channel_clone = completed_channel.clone();
                tokio::spawn(
                    // connection succeeded
                    preprotocol_receiver_session(
                            our_identity,
                            SelfNetworkRole::Server,
                            peer_address,
                            stream, 
                            completed_channel_clone, 
                            protocol_dir.clone(),
                            mismatch_report_channel.sender_subscribe(), 
                            mismatch_approver_channel.receiver_subscribe(),
                    )
                );
            },
            Err(e) => {
                error!("An error was encountered in accepting an incoming session: {:?}", e);
            }
        }
    }
}

pub async fn preprotocol_connect_inner(stream: &mut TcpStream, 
        our_identity: IdentityKeyPair, 
        _our_role: SelfNetworkRole, 
        protocol_dir: PathBuf,
        server_address: SocketAddr,
        report_mismatch: NewProtocolKeyReporter,
        mismatch_approver: NewProtocolKeyApprover) -> Result<SuccessfulConnect, HandshakeError> {
    // Get protocols 
    let query_request_protocols = PreProtocolQuery::SupportedProtocols;
    let json_query = serde_json::to_string(&query_request_protocols)?;
    write_preprotocol_message(&json_query, stream).await
        .map_err(HandshakeError::NetIoError)?;
    stream.flush().await.map_err(HandshakeError::NetIoError)?;
    
    let msg = read_preprotocol_message(stream).await
        .map_err(HandshakeError::NetIoError)?;
    let reply = serde_json::from_str::<PreProtocolReply>(&msg)?;
    let server_protocols = if let PreProtocolReply::SupportedProtocols(protocols) = reply { 
        protocols.supported_protocols
    } else { 
        return Err(HandshakeError::NoProtocolsInCommon);
    };

    // Figure out which protocol to use. Right now, it's either "the current protocol" or "nothing"
    let current_protocol = ProtocolDef{ 
        protocol: PROTOCOL_NAME.to_string(), 
        version: PROTOCOL_VERSION,
    };
    if !(server_protocols.contains(&current_protocol)) { 
        return Err(HandshakeError::NoProtocolsInCommon);
    }

    let noise_dir = noise_protocol_dir(&protocol_dir);

    // Send first handshake message.
    let mut handshake_initiator = HandshakeInitiator::new(
        noise_dir, 
        load_noise_local_keys(protocol_dir.clone(), 
        our_identity.public).await?, 
        our_identity, 
        report_mismatch,
        mismatch_approver);
    let handshake_first = handshake_initiator.send_first()?;
    let query = PreProtocolQuery::StartHandshake(StartHandshakeMsg{
        use_protocol: current_protocol,
        handshake: handshake_first,
        initiator_role: NetworkRole::Client,
    });
    let json_query = serde_json::to_string(&query)?;
    write_preprotocol_message(&json_query, stream).await
        .map_err(HandshakeError::NetIoError)?;
    
    // Loop until we're done.
    while !handshake_initiator.is_done() {
        let msg = read_preprotocol_message(stream).await
            .map_err(HandshakeError::NetIoError)?;
        trace!("Got a pre-protocol reply: {}", &msg);
        let reply = serde_json::from_str::<PreProtocolReply>(&msg)?;
        let handshake_step = if let PreProtocolReply::Handshake(step) = reply { 
            step
        } else { 
            return Err(HandshakeError::WrongOrder);
        };

        match handshake_initiator.advance(handshake_step).await? {
            HandshakeNext::SendMessage(msg) => {
                let query = PreProtocolQuery::Handshake(msg);
                let json_query = serde_json::to_string(&query)?;
                write_preprotocol_message(&json_query, stream).await
                    .map_err(HandshakeError::NetIoError)?;
            },
            HandshakeNext::Done => break,
        }
    }

    // We should be done here! Let's go ahead and connect.

    let (transport, counter, server_identity, session_id) = handshake_initiator.complete()?;

    trace!("Connection to {:?} successful and our cryptographic counter is {}", server_identity.to_base64(), counter); 

    Ok(SuccessfulConnect{
        session_id,
        peer_identity: server_identity,
        peer_address: server_address,
        transport_cryptography: transport,
        transport_counter: counter as u32,
        peer_role: NetworkRole::Server,
    })
}

pub async fn preprotocol_connect_to_server(our_identity: IdentityKeyPair, 
        server_address: SocketAddr, 
        connect_timeout: Duration,
        protocol_dir: PathBuf,
        report_mismatch: NewProtocolKeyReporter,
        mismatch_approver: NewProtocolKeyApprover) -> Result<SuccessfulConnect, HandshakeError> {
    match tokio::time::timeout(connect_timeout, TcpStream::connect(&server_address)).await {
        Ok(Ok(mut stream)) => {
            // TODO figure out how connections where the initiator will be a non-client at some point
            match preprotocol_connect_inner(&mut stream, our_identity, SelfNetworkRole::Client, protocol_dir, server_address, report_mismatch, mismatch_approver).await {
                Ok(completed_connection) => {
                    info!("Successfully initiated connection to a server with identity {}", completed_connection.peer_identity.to_base64());
                    stream.shutdown().await.unwrap();
                    Ok(completed_connection)
                },
                Err(error) => {
                    error!("Handshake error connecting to server: {:?}", error);
                    let error_to_send = PreProtocolQuery::Err(format!("Handshake error: {:?}", error));
                    let json_error = serde_json::to_string(&error_to_send).unwrap();
                    write_preprotocol_message(&json_error, &mut stream).await.unwrap();
                    stream.shutdown().await.unwrap();
                    Err(error)
                },
            }
        },
        Err(e) => { 
            error!("Timed out attempting to connect to server: {:?}", e);
            Err(e.into())
        },
        Ok(Err(e)) => { 
            error!("Could not initiate connection to server: {:?}", e);
            Err(HandshakeError::NetIoError(e))
        },
    }
}

#[cfg(test)]
pub mod test {
    use std::{time::Duration, net::Ipv6Addr};
    use tokio::sync::mpsc;
    use crate::{common::identity::IdentityKeyPair, message::{BroadcastChannel, ReceiverChannel, SenderChannel}, net::handshake::approver_no_mismatch};
    use super::*;

    async fn find_available_port(range: std::ops::Range<u16>) -> Option<u16> { 
        for i in range { 
            match TcpListener::bind((Ipv6Addr::LOCALHOST, i)).await { 
                Ok(_) => return Some(i),
                Err(_) => {},
            }
        }
        None
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn preprotocol_connect_to_localhost() {
        use crate::net::test::NET_TEST_MUTEX;
        let _guard = NET_TEST_MUTEX.lock();
        
        let protocol_dir = tempfile::tempdir().unwrap();

        //Mismatch approver stuff. 
        let mismatch_report_channel = BroadcastChannel::new(1024); 
        let mismatch_approve_channel = BroadcastChannel::new(1024);
        let mismatch_report_receiver = mismatch_report_channel.receiver_subscribe();
        let mismatch_approve_sender = mismatch_approve_channel.sender_subscribe();
        // Spawn our little "explode if the key isn't new" system. 
        tokio::spawn( approver_no_mismatch(mismatch_report_receiver, mismatch_approve_sender) );

        // Find an available port
        let port = find_available_port(3223..4223).await.unwrap();
        
        let server_key_pair = IdentityKeyPair::generate_for_tests();
        let client_key_pair = IdentityKeyPair::generate_for_tests();
        let (serv_completed_sender, mut serv_completed_receiver) = mpsc::unbounded_channel();
        let (client_completed_sender, mut client_completed_receiver) = mpsc::unbounded_channel();
        let connect_timeout = Duration::from_secs(2);
    
        let server_addr = IpAddr::V6(Ipv6Addr::LOCALHOST);
        let server_socket_addr = SocketAddr::new(server_addr.clone(), port);
        //Launch the server
        tokio::spawn(launch_preprotocol_listener(server_key_pair, 
            Some(server_socket_addr), 
            serv_completed_sender, 
            port,
            PathBuf::from(protocol_dir.path()),
            mismatch_report_channel.clone(),
            mismatch_approve_channel.clone()));
        //Give it a moment
        tokio::time::sleep(Duration::from_millis(100)).await;
        //Try to connect
        let client_connection = preprotocol_connect_to_server(
            client_key_pair, 
            server_socket_addr, 
            connect_timeout,
            PathBuf::from(protocol_dir.path()),
            mismatch_report_channel.sender_subscribe(), 
            mismatch_approve_channel.receiver_subscribe()).await.unwrap();
        client_completed_sender.send(client_connection).unwrap();
    
        let success_timeout = Duration::from_secs(2);
        //Make sure it has a little time to complete this.
        let successful_server_end = tokio::time::timeout(success_timeout, serv_completed_receiver.recv()).await.unwrap().unwrap();
        let successful_client_end = tokio::time::timeout(success_timeout, client_completed_receiver.recv()).await.unwrap().unwrap();
        // Check if all is valid
        assert_eq!(successful_server_end.peer_identity, client_key_pair.public);
        assert_eq!(successful_client_end.peer_identity, server_key_pair.public);
    }
}
