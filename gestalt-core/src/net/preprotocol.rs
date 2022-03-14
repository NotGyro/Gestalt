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

use hashbrown::HashSet;
use log::{error, info, warn, debug};
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};
use snow::StatelessTransportState;
use snow::params::NoiseParams;

use crate::common::identity::{IdentityKeyPair, DecodeIdentityError};
use crate::common::{identity::NodeIdentity, Version};

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::{thread, fs};
use std::net::{TcpListener, TcpStream, Shutdown, IpAddr, Ipv4Addr, SocketAddr};
use std::io::{Read, Write};

use self::current_protocol::{HandshakeReceiver, load_noise_local_keys, HandshakeError, HandshakeIntitiator};

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

#[derive(Debug, Serialize, Deserialize)]
pub struct StartHandshakeMsg { 
    pub use_protocol: ProtocolDef, 
    /// Base-64 encoded [`NodeIdentity`], identifying the user who is connecting. 
    pub initiator_identity: String,
    pub handshake: HandshakeStepMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PreProtocolQuery {
    /// Open a PreProtocol session with our Base-64 encoded [`crate::common::identity::NodeIdentity`], telling the server who we are. 
    Introduction(String),
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
    /// Base-64 encoded [`crate::common::identity::NodeIdentity`]
    Identity(String),
    /// Name, current playercount, etc of the server. 
    /// Response will be json that is not guaranteed to be in any particular structure 
    ServerInfo(serde_json::Value),
    Handshake(HandshakeStepMessage),
    /// Find out which protocols the server supports. 
    SupportedProtocols(SupportedProtocols),
    /// Sent by the party who encountered an error when an error is encountered. 
    Err(String),
}

pub fn protocol_store_dir() -> PathBuf { 
    const PROTOCOL_STORE_DIR: &'static str = "protocol/"; 
    let path = PathBuf::from(PROTOCOL_STORE_DIR);
    if !path.exists() { 
        fs::create_dir(&path).unwrap(); 
    }
    path
}

pub mod current_protocol {

    use std::fs::OpenOptions;

    use log::{debug, warn};
    use rand::Rng;
    use signature::Signature;

    use super::*;
    use crate::common::identity::IdentityKeyPair;
    use crate::common::{identity::NodeIdentity, Version};

    pub const PROTOCOL_VERSION: Version = version!(1,0,0);
    pub const PROTOCOL_NAME: &'static str = "Gestalt_Noise_XX";

    pub const NOISE_PARAM_STR: &'static str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

    lazy_static! {
        pub(crate) static ref NOISE_PARAMS: NoiseParams = NOISE_PARAM_STR.parse().unwrap();
    }

    #[derive(thiserror::Error, Debug)]
    pub enum HandshakeError {
        #[error("cryptographic error in noise protocol implementation (snow): {0:?}")]
        SnowError(#[from] snow::Error),
        #[error("error decoding or encoding Json message for handshake: {0:?}")]
        JsonError(#[from] serde_json::Error),
        #[error("error decoding Base-64 bytes for handshake: {0:?}")]
        Base64Error(#[from] base64::DecodeError),
        #[error("error decoding identity: {0:?}")]
        IdentityDecodeError(#[from] DecodeIdentityError),
        #[error("i/o error interacting with Noise keystore: {0:?}")]
        IoError(#[from] std::io::Error),
        /// Expected, received.
        #[error("Unexpected step in handshake process - expected {0}, got a handshake step message at {1}")]
        UnexpectedStep(u8, u8),
        #[error("Attempted to send a Gestalt handshake message on the Handshake channel before the Noise handshake was done")]
        SendBeforeNoiseDone,
        #[error("Protocol key for node {0} changed, and the callback for this situation denied accepting the new key.")]
        IdentityChanged(String),
        #[error("Remote static noise key was expected at handshake step {0}, but it was not present!")]
        MissingRemoteStatic(u8),
        #[error("Unable to sign a buffer so that we can confirm our identity to a peer in a handshake: {0:?}")]
        CannotSign(ed25519_dalek::SignatureError),
        #[error("Gestalt signature sent to us by a peer did not pass validation. The other side's attempt to sign the nonce we gave it resulted in a signature which seems invalid.")]
        BadSignature(ed25519_dalek::SignatureError),
        #[error("Handshake messages were sent in the wrong order")]
        WrongOrder,
        #[error("Called send_first() on a handshake initiator more than once, or after calling advance()")]
        FirstAfterInit,
        #[error("Attempted to advance a handshake after it was already done.")]
        AdvanceAfterDone,
        #[error("Attempted to close a handshake before it was done.")]
        CompleteBeforeDone,
        #[error("No identity when we expected an identity!")]
        NoIdentity,
        #[error("Client and server do not have any protocols in common.")]
        NoProtocolsInCommon,
    }

    pub fn noise_protocol_dir() -> PathBuf {
        const SUB_DIR: &'static str = "noise/";
        let path = protocol_store_dir().join(PathBuf::from(SUB_DIR));
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        path
    }

    pub fn noise_peer_dir() -> PathBuf {
        const SUB_DIR: &'static str = "peers/";
        let path = noise_protocol_dir().join(PathBuf::from(SUB_DIR));
        if !path.exists() { 
            fs::create_dir_all(&path).unwrap(); 
        }
        path
    }

    pub fn load_noise_local_keys() -> Result<snow::Keypair, HandshakeError> { 
        const FILENAME: &'static str = "local_keys"; 
        let path = noise_protocol_dir().join(PathBuf::from(FILENAME));
        let keypair = if path.exists() {
            let mut private = [0u8;32];
            let mut public = [0u8;32];

            let mut file = OpenOptions::new()
                .create(false)
                .read(true)
                .open(&path)?;

            
            file.read_exact(&mut private)?;
            file.read_exact(&mut public)?;

            snow::Keypair{
                private: private.to_vec(),
                public: public.to_vec(),
            }
        }
        else {
            let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
            let keypair = builder.generate_keypair().unwrap();
            
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&path)?;

            file.write_all(&keypair.private)?;
            file.write_all(&keypair.public)?;
            file.flush()?;
            drop(file);
            keypair
        };
        Ok(keypair)
    }
    
    pub fn load_validate_noise_peer_key<Callback>(peer_identity: &NodeIdentity, peer_noise_key: &[u8], callback_nonmatching: Callback) -> Result<(), HandshakeError>
            where Callback: FnOnce(&NodeIdentity, &[u8], &[u8]) -> bool {
        let filename = peer_identity.to_base64();
        let path = noise_peer_dir().join(PathBuf::from(filename));
        if path.exists() {
            let mut public = [0u8;32];
            let mut file = OpenOptions::new()
                .create(false)
                .read(true)
                .open(&path)?;
            file.read_exact(&mut public)?;
            file.flush()?;
            drop(file);
            
            if &public == peer_noise_key { 
                // Valid identity, this is what we were expecting.
                Ok(())
            }
            else {
                if callback_nonmatching(peer_identity, &public, peer_noise_key) { 
                    // Our request to go forward with this is approved. 
                    Ok(())
                }
                else { 
                    Err(HandshakeError::IdentityChanged(peer_identity.to_base64()))
                }
            }
        }
        else {
            warn!("Storing new noise keys for unfamiliar peer {:?}", peer_identity);
            
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&path)?;

            file.write_all(&peer_noise_key)?;
            file.flush()?;
            Ok(())
        }
    }

    // Handshake steps:
    // * Step 0 is reserved.
    // * Step 1: Initiator starts handshake. "e ->" in Noise protocol terms  
    // * Step 2: Responder sends response. Noise step: "<- e, ee, s, es". This means the initiator has the responder's Noise public key.
    // * Step 3: Initiator sends closing Noise protocol response. Noise step: "s, se ->" This means we can call handshake_state.into_transport_mode(self), giving us a secure channel.
    // * Both sides have transformed this into a snow::StatelessTransportState.
    // * Step 4: Responder sends a "nonce" buffer for the initiator to sign with its Gestalt identity key.
    // * Step 5: Initiator sends its signature, and also a nonce buffer for the responder to sign.
    // * Step 6: Responder replies with signature on the step 5 buffer. 
    // * Step 7: The initiator receives the signature, verifies, completing the handshake (if both Gestalt identities verified)

    /// Initiator-sided, step 1. Noise protocol  "-> e"  message.
    pub fn initiate_handshake(local_noise_keys: &snow::Keypair) -> Result<(snow::HandshakeState, HandshakeStepMessage), HandshakeError> { 
        let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
        let mut noise = builder.local_private_key(&local_noise_keys.private).build_initiator()?;

        // Generate "-> e" message
        let mut first_message_buf = [0u8; 1024];
        let wrote_len = noise.write_message(&[], &mut first_message_buf)?;
        debug!("Wrote a handshake initiator message which is {} bytes long", wrote_len);
        // Encode
        let msg = HandshakeStepMessage {
            handshake_step: 1,
            data: base64::encode(&first_message_buf[0..wrote_len]),
        };

        Ok(
            (noise, msg)
        )
    }

    /// Receiver-sided, step 2. Noise protocol "<- e, ee, s, es" message.
    pub fn receive_initial(local_noise_keys: &snow::Keypair, input: HandshakeStepMessage) -> Result<(snow::HandshakeState, HandshakeStepMessage), HandshakeError> { 
        if input.handshake_step == 1 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 1024];

            let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
            let mut state = builder.local_private_key(&local_noise_keys.private).build_responder()?;

            // Read their message.
            let read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);
    
            // Generate "e, ee, s, es" message
            let mut write_buf = [0u8; 1024];
            let wrote_len = state.write_message(&[], &mut write_buf)?;
            debug!("Wrote a handshake responder message which is {} bytes long", wrote_len);
            // Encode
            let msg = HandshakeStepMessage {
                handshake_step: 2,
                data: base64::encode(&write_buf[0..wrote_len]),
            };
    
            Ok(
                (state, msg)
            )
        } else {
            Err(HandshakeError::UnexpectedStep(1, input.handshake_step))
        }
    }

    /// Initiator-sided, step 3. Noise protocol "s, se ->" message.
    pub fn initiator_reply<Callback>(mut state: snow::HandshakeState, input: HandshakeStepMessage, peer_gestalt_identity: &NodeIdentity, callback_different_key: Callback) 
            -> Result<(snow::StatelessTransportState, HandshakeStepMessage), HandshakeError> 
            where Callback: FnOnce(&NodeIdentity, &[u8], &[u8]) -> bool { 
        if input.handshake_step == 2 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 1024];

            // Read their message.
            let read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);

            // Send our "s, se ->"
            let mut send_buf = [0u8; 1024];
            let wrote_len = state.write_message(&[], &mut send_buf)?;
            debug!("Wrote a response which is {} bytes long", wrote_len);

            // Format it
            let output = HandshakeStepMessage { 
                handshake_step: 3, 
                data: base64::encode(&send_buf[0..wrote_len]),
            };

            // Get Noise key.
            let remote_static = state.get_remote_static().ok_or(HandshakeError::MissingRemoteStatic(2))?;
            // Make sure we notice if the key changed.
            load_validate_noise_peer_key(peer_gestalt_identity, remote_static, callback_different_key)?;

            // Turn handshake state into a transport
            let transport = state.into_stateless_transport_mode()?;

            Ok((transport, output))
        } else { 
            Err(HandshakeError::UnexpectedStep(2, input.handshake_step))
        }
    }

    fn make_signing_nonce() -> [u8; 32] {
        let mut rng = rand_core::OsRng::default();
        rng.gen()
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct HandshakeMessage4 {
        /// Base-64 encoded buf to sign here.
        pub please_sign: String,
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct HandshakeMessage5 {
        /// Base-64 encoded initiator signature on responder's HandshakeMessage4 "please_sign"
        pub initiator_signature: String,
        /// Base-64 encoded buf to sign here.
        pub please_sign: String,
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct HandshakeMessage6 {
        /// Base-64 encoded responder signature on initiator's HandshakeMessage5 "please_sign"
        pub responder_signature: String,
    }

    pub type MessageCounter = u64;

    /// Receiver-sided, receive step 3 message from initiator and finish Noise handshake. Now we do funky identity stuff, sending a buffer and asking the other side to sign it. 
    pub fn receive_last_noise<Callback>(mut state: snow::HandshakeState, input: HandshakeStepMessage, peer_gestalt_identity: &NodeIdentity, callback_different_key: Callback) -> Result<(snow::StatelessTransportState, HandshakeStepMessage, [u8;32], MessageCounter), HandshakeError>
            where Callback: FnOnce(&NodeIdentity, &[u8], &[u8]) -> bool { 
        if input.handshake_step == 3 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 1024];

            // Read their message.
            let read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);

            // Get Noise key.
            let remote_static = state.get_remote_static().ok_or(HandshakeError::MissingRemoteStatic(3))?;
            // Make sure we notice if the key changed.
            load_validate_noise_peer_key(peer_gestalt_identity, remote_static, callback_different_key)?;

            // Turn handshake state into a transport
            let transport = state.into_stateless_transport_mode()?;

            // Build a HandshakeMessage asking for a signature. 
            let nonce = make_signing_nonce();
            let base64_nonce = base64::encode(&nonce); 
            let message = HandshakeMessage4 { 
                please_sign: base64_nonce,
            };
            let json_message = serde_json::to_string(&message)?;
            
            let mut buf = vec![0u8; 65535];
            let encoded_length = transport.write_message(0, json_message.as_bytes(), &mut buf)?;
            let buf_bytes = &buf[0..encoded_length];
            let encoded_message = base64::encode(&buf_bytes);

            let step = HandshakeStepMessage {
                handshake_step: 4,
                data: encoded_message,
            };
            
            Ok((transport, step, nonce, 0))
        } else { 
            Err(HandshakeError::UnexpectedStep(3, input.handshake_step))
        }
    }
    /// Receive step 4 message, produce step 5 message.
    pub fn initiator_sign_buf(state: snow::StatelessTransportState, input: HandshakeStepMessage, our_keys: &IdentityKeyPair) -> Result<(snow::StatelessTransportState, HandshakeStepMessage, [u8;32], MessageCounter), HandshakeError> { 
        if input.handshake_step == 4 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 65535];

            // Read their message.
            let read_buf_len = state.read_message(0, &bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);

            // Get the inner message.
            let msg: HandshakeMessage4 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
            let to_sign = base64::decode(msg.please_sign)?;
            let our_signature = our_keys.sign(&to_sign).map_err(|e| HandshakeError::CannotSign(e))?; 
            let our_signature_bytes = our_signature.as_bytes();
            let our_signature_b64 = base64::encode(our_signature_bytes);
            
            // Build a HandshakeMessage asking for a signature. 
            let nonce = make_signing_nonce();
            let base64_nonce = base64::encode(&nonce); 
            let message = HandshakeMessage5 { 
                please_sign: base64_nonce,
                initiator_signature: our_signature_b64,
            };
            let json_message = serde_json::to_string(&message)?;
            
            let mut buf = vec![0u8; 65535];
            let encoded_length = state.write_message(0, json_message.as_bytes(), &mut buf)?;
            let buf_bytes = &buf[0..encoded_length];
            let encoded_message = base64::encode(&buf_bytes);

            let step = HandshakeStepMessage {
                handshake_step: 5,
                data: encoded_message,
            };
            
            Ok((state, step, nonce, 0))
        } else { 
            Err(HandshakeError::UnexpectedStep(4, input.handshake_step))
        }
    } 
    /// Receive step 5 message, produce step 6 message.
    pub fn responder_sign(state: snow::StatelessTransportState, input: HandshakeStepMessage, our_keys: &IdentityKeyPair, peer_identity: &NodeIdentity, our_nonce: [u8;32]) -> Result<(snow::StatelessTransportState, HandshakeStepMessage, MessageCounter), HandshakeError> { 
        if input.handshake_step == 5 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 65535];

            // Read their message.
            let read_buf_len = state.read_message(0, &bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);
            // Get the inner message.
            let msg: HandshakeMessage5 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
            // Validate their signature
            let their_sig = base64::decode(msg.initiator_signature)?;
            peer_identity.verify_signature(&our_nonce, &their_sig).map_err(|e| HandshakeError::BadSignature(e))?;

            // Make our signature.
            let to_sign = base64::decode(msg.please_sign)?;
            let our_signature = our_keys.sign(&to_sign).map_err(|e| HandshakeError::CannotSign(e))?; 
            let our_signature_bytes = our_signature.as_bytes();
            let our_signature_b64 = base64::encode(our_signature_bytes);
            
            // Build a HandshakeMessage sending our signature.
            let message = HandshakeMessage6 {
                responder_signature: our_signature_b64,
            };
            let json_message = serde_json::to_string(&message)?;
            
            let mut buf = vec![0u8; 65535];
            let encoded_length = state.write_message(1, json_message.as_bytes(), &mut buf)?;
            let buf_bytes = &buf[0..encoded_length];
            let encoded_message = base64::encode(&buf_bytes);

            let step = HandshakeStepMessage {
                handshake_step: 6,
                data: encoded_message,
            };
            
            Ok((state, step, 1))
        } else { 
            Err(HandshakeError::UnexpectedStep(4, input.handshake_step))
        }
    }
    /// Receive step 6 message, ending handshake.
    pub fn initiator_final(state: snow::StatelessTransportState, input: HandshakeStepMessage, peer_identity: &NodeIdentity, our_nonce: [u8;32]) -> Result<(snow::StatelessTransportState, MessageCounter), HandshakeError> { 
        if input.handshake_step == 6 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 65535];

            // Read their message.
            let read_buf_len = state.read_message(1, &bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);
            // Get the inner message.
            let msg: HandshakeMessage6 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
            // Validate their signature
            let their_sig = base64::decode(msg.responder_signature)?;
            peer_identity.verify_signature(&our_nonce, &their_sig).map_err(|e| HandshakeError::BadSignature(e))?;

            Ok((state, 1))
        } else { 
            Err(HandshakeError::UnexpectedStep(4, input.handshake_step))
        }
    }

    /// Cleaner state machine wrapper for otherwise-imperative handshake process.
    pub enum HandshakeIntitiatorState { 
        Init, 
        SentFirstAwaitSecond(snow::HandshakeState),
        SentThirdAwaitFourth(snow::StatelessTransportState),
        SentFifthAwaitSixth(snow::StatelessTransportState, [u8;32], MessageCounter),
        Done(snow::StatelessTransportState, MessageCounter),
    }
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum HandshakeIntitiatorStep { 
        Init, 
        SentFirstAwaitSecond,
        SentThirdAwaitFourth,
        SentFifthAwaitSixth,
        Done,
    }

    impl HandshakeIntitiatorState {
        pub fn new() -> Self { 
            HandshakeIntitiatorState::Init
        }
        pub fn get_step(&self) -> HandshakeIntitiatorStep { 
            match self {
                HandshakeIntitiatorState::Init => HandshakeIntitiatorStep::Init,
                HandshakeIntitiatorState::SentFirstAwaitSecond(_) => HandshakeIntitiatorStep::SentFirstAwaitSecond,
                HandshakeIntitiatorState::SentThirdAwaitFourth(_) => HandshakeIntitiatorStep::SentThirdAwaitFourth,
                HandshakeIntitiatorState::SentFifthAwaitSixth(_, _, _) => HandshakeIntitiatorStep::SentFifthAwaitSixth,
                HandshakeIntitiatorState::Done(_, _) => HandshakeIntitiatorStep::Done,
            }
        }
    }
    pub struct HandshakeIntitiator {
        /// Technically this will never be None, I'm just convincing the borrow checker to work with me here. 
        last_state: Option<HandshakeIntitiatorState>,
        pub local_noise_keys: snow::Keypair,
        pub local_gestalt_keys: IdentityKeyPair,
    }
    impl HandshakeIntitiator {
        pub fn new(local_noise_keys: snow::Keypair, local_gestalt_keys: IdentityKeyPair) -> Self { 
            HandshakeIntitiator { 
                last_state: Some(HandshakeIntitiatorState::Init), 
                local_noise_keys,
                local_gestalt_keys,
            }
        }
        pub fn send_first(&mut self)
                -> Result<HandshakeStepMessage, HandshakeError> {
            if self.last_state.as_ref().unwrap().get_step() == HandshakeIntitiatorStep::Init {
                let (state, message) = initiate_handshake(&self.local_noise_keys)?;
                self.last_state = Some(HandshakeIntitiatorState::SentFirstAwaitSecond(state));
                Ok(message)
            }
            else { 
                Err(HandshakeError::FirstAfterInit)
            }
        }
        pub fn advance<Callback>(&mut self, incoming: HandshakeStepMessage, receiver_identity: &NodeIdentity, callback_different_key: Callback)
                -> Result<HandshakeNext, HandshakeError> 
                where Callback: FnOnce(&NodeIdentity, &[u8], &[u8]) -> bool {
            match self.last_state.take().unwrap() {
                HandshakeIntitiatorState::Init => {
                    Err(HandshakeError::WrongOrder)
                },
                HandshakeIntitiatorState::SentFirstAwaitSecond(state) => { 
                    let (new_state, message) = initiator_reply(state, incoming, receiver_identity, callback_different_key)?;
                    self.last_state = Some(HandshakeIntitiatorState::SentThirdAwaitFourth(new_state));
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeIntitiatorState::SentThirdAwaitFourth(state) => { 
                    let (transport, message, nonce, seq) = initiator_sign_buf(state, incoming, &self.local_gestalt_keys)?;
                    self.last_state = Some(HandshakeIntitiatorState::SentFifthAwaitSixth(transport, nonce, seq));
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeIntitiatorState::SentFifthAwaitSixth(state, nonce, _seq)  => { 
                    let (transport, new_seq) = initiator_final(state, incoming, receiver_identity, nonce)?;
                    self.last_state = Some(HandshakeIntitiatorState::Done(transport, new_seq));
                    Ok(HandshakeNext::Done)
                },
                HandshakeIntitiatorState::Done(_, _) => {
                    Err(HandshakeError::AdvanceAfterDone)
                },
            }
        }
        pub fn is_done(&self) -> bool { 
            if let HandshakeIntitiatorState::Done(_,_) = &self.last_state.as_ref().unwrap() { 
                true
            }
            else { 
                false
            }
        }
        pub fn complete(mut self) -> Result<(snow::StatelessTransportState, MessageCounter), HandshakeError> { 
            if let HandshakeIntitiatorState::Done(transport, counter) = self.last_state.take().unwrap() { 
                Ok((transport, counter))
            }
            else {
                Err(HandshakeError::CompleteBeforeDone)    
            }
        }
    }
    /// Cleaner state machine wrapper for otherwise-imperative handshake process.
    pub enum HandshakeReceiverState { 
        Init, 
        SentSecondAwaitThird(snow::HandshakeState),
        SentFourthAwaitFifth(snow::StatelessTransportState, [u8;32], MessageCounter),
        Done(snow::StatelessTransportState, MessageCounter),
    }
    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    pub enum HandshakeReceiverStep { 
        Init, 
        SentSecondAwaitThird,
        SentFourthAwaitFifth,
        Done,
    }

    impl HandshakeReceiverState {
        pub fn new() -> Self { 
            HandshakeReceiverState::Init
        }
        pub fn get_step(&self) -> HandshakeReceiverStep { 
            match self {
                HandshakeReceiverState::Init => HandshakeReceiverStep::Init,
                HandshakeReceiverState::SentSecondAwaitThird(_) => HandshakeReceiverStep::SentSecondAwaitThird,
                HandshakeReceiverState::SentFourthAwaitFifth(_, _, _) => HandshakeReceiverStep::SentFourthAwaitFifth,
                HandshakeReceiverState::Done(_,_) => HandshakeReceiverStep::Done,
            }
        }
    }
    pub struct HandshakeReceiver {
        /// Technically this will never be None, I'm just convincing the borrow checker to work with me here. 
        last_state: Option<HandshakeReceiverState>,
        pub local_noise_keys: snow::Keypair,
        pub local_gestalt_keys: IdentityKeyPair,
    }
    impl HandshakeReceiver {
        pub fn new(local_noise_keys: snow::Keypair, local_gestalt_keys: IdentityKeyPair) -> Self { 
            HandshakeReceiver { 
                last_state: Some(HandshakeReceiverState::Init), 
                local_noise_keys,
                local_gestalt_keys,
            }
        }
        pub fn advance<Callback>(&mut self, incoming: HandshakeStepMessage, initiator_identity: &NodeIdentity, callback_different_key: Callback)
                -> Result<HandshakeNext, HandshakeError> 
                where Callback: FnOnce(&NodeIdentity, &[u8], &[u8]) -> bool {
            match self.last_state.take().unwrap() {
                HandshakeReceiverState::Init => { 
                    let (state, message) = receive_initial(&self.local_noise_keys, incoming)?;
                    self.last_state = Some(HandshakeReceiverState::SentSecondAwaitThird(state));
                    debug!("First receiver handshake step.");
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeReceiverState::SentSecondAwaitThird(state) => { 
                    let (new_state, message, nonce, seq) = receive_last_noise(state,incoming, initiator_identity, callback_different_key)?;
                    self.last_state = Some(HandshakeReceiverState::SentFourthAwaitFifth(new_state, nonce, seq));
                    debug!("Second receiver handshake step.");
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeReceiverState::SentFourthAwaitFifth(state, nonce, _seq) => {
                    let (new_state, message, seq) = responder_sign(state,  incoming, &self.local_gestalt_keys, initiator_identity, nonce)?;
                    self.last_state = Some(HandshakeReceiverState::Done(new_state, seq));
                    debug!("Third receiver handshake step.");
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeReceiverState::Done(_, _) => {
                    Err(HandshakeError::AdvanceAfterDone)
                },
            }
        }
        pub fn is_done(&self) -> bool { 
            if let HandshakeReceiverState::Done(_,_) = &self.last_state.as_ref().unwrap() { 
                true
            }
            else { 
                false
            }
        }
        pub fn complete(mut self) -> Result<(snow::StatelessTransportState, MessageCounter), HandshakeError> { 
            if let HandshakeReceiverState::Done(transport, counter) = self.last_state.take().unwrap() { 
                Ok((transport, counter))
            }
            else {
                Err(HandshakeError::CompleteBeforeDone)    
            }
        }
    }
    pub enum HandshakeNext {
        SendMessage(HandshakeStepMessage),
        Done,
    }
}

lazy_static! {
    pub static ref SUPPORTED_PROTOCOL_SET: HashSet<ProtocolDef> = { 
        let mut set = HashSet::new(); 
        set.insert(ProtocolDef{ 
            protocol: current_protocol::PROTOCOL_NAME.to_string(), 
            version: current_protocol::PROTOCOL_VERSION.clone(),
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
    HandshakeError(#[from] current_protocol::HandshakeError),
    #[error("Identity parsing error: {0:?}")]
    IdentityError(#[from] DecodeIdentityError),
    #[error("Attempted to start a handshake, but the Initiator has not provided a node identity.")]
    HandshakeNoIdentity,
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
    peer_identity: Option<NodeIdentity>,
}

impl PreProtocolReceiver { 
    pub fn new(our_identity: IdentityKeyPair) -> Self { 
        PreProtocolReceiver { 
            state: PreProtocolReceiverState::QueryAnswerer, 
            description: serde_json::Value::default(),
            our_identity,
            peer_identity: None,
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
    pub fn complete_handshake(&mut self) -> Result<(snow::StatelessTransportState, current_protocol::MessageCounter), PreProtocolError> { 
        match std::mem::take(&mut self.state) {
            PreProtocolReceiverState::QueryAnswerer => return Err(HandshakeError::CompleteBeforeDone.into()),
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
                let maybe_ident = NodeIdentity::from_base64(&identity); 
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
                    PreProtocolReply::Identity(self.our_identity.public.to_base64())
                )
            },
            PreProtocolQuery::RequestServerStatus => { 
                PreProtocolOutput::Reply( 
                    PreProtocolReply::Status(SERVER_STATUS.clone().lock().clone())
                )
            },
            PreProtocolQuery::RequestServerInfo => { 
                PreProtocolOutput::Reply( 
                    PreProtocolReply::ServerInfo(self.description.clone())
                )
            },
            PreProtocolQuery::StartHandshake(start_handshake) => { 
                debug!("Starting handshake with handshake step {}", start_handshake.handshake.handshake_step);
                let maybe_ident = NodeIdentity::from_base64(&start_handshake.initiator_identity); 
                match maybe_ident { 
                    Ok(ident) => { 
                        self.peer_identity = Some(ident.clone());
                        if !self.state.is_in_handshake() { 
                            let mut receiver_state = HandshakeReceiver::new(load_noise_local_keys()?, self.our_identity.clone());
                            let out = receiver_state.advance(start_handshake.handshake, &ident, callback_different_key);
                            match out { 
                                Ok(current_protocol::HandshakeNext::SendMessage(message)) => { 
                                    self.state = PreProtocolReceiverState::Handshake(receiver_state);
                                    PreProtocolOutput::Reply(PreProtocolReply::Handshake(message))
                                },
                                Ok(current_protocol::HandshakeNext::Done) => return Err(PreProtocolError::NoReplyToStart),
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
                debug!("Handshake step message received: {}", msg.handshake_step);
                match &mut self.state { 
                    PreProtocolReceiverState::Handshake(receiver) => { 
                        let out = receiver.advance(msg, &self.peer_identity.unwrap(), callback_different_key);
                        match out { 
                            Ok(current_protocol::HandshakeNext::SendMessage(message)) => {
                                debug!("Sending handshake step: {}", message.handshake_step);
                                PreProtocolOutput::Reply(PreProtocolReply::Handshake(message))
                            },
                            // Receiver doesn't work this way.
                            Ok(current_protocol::HandshakeNext::Done) => unreachable!(),
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

/// Represents a client who has completed a handshake in the pre-protocol and will now be moving over to the game protocol proper
pub struct SuccessfulConnect {
    pub peer_identity: NodeIdentity,
    pub peer_address: SocketAddr,
    pub transport_cryptography: StatelessTransportState,
    pub transport_sequence_number: u64,
}

pub const PREPROTCOL_PORT: u16 = 54134;
pub const GESTALT_PORT: u16 = 54135;

pub fn write_preprotocol_message(json: &str, stream: &mut TcpStream) -> Result<(), std::io::Error> { 
    let bytes = json.as_bytes();
    let message_len_bytes = (bytes.len() as u32).to_le_bytes();
    assert_eq!(message_len_bytes.len(), 4);
    stream.write_all(&message_len_bytes)?;
    stream.write_all(bytes)?;
    stream.flush()?;
    Ok(())
}

pub fn read_preprotocol_message(stream: &mut TcpStream) -> Result<String, std::io::Error> { 
    let mut next_message_size_buf = [0 as u8; 4];
    stream.read_exact(&mut next_message_size_buf)?;
    stream.flush()?;
    let next_message_size = u32::from_le_bytes(next_message_size_buf);
    let mut message_buf: Vec<u8> = vec![0u8; next_message_size as usize];
    stream.read_exact(&mut message_buf)?;

    Ok(String::from_utf8_lossy(&message_buf).to_string())
}

pub fn preprotocol_receiver_session(our_identity: IdentityKeyPair, peer_address: SocketAddr, mut stream: TcpStream, completed_channel: crossbeam_channel::Sender<SuccessfulConnect>) {
    let mut receiver = PreProtocolReceiver::new(our_identity);
    while match read_preprotocol_message(&mut stream) {
        Ok(msg) => match serde_json::from_str::<PreProtocolQuery>(&msg) { 
                Ok(query) => {
                    match receiver.receive_and_reply(query) {
                        Ok(out) => match out {
                            PreProtocolOutput::Reply(to_send) => {
                                let json_string = serde_json::to_string(&to_send).unwrap();
                                write_preprotocol_message( &json_string, &mut stream).unwrap();

                                match receiver.is_handshake_done() {
                                    true => { 
                                        let (transport, seq) = receiver.complete_handshake().unwrap();
                                        info!("Successfully completed handshake with {}!", peer_address);
                                        let completed = SuccessfulConnect {
                                            peer_identity: receiver.peer_identity.unwrap(),
                                            peer_address,
                                            transport_cryptography: transport,
                                            transport_sequence_number: seq,
                                        };
                                        completed_channel.send(completed).unwrap();
                                        // Done with this part, stop sending. 
                                        false 
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
                            false
                        },
                    }
                },
                Err(e) => { 
                    error!("Error parsing PreProtocolQuery from json received from {}: {:?}", stream.peer_addr().unwrap(), e);
                    false
                },
            },
        Err(_) => {
            error!("Error getting message length from {}", stream.peer_addr().unwrap());
            false
        }
    } {}
    stream.shutdown(Shutdown::Both).unwrap();
}

/// Spawns a thread which listens for pre-protocol connections on TCP.
pub fn launch_preprotocol_listener(our_identity: IdentityKeyPair, our_address: Option<IpAddr>, completed_channel: crossbeam_channel::Sender<SuccessfulConnect>) { 
    std::thread::spawn( move || { 
        let ip = match our_address { 
            Some(value) => value, 
            None => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        };
        let listener = TcpListener::bind(std::net::SocketAddr::new(ip, PREPROTCOL_PORT) ).unwrap();
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    let peer_address = stream.peer_addr().unwrap();
                    info!("New PreProtocol connection: {}", stream.peer_addr().unwrap());
                    let completed_channel_clone = completed_channel.clone();
                    thread::spawn( move || {
                        // connection succeeded
                        preprotocol_receiver_session(our_identity.clone(), peer_address, stream, completed_channel_clone);
                    });
                }
                Err(e) => {
                    error!("PreProtocol connection error: {}", e);
                    /* connection failed */
                }
            }
        }
    });
}

pub fn preprotocol_connect_inner(stream: &mut TcpStream, our_identity: IdentityKeyPair, server_address: SocketAddr) -> Result<SuccessfulConnect, HandshakeError> {
    println!("1"); 
    let callback_different_key = | node_identity: &NodeIdentity, _old_key: &[u8], _new_key: &[u8]| -> bool {
        warn!("Protocol keys for {} have changed. Accepting new key.", node_identity.to_base64());
        true
    };
    // Exchange identities.
    let query_introduce = PreProtocolQuery::Introduction(our_identity.public.to_base64());
    let json_query = serde_json::to_string(&query_introduce)?;
    write_preprotocol_message(&json_query, stream)?;
    println!("2");

    let query_request_identity = PreProtocolQuery::RequestIdentity;
    let json_query = serde_json::to_string(&query_request_identity)?;
    write_preprotocol_message(&json_query, stream)?;
    stream.flush()?;
    println!("3");

    let msg = read_preprotocol_message(stream)?;
    let reply = serde_json::from_str::<PreProtocolReply>(&msg)?;
    let server_identity = if let PreProtocolReply::Identity(identity) = reply { 
        NodeIdentity::from_base64(&identity)?
    } else { 
        return Err(HandshakeError::NoIdentity);
    };
    println!("4");
    
    // Get protocols 
    let query_request_protocols = PreProtocolQuery::SupportedProtocols;
    let json_query = serde_json::to_string(&query_request_protocols)?;
    write_preprotocol_message(&json_query, stream)?;
    stream.flush()?;
    println!("5");
    
    let msg = read_preprotocol_message(stream)?;
    let reply = serde_json::from_str::<PreProtocolReply>(&msg)?;
    let server_protocols = if let PreProtocolReply::SupportedProtocols(protocols) = reply { 
        protocols.supported_protocols
    } else { 
        return Err(HandshakeError::NoProtocolsInCommon);
    };
    println!("6");

    // Figure out which protocol to use. Right now, it's either "the current protocol" or "nothing"
    let current_protocol = ProtocolDef{ 
        protocol: current_protocol::PROTOCOL_NAME.to_string(), 
        version: current_protocol::PROTOCOL_VERSION.clone(),
    };
    if !(server_protocols.contains(&current_protocol)) { 
        return Err(HandshakeError::NoProtocolsInCommon);
    }
    println!("7");

    // Send first handshake message.
    let mut handshake_initiator = HandshakeIntitiator::new(load_noise_local_keys()?, our_identity);
    let handshake_first = handshake_initiator.send_first()?;
    let query = PreProtocolQuery::StartHandshake(StartHandshakeMsg{
        use_protocol: current_protocol.clone(),
        initiator_identity: our_identity.public.to_base64(),
        handshake: handshake_first,
    });
    let json_query = serde_json::to_string(&query)?;
    write_preprotocol_message(&json_query, stream)?;
    println!("8");

    let mut step = 8; 
    // Loop until we're done.
    while !handshake_initiator.is_done() {
        step += 1; 
        println!("{}", step);
        let msg = read_preprotocol_message(stream)?;
        debug!("Got a pre-protocol reply: {}", &msg);
        let reply = serde_json::from_str::<PreProtocolReply>(&msg)?;
        let handshake_step = if let PreProtocolReply::Handshake(step) = reply { 
            step
        } else { 
            return Err(HandshakeError::WrongOrder);
        };

        match handshake_initiator.advance(handshake_step, &server_identity, callback_different_key)? {
            current_protocol::HandshakeNext::SendMessage(msg) => {
                let query = PreProtocolQuery::Handshake(msg);
                let json_query = serde_json::to_string(&query)?;
                write_preprotocol_message(&json_query, stream)?;
            },
            current_protocol::HandshakeNext::Done => break,
        }
    }
    println!("done?");

    // We should be done here! Let's go ahead and connect.

    let (transport, counter) = handshake_initiator.complete()?;

    Ok(SuccessfulConnect{
        peer_identity: server_identity,
        peer_address: server_address,
        transport_cryptography: transport,
        transport_sequence_number: counter,
    })
}

pub fn preprotocol_connect_to_server(our_identity: IdentityKeyPair, server_address: SocketAddr, connect_timeout: Duration, completed_channel: crossbeam_channel::Sender<SuccessfulConnect>) { 
    std::thread::spawn( move || {
        match TcpStream::connect_timeout(&server_address, connect_timeout) {
            Ok(mut stream) => {
                match preprotocol_connect_inner(&mut stream, our_identity, server_address) {
                    Ok(completed_connection) => {
                        completed_channel.send(completed_connection).unwrap();
                    },
                    Err(error) => {
                        error!("Handshake error connecting to server: {:?}", error);
                        let error_to_send = PreProtocolQuery::HandshakeFailed(format!("{:?}", error));
                        let json_error = serde_json::to_string(&error_to_send).unwrap();
                        write_preprotocol_message(&json_error, &mut stream).unwrap();
                    },
                }
                stream.shutdown(Shutdown::Both).unwrap();
            },
            Err(e) => error!("Could not initiate connection to server: {:?}", e),
        }
    });
}

#[test]
fn handshake_test() {
    let bob_gestalt_keys = IdentityKeyPair::generate_for_tests();
    let alice_gestalt_keys = IdentityKeyPair::generate_for_tests();

    let builder: snow::Builder<'_> = snow::Builder::new(current_protocol::NOISE_PARAMS.clone());
    let bob_noise_keys = builder.generate_keypair().unwrap();
    let alice_noise_keys = builder.generate_keypair().unwrap();

    let callback_different_key = |_node_identity: &NodeIdentity, _old_key: &[u8], _new_key: &[u8]| -> bool {
        true
    };
    
    let (bob_state, message_1) = current_protocol::initiate_handshake(&bob_noise_keys).unwrap();
    let (alice_state, message_2) = current_protocol::receive_initial(&alice_noise_keys, message_1).unwrap();
    let (bob_transport, message_3) = current_protocol::initiator_reply(bob_state, message_2, &alice_gestalt_keys.public, callback_different_key).unwrap();
    let (alice_transport, message_4, alice_nonce, _alice_seq) = current_protocol::receive_last_noise(alice_state, message_3, &bob_gestalt_keys.public, callback_different_key).unwrap();
    let (bob_transport, message_5, bob_nonce, _bob_seq) = current_protocol::initiator_sign_buf(bob_transport, message_4, &bob_gestalt_keys).unwrap();
    let (alice_transport, message_6, _alice_seq) = current_protocol::responder_sign(alice_transport, message_5, &alice_gestalt_keys, &bob_gestalt_keys.public, alice_nonce).unwrap();
    let (bob_transport, _bob_seq) = current_protocol::initiator_final(bob_transport, message_6, &alice_gestalt_keys.public, bob_nonce).unwrap();

    // Try sending a message!
    let mut write_buf = [0u8; 1024];
    let write_len = bob_transport.write_message(1337, "Hello!".as_bytes(), &mut write_buf).unwrap();

    let mut read_buf = [0u8; 1024];
    let read_len = alice_transport.read_message(1337, &write_buf[0..write_len], &mut read_buf).unwrap();
    let read_result = String::from_utf8_lossy(&read_buf[0..read_len]).to_string(); 
    assert!(read_result.as_str() == "Hello!");

    // Now let's try messing up on purpose.
    let mut write_buf = [0u8; 1024];
    let write_len = bob_transport.write_message(123, "This should fail!".as_bytes(), &mut write_buf).unwrap();

    // This should fail because sequence number doesn't match
    let mut read_buf = [0u8; 1024];
    let _err = alice_transport.read_message(456, &write_buf[0..write_len], &mut read_buf).unwrap_err();
}
#[test]
fn handshake_test_state_machine() {
    let bob_gestalt_keys = IdentityKeyPair::generate_for_tests();
    let alice_gestalt_keys = IdentityKeyPair::generate_for_tests();

    let builder: snow::Builder<'_> = snow::Builder::new(current_protocol::NOISE_PARAMS.clone());
    let bob_noise_keys = builder.generate_keypair().unwrap();
    let alice_noise_keys = builder.generate_keypair().unwrap();
    
    let mut initiator = current_protocol::HandshakeIntitiator::new(bob_noise_keys, bob_gestalt_keys.clone());
    let mut receiver = current_protocol::HandshakeReceiver::new(alice_noise_keys, alice_gestalt_keys.clone());

    let callback_different_key = |_node_identity: &NodeIdentity, _old_key: &[u8], _new_key: &[u8]| -> bool {
        true
    };
    
    let first_message = initiator.send_first().unwrap();
    let mut bobs_turn = false; 

    let mut steps_counter: usize = 1; // Step 1 is first message, we just sent that. 

    let mut last_message = Some(current_protocol::HandshakeNext::SendMessage(first_message));
    while let current_protocol::HandshakeNext::SendMessage(msg) = last_message.take().unwrap() {
        println!("{:?}", &msg);
        if bobs_turn {
            last_message = Some(initiator.advance(msg, &alice_gestalt_keys.public, callback_different_key).unwrap());
        }
        else { 
            last_message = Some(receiver.advance(msg, &bob_gestalt_keys.public, callback_different_key).unwrap())
        }
        steps_counter += 1;
        if steps_counter > 7 { 
            panic!("Too many steps!");
        }
        bobs_turn = !bobs_turn;
    }
    // Breaking this loop requires encountering a HandshakeNext::Done

    assert!(initiator.is_done());
    let (_bob_transport, _bob_seq) = initiator.complete().unwrap();

    assert!(receiver.is_done());
    let (_alice_transport, _alice_seq) = receiver.complete().unwrap();
}