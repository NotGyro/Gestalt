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
use log::{error, info, warn, debug, trace};
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::sync::mpsc;

use crate::common::identity::{IdentityKeyPair, DecodeIdentityError};
use crate::common::{identity::NodeIdentity, Version};
use crate::net::handshake::{PROTOCOL_NAME, PROTOCOL_VERSION};

use std::sync::Arc;
use std::time::Duration;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::{TcpStream, TcpListener};

use super::handshake::HandshakeNext;
use super::{SessionId, SuccessfulConnect, handshake::{HandshakeReceiver, load_noise_local_keys, HandshakeError, HandshakeIntitiator}};

use super::{MessageCounter, GESTALT_PORT, NetworkRole, SelfNetworkRole};

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
    pub handshake_step: u8, 
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Introduction { 
    /// Base-64 encoded [`crate::common::identity::NodeIdentity`]
    pub identity_key: String,
    /// What kind of network node are we? 
    pub role: NetworkRole,
    // Unnecessary reduction in anonymity, there should be an AnnounceName netmsg that goes over ciphertext.
    // What should we call you?
    // Note that this is a valid field for both a server and a player, but they will do different things in each context.
    // pub display_name: String,
    /// What version of Gestalt is this?
    #[serde(with = "crate::common::version_string")] 
    pub gestalt_engine_version: Version,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartHandshakeMsg { 
    pub use_protocol: ProtocolDef, 
    /// Contains a Base-64 encoded [`NodeIdentity`], identifying the user who is connecting. 
    pub initiator_identity: Introduction,
    pub handshake: HandshakeStepMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PreProtocolQuery {
    /// Open a PreProtocol session with our Base-64 encoded [`crate::common::identity::NodeIdentity`], telling the server who we are. 
    Introduction(Introduction),
    /// Find out which protocols the server supports. 
    SupportedProtocols,
    /// Get Gestalt identity when you only have an IP.
    RequestIdentity,
    /// Is the server ready to join? 
    RequestServerStatus,
    /// Asks for the name, current playercount, etc of the server. 
    /// Response will be json that is not guaranteed to be in any particular structure 
    RequestServerInfo,
    /// Initiates a handshake, providing the handshake protocol definition of the handshake we will use.
    StartHandshake(StartHandshakeMsg),
    Handshake(HandshakeStepMessage),
    /// Sent by the party who encountered an error when an error is encountered. Initiator will only ever send an error during handshake.
    HandshakeFailed(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PreProtocolReply {
    Status(ServerStatus),
    /// Basic information. Contains a Base-64 encoded [`crate::common::identity::NodeIdentity`]
    Identity(Introduction),
    
    /// Name, current playercount, etc of the server. 
    /// Response will be json that is not guaranteed to be in any particular structure 
    ServerInfo(serde_json::Value),
    Handshake(HandshakeStepMessage),
    /// Find out which protocols the server supports. 
    SupportedProtocols(SupportedProtocols),
    /// Sent by the party who encountered an error when an error is encountered. 
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
    description: serde_json::Value,
    our_identity: IdentityKeyPair,
    our_role: SelfNetworkRole,
    peer_identity: Option<NodeIdentity>,
    peer_role: Option<NetworkRole>,
    peer_engine_version: Option<Version>,
}

impl PreProtocolReceiver { 
    pub fn new(our_identity: IdentityKeyPair, role: SelfNetworkRole) -> Self { 
        PreProtocolReceiver { 
            state: PreProtocolReceiverState::QueryAnswerer, 
            description: serde_json::Value::default(),
            our_identity,
            our_role: role,
            peer_identity: None,
            peer_role: None, 
            peer_engine_version: None,
        }
    }
    pub fn update_description(&mut self, description: serde_json::Value) { 
        self.description = description;
    }
    pub fn is_handshake_done(&self) -> bool { 
        match &self.state {
            PreProtocolReceiverState::QueryAnswerer => false,
            PreProtocolReceiverState::Handshake(receiver) => {
                receiver.is_done()
            },
        }
    }
    pub fn complete_handshake(&mut self) -> Result<(snow::StatelessTransportState, MessageCounter, SessionId), PreProtocolError> { 
        match std::mem::take(&mut self.state) {
            PreProtocolReceiverState::QueryAnswerer => Err(HandshakeError::CompleteBeforeDone.into()),
            PreProtocolReceiverState::Handshake(receiver) => {
                receiver.complete().map_err(|e| e.into())
            },
        }
    }
    pub fn receive_and_reply(&mut self, incoming: PreProtocolQuery) -> Result<PreProtocolOutput, PreProtocolError>{
        let callback_different_key = | node_identity: &NodeIdentity, _old_key: &[u8], _new_key: &[u8]| -> bool {
            warn!("Protocol keys for {} have changed. Accepting new key.", node_identity.to_base64());
            true
        };
        Ok(match incoming {
            PreProtocolQuery::Introduction(identity) => {
                let maybe_ident = NodeIdentity::from_base64(&identity.identity_key); 
                match maybe_ident { 
                    Ok(ident) => { 
                        self.peer_identity = Some(ident);
                        PreProtocolOutput::NoMessage
                    }, 
                    Err(e) => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("{:?}", e))),
                }
            },
            PreProtocolQuery::SupportedProtocols => {
                PreProtocolOutput::Reply(
                    PreProtocolReply::SupportedProtocols( SupportedProtocols {
                        supported_protocols: SUPPORTED_PROTOCOL_SET.clone(),
                    })
                )
            },
            PreProtocolQuery::RequestIdentity => {
                PreProtocolOutput::Reply(
                    PreProtocolReply::Identity(Introduction { 
                        identity_key: self.our_identity.public.to_base64(),
                        role: self.our_role.into(), // TODO when mirror-servers / CDN-type stuff is implemented - make this more flexible. 
                        gestalt_engine_version: crate::ENGINE_VERSION,
                    })
                )
            },
            PreProtocolQuery::RequestServerStatus => { 
                PreProtocolOutput::Reply( 
                    PreProtocolReply::Status(*SERVER_STATUS.clone().lock())
                )
            },
            PreProtocolQuery::RequestServerInfo => { 
                PreProtocolOutput::Reply( 
                    PreProtocolReply::ServerInfo(self.description.clone())
                )
            },
            PreProtocolQuery::StartHandshake(start_handshake) => { 
                self.peer_engine_version = Some(start_handshake.initiator_identity.gestalt_engine_version);
                self.peer_role = Some(start_handshake.initiator_identity.role);

                let maybe_ident = NodeIdentity::from_base64(&start_handshake.initiator_identity.identity_key); 
                match maybe_ident { 
                    Ok(ident) => { 
                        self.peer_identity = Some(ident);
                        if !self.state.is_in_handshake() { 
                            let mut receiver_state = HandshakeReceiver::new(load_noise_local_keys(self.our_identity.public)?, self.our_identity);
                            let out = receiver_state.advance(start_handshake.handshake, &ident, callback_different_key);
                            match out { 
                                Ok(HandshakeNext::SendMessage(message)) => { 
                                    self.state = PreProtocolReceiverState::Handshake(receiver_state);
                                    PreProtocolOutput::Reply(PreProtocolReply::Handshake(message))
                                },
                                Ok(HandshakeNext::Done) => return Err(PreProtocolError::NoReplyToStart),
                                Err(e) => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("{:?}",e))),
                            }
                        }
                        else {
                            PreProtocolOutput::Reply(PreProtocolReply::Err(format!("{:?}", PreProtocolError::HandshakeAlreadyStarted)))
                        }
                    }, 
                    Err(e) => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("{:?}", e))),
                }
            },
            PreProtocolQuery::Handshake(msg) => { 
                trace!("Handshake step message received: {}", msg.handshake_step);
                match &mut self.state { 
                    PreProtocolReceiverState::Handshake(receiver) => { 
                        let out = receiver.advance(msg, &self.peer_identity.unwrap(), callback_different_key);
                        match out { 
                            Ok(HandshakeNext::SendMessage(message)) => {
                                trace!("Sending handshake step: {}", message.handshake_step);
                                PreProtocolOutput::Reply(PreProtocolReply::Handshake(message))
                            },
                            // Receiver doesn't work this way.
                            Ok(HandshakeNext::Done) => unreachable!(),
                            Err(e) => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("{:?}",e))),
                        }
                    },
                    PreProtocolReceiverState::QueryAnswerer => PreProtocolOutput::Reply(PreProtocolReply::Err(format!("{:?}", PreProtocolError::HandshakeMessageWithoutHandshakeStart)) ),
                }
            },
            PreProtocolQuery::HandshakeFailed(err) => {
                self.state = PreProtocolReceiverState::QueryAnswerer;
                match &self.peer_identity {
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

pub async fn preprotocol_receiver_session(our_identity: IdentityKeyPair, our_role: SelfNetworkRole /* In most cases this will be Server for a receiver, but I want to leave it flexible. */, 
        peer_address: SocketAddr, mut stream: TcpStream, completed_channel: mpsc::UnboundedSender<SuccessfulConnect>) {
    let mut receiver = PreProtocolReceiver::new(our_identity, our_role);
    while match read_preprotocol_message(&mut stream).await {
        Ok(msg) => match serde_json::from_str::<PreProtocolQuery>(&msg) { 
                Ok(query) => {
                    match receiver.receive_and_reply(query) {
                        Ok(out) => match out {
                            PreProtocolOutput::Reply(to_send) => {
                                let json_string = serde_json::to_string(&to_send).unwrap();
                                write_preprotocol_message( &json_string, &mut stream).await.unwrap();

                                match receiver.is_handshake_done() {
                                    true => {
                                        let peer_identity =  receiver.peer_identity.unwrap().clone();
                                        let (transport, seq, session_id) = receiver.complete_handshake().unwrap();
                                        info!("Successfully completed handshake with {}!", peer_identity.to_base64());

                                        match (receiver.peer_role, receiver.peer_engine_version) { 
                                            (Some(peer_role), Some(peer_engine_version)) => {
                                            
                                                let completed = SuccessfulConnect {
                                                    session_id,
                                                    peer_identity,
                                                    peer_address,
                                                    peer_role,
                                                    peer_engine_version,
                                                    transport_cryptography: transport,
                                                    transport_counter: seq as u32,
                                                };
                                                
                                                info!("A connection to this server was successfully made by client {}, running Gestalt v{}", completed.peer_identity.to_base64(), &completed.peer_engine_version);
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
                        format!("Parsing error: {:?}", e)
                    );
                    let json_string = serde_json::to_string(&reply).unwrap();
                    write_preprotocol_message( &json_string, &mut stream).await.unwrap();

                    false
                },
            },
        Err(_) => {
            error!("Error getting message length from {}", stream.peer_addr().unwrap());
            let reply = PreProtocolReply::Err(String::from("Error getting message length."));
            let json_string = serde_json::to_string(&reply).unwrap();
            write_preprotocol_message( &json_string, &mut stream).await.unwrap();

            false
        }
    } {}
    stream.shutdown().await.unwrap();
}

/// Spawns a thread which listens for pre-protocol connections on TCP.
pub async fn launch_preprotocol_listener(our_identity: IdentityKeyPair, our_address: Option<SocketAddr>, completed_channel: mpsc::UnboundedSender<SuccessfulConnect>) { 
    let ip = match our_address { 
        Some(value) => value, 
        None => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), GESTALT_PORT),
    };
    let listener = TcpListener::bind(ip).await.unwrap();
    loop {
        match listener.accept().await {
            Ok((stream, peer_address)) => {
                trace!("New PreProtocol connection: {}", peer_address);
                let completed_channel_clone = completed_channel.clone();
                tokio::spawn(
                    // connection succeeded
                    preprotocol_receiver_session(our_identity,  SelfNetworkRole::Server, peer_address, stream, completed_channel_clone)
                );
            },
            Err(e) => {
                error!("An error was encountered in accepting an incoming session: {:?}", e);
            }
        }
    }
}

pub async fn preprotocol_connect_inner(stream: &mut TcpStream, our_identity: IdentityKeyPair, our_role: SelfNetworkRole, server_address: SocketAddr) -> Result<SuccessfulConnect, HandshakeError> {
    let callback_different_key = | node_identity: &NodeIdentity, _old_key: &[u8], _new_key: &[u8]| -> bool {
        warn!("Protocol keys for {} have changed. Accepting new key.", node_identity.to_base64());
        true
    };

    let introduction = Introduction {
        identity_key: our_identity.public.to_base64(),
        role: our_role.into(),
        gestalt_engine_version: crate::ENGINE_VERSION,
    };
    // Exchange identities.
    let query_introduce = PreProtocolQuery::Introduction(introduction.clone());
    let json_query = serde_json::to_string(&query_introduce)?;
    write_preprotocol_message(&json_query, stream).await?;

    let query_request_identity = PreProtocolQuery::RequestIdentity;
    let json_query = serde_json::to_string(&query_request_identity)?;
    write_preprotocol_message(&json_query, stream).await?;
    stream.flush().await?;

    let msg = read_preprotocol_message(stream).await?;
    let reply = serde_json::from_str::<PreProtocolReply>(&msg)?;
    let server_introduction = if let PreProtocolReply::Identity(introduction) = reply { 
        introduction
    } else { 
        return Err(HandshakeError::NoIdentity);
    };
    let server_identity = NodeIdentity::from_base64(&server_introduction.identity_key)?;
    
    
    // Get protocols 
    let query_request_protocols = PreProtocolQuery::SupportedProtocols;
    let json_query = serde_json::to_string(&query_request_protocols)?;
    write_preprotocol_message(&json_query, stream).await?;
    stream.flush().await?;
    
    let msg = read_preprotocol_message(stream).await?;
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

    // Send first handshake message.
    let mut handshake_initiator = HandshakeIntitiator::new(load_noise_local_keys(our_identity.public)?, our_identity);
    let handshake_first = handshake_initiator.send_first()?;
    let query = PreProtocolQuery::StartHandshake(StartHandshakeMsg{
        use_protocol: current_protocol,
        initiator_identity: introduction,
        handshake: handshake_first,
    });
    let json_query = serde_json::to_string(&query)?;
    write_preprotocol_message(&json_query, stream).await?;
    
    // Loop until we're done.
    while !handshake_initiator.is_done() {
        let msg = read_preprotocol_message(stream).await?;
        trace!("Got a pre-protocol reply: {}", &msg);
        let reply = serde_json::from_str::<PreProtocolReply>(&msg)?;
        let handshake_step = if let PreProtocolReply::Handshake(step) = reply { 
            step
        } else { 
            return Err(HandshakeError::WrongOrder);
        };

        match handshake_initiator.advance(handshake_step, &server_identity, callback_different_key)? {
            HandshakeNext::SendMessage(msg) => {
                let query = PreProtocolQuery::Handshake(msg);
                let json_query = serde_json::to_string(&query)?;
                write_preprotocol_message(&json_query, stream).await?;
            },
            HandshakeNext::Done => break,
        }
    }

    // We should be done here! Let's go ahead and connect.

    let (transport, counter, session_id) = handshake_initiator.complete()?;

    Ok(SuccessfulConnect{
        session_id,
        peer_identity: server_identity,
        peer_address: server_address,
        transport_cryptography: transport,
        transport_counter: counter as u32,
        peer_role: server_introduction.role,
        peer_engine_version: server_introduction.gestalt_engine_version,
    })
}

pub async fn preprotocol_connect_to_server(our_identity: IdentityKeyPair, server_address: SocketAddr, connect_timeout: Duration) -> Result<SuccessfulConnect, HandshakeError> {
    match tokio::time::timeout(connect_timeout, TcpStream::connect(&server_address)).await {
        Ok(Ok(mut stream)) => {
            // TODO figure out how connections where the initiator will be a non-client at some point
            match preprotocol_connect_inner(&mut stream, our_identity, SelfNetworkRole::Client, server_address).await {
                Ok(completed_connection) => {
                    info!("Successfully initiated connection to a server with identity {}, running Gestalt v{}", completed_connection.peer_identity.to_base64(), &completed_connection.peer_engine_version);
                    stream.shutdown().await.unwrap();
                    Ok(completed_connection)
                },
                Err(error) => {
                    error!("Handshake error connecting to server: {:?}", error);
                    let error_to_send = PreProtocolQuery::HandshakeFailed(format!("{:?}", error));
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
            Err(e.into())
        },
    }
}

#[cfg(test)]
pub mod test {
    use std::{time::Duration, net::Ipv6Addr};
    use tokio::sync::mpsc;
    use crate::common::identity::IdentityKeyPair;
    use super::*;
 
    #[tokio::test(flavor = "multi_thread")]
    async fn preprotocol_connect_to_localhost() {
        use crate::net::test::NET_TEST_MUTEX;
        let _guard = NET_TEST_MUTEX.lock();
        
        let server_key_pair = IdentityKeyPair::generate_for_tests();
        let client_key_pair = IdentityKeyPair::generate_for_tests();
        let (serv_completed_sender, mut serv_completed_receiver) = mpsc::unbounded_channel();
        let (client_completed_sender, mut client_completed_receiver) = mpsc::unbounded_channel();
        let connect_timeout = Duration::from_secs(2);
    
        let server_addr = IpAddr::V6(Ipv6Addr::LOCALHOST);
        let server_socket_addr = SocketAddr::new(server_addr.clone(), GESTALT_PORT);
        //Launch the server
        tokio::spawn(launch_preprotocol_listener(server_key_pair, Some(server_socket_addr), serv_completed_sender));
        //Give it a moment
        tokio::time::sleep(Duration::from_millis(100)).await;
        //Try to connect
        let client_connection = preprotocol_connect_to_server(client_key_pair, server_socket_addr, connect_timeout).await.unwrap();
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
