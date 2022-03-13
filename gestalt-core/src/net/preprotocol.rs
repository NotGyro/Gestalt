//! The "pre-protocol" is a TCP connection established between two Gestalt nodes to exchange information about 
//! which protocols they support, which versions of these protocols they support, and other metadata 
//! such as "server name", estimating RTT, cryptographic primitives supported, etc. All of this happens *before*, 
//! not simultaneously with, any gameplay or exchange of content-addressed Gestalt resources.
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
use serde::{Serialize, Deserialize};
use snow::params::NoiseParams;

use crate::common::identity::IdentityKeyPair;
use crate::common::{identity::NodeIdentity, Version};

use std::thread;
use std::net::{TcpListener, TcpStream, Shutdown};
use std::io::{Read, Write};

// TODO/NOTE - Cryptography should behave differently on known long-term static public key and unknown long-term static public key. 

// Types of pre-protocol request / response pairs:
// * What's your name/alias?
// * Is the server currently full? Is it unavailable to join for some other reasons? 
// * I want to connect, what handshake protocols & game protocols do you support? 
// That third one starts a state machine for connection.

/// Represents a supported version of a supported protocol. 
#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, PartialOrd, Eq, Ord)]
pub struct ProtocolDef {
    /// Name of the protocol, such as "gestalt-laminar"
    pub protocol: String,
    #[serde(with = "crate::common::version_string")] 
    pub version: Version,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportedProtocols {
    pub game_protocols: HashSet<ProtocolDef>, 
    pub handshake_protocols: HashSet<ProtocolDef>, 
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerStatus { 
    NoResponse,
    Unavailable,
    Starting,
    Ready,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HandshakeStep { 
    pub handshake_step: u8, 
    pub data: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StartHandshakeMsg { 
    pub use_protocol: ProtocolDef, 
    pub message: HandshakeStep, 
}

#[derive(Debug, Serialize, Deserialize)]
/// Every variant of this EXCEPT StartHandshake and HandshakeStep 
pub enum PreProtocolQuery {
    /// Find out which protocols the server supports. 
    SupportedProtocols(SupportedProtocols),
    /// Is the server ready to join? 
    RequestServerStatus,
    /// Asks for the name, current playercount, etc of the server. 
    /// Response will be json that is not guaranteed to be in any particular structure 
    RequestServerInfo,
    /// Initiates a handshake, providing the handshake protocol definition of the handshake we will use.
    StartHandshake(StartHandshakeMsg),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PreProtocolReply {
    Status(ServerStatus),
    /// Name, current playercount, etc of the server. 
    /// Response will be json that is not guaranteed to be in any particular structure 
    ServerInfo(serde_json::Value),
}

// 32 bytes, 256 bits. 
pub const SESSION_KEY_LEN: usize = 32;

pub mod gestalt_handshake_current {
    use curve25519_dalek::digest::Digest;

    use log::debug;

    use super::*;
    use crate::common::identity::IdentityKeyPair;
    use crate::common::{identity::NodeIdentity, Version};

    const HANDSHAKE_VERSION: Version = version!(1,0,0);
    const HANDSHAKE_NAME: &'static str = "Gestalt_Noise_XX";

    const NOISE_PARAM_STR: &'static str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

    lazy_static! {
        static ref NOISE_PARAMS: NoiseParams = NOISE_PARAM_STR.parse().unwrap();
    }

    /// Mangle (or unmangle, depending on perspective) private key such that ed25519-dalek and Snow get along. 
    fn modify_key(input: &[u8]) -> [u8; 32] {
        let mut h: ed25519_dalek::Sha512 = ed25519_dalek::Sha512::new();
        let mut hash: [u8; 64] = [0u8; 64];
        let mut digest: [u8; 32] = [0u8; 32];

        h.update(input);
        hash.copy_from_slice(h.finalize().as_slice());

        digest.copy_from_slice(&hash[..32]);
        digest
    } 
    
    #[derive(thiserror::Error, Debug)]
    pub enum HandshakeError {
        #[error("cryptographic error in noise protocol implementation (snow): {0:?}")]
        SnowError(#[from] snow::Error),
        #[error("error decoding or encoding Json message for handshake: {0:?}")]
        JsonError(#[from] serde_json::Error),
        #[error("error decoding Base-64 bytes for handshake: {0:?}")]
        Base64Error(#[from] base64::DecodeError),
        /// Expected, received.
        #[error("Unexpected step in handshake process - expected {0}, got a handshake step message at {1}")]
        UnexpectedStep(u8, u8),
        #[error("Attempted to send a Gestalt handshake message on the Handshake channel before the Noise handshake was done")]
        SendBeforeNoiseDone,
        #[error("The other side's attempt to sign the nonce we gave it resulted in a signature which seems invalid.")]
        BadSignature,
    }
    /*pub struct HandshakeDoneOutput {
        pub local_session_key: Vec<u8>, 
        pub remote_session_key: Vec<u8>,
        pub remote_identity: NodeIdentity,
    }*/

    // Handshake steps:
    // * Step 0 is reserved.
    // * Step 1: Initiator starts handshake. "e ->" in Noise protocol terms  
    // * Step 2: Responder sends response. Noise step: "<- e, ee, s, es". This means the initiator has the responder's [`crate::common::identity::NodeIdentity`]
    // * Step 3: Initiator sends closing Noise protocol response. Noise step: "s, se ->" This means we can call handshake_state.into_transport_mode(self), giving us a secure channel.
    // * Both sides have transformed this into a snow::StatelessTransportState.

    
    /// Initiator-sided, step 1
    pub fn initiate_handshake(keys: &IdentityKeyPair) -> Result<(snow::HandshakeState, HandshakeStep), HandshakeError> { 
        let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
        let key = keys.private.get_bytes();
        let mut noise = builder.local_private_key(key).build_initiator()?;

        // Generate "-> e" message
        let mut first_message_buf = [0u8; 1024];
        let wrote_len = noise.write_message(&[], &mut first_message_buf)?;
        debug!("Wrote a handshake initiator message which is {} bytes long", wrote_len);
        // Encode
        let msg = HandshakeStep {
            handshake_step: 1,
            data: base64::encode(&first_message_buf[0..wrote_len]),
        };

        Ok(
            (noise, msg)
        )
    }
    
    /// Initiator-sided, step 3
    pub fn initiator_reply(mut state: snow::HandshakeState, input: HandshakeStep) -> Result<(snow::StatelessTransportState, HandshakeStep), HandshakeError> { 
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
            let output = HandshakeStep { 
                handshake_step: 3, 
                data: base64::encode(&send_buf[0..wrote_len]),
            };

            // Turn handshake state into a transport
            let transport = state.into_stateless_transport_mode()?;

            Ok((transport, output))
        } else { 
            Err(HandshakeError::UnexpectedStep(2, input.handshake_step))
        }
    }

    /// Receiver-sided, step 2
    pub fn receive_initial(keys: &IdentityKeyPair, input: HandshakeStep) -> Result<(snow::HandshakeState, HandshakeStep), HandshakeError> { 
        if input.handshake_step == 1 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 1024];

            let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
            let key = keys.private.get_bytes();
            let mut state = builder.local_private_key(key).build_responder()?;

            // Read their message.
            let read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);
    
            // Generate "e, ee, s, es" message
            let mut write_buf = [0u8; 1024];
            let wrote_len = state.write_message(&[], &mut write_buf)?;
            debug!("Wrote a handshake responder message which is {} bytes long", wrote_len);
            // Encode
            let msg = HandshakeStep {
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
    
    /// Receiver-sided, receive step 3 message from initiator and finish handshake. 
    pub fn receive_last(mut state: snow::HandshakeState, input: HandshakeStep) -> Result<snow::StatelessTransportState, HandshakeError> { 
        if input.handshake_step == 3 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 1024];

            // Read their message.
            let read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
            debug!("Noise handshake message came with {}", read_buf_len);

            // Turn handshake state into a transport
            let transport = state.into_stateless_transport_mode()?;

            Ok(transport)
        } else { 
            Err(HandshakeError::UnexpectedStep(2, input.handshake_step))
        }
    }
}

pub use gestalt_handshake_current as handshake;

#[test]
fn handshake_test() { 
    let bob_keys = IdentityKeyPair::generate_for_tests();
    let alice_keys = IdentityKeyPair::generate_for_tests();

    let (bob_state, message_1) = handshake::initiate_handshake(&bob_keys).unwrap();
    let (alice_state, message_2) = handshake::receive_initial(&alice_keys, message_1).unwrap();
    let (bob_transport, message_3) = handshake::initiator_reply(bob_state, message_2).unwrap();
    let alice_transport = handshake::receive_last(alice_state, message_3).unwrap();

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