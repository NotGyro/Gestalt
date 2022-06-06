

    use std::fs::{OpenOptions, self};
    use std::io::{Read, Write};
    use std::path::PathBuf;

    use log::{debug, warn};
    use rand::Rng;
    use serde::{Serialize, Deserialize};
    use signature::Signature;
    use snow::params::NoiseParams;
    
    use crate::common::identity::{IdentityKeyPair, DecodeIdentityError};
    use crate::common::{identity::NodeIdentity, Version};
    use crate::net::protocol_store_dir;
    use lazy_static::lazy_static;

    use super::{SessionId, MessageCounter};
    use super::preprotocol::HandshakeStepMessage;

    pub const PROTOCOL_VERSION: Version = version!(1,0,0);
    pub const PROTOCOL_NAME: &str = "gestalt_noise_laminar_udp";

    pub const NOISE_PARAM_STR: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

    lazy_static! {
        pub(crate) static ref NOISE_PARAMS: NoiseParams = NOISE_PARAM_STR.parse().unwrap();
    }

    pub const fn truncate_to_session_id(val: &[u8]) -> SessionId {
        assert!(val.len() >= 4);
        [val[0], val[1], val[2], val[3]]
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
        #[error("Key challenge header failed to validate in handshake.")]
        BadChallengeHeader,
    }

    pub fn noise_protocol_dir() -> PathBuf {
        const SUB_DIR: &str = "noise/";
        let path = protocol_store_dir().join(PathBuf::from(SUB_DIR));
        if !path.exists() {
            fs::create_dir_all(&path).unwrap();
        }
        path
    }

    pub fn noise_peer_dir() -> PathBuf {
        const SUB_DIR: &str = "peers/";
        let path = noise_protocol_dir().join(PathBuf::from(SUB_DIR));
        if !path.exists() { 
            fs::create_dir_all(&path).unwrap(); 
        }
        path
    }

    pub fn load_noise_local_keys(our_ident:NodeIdentity) -> Result<snow::Keypair, HandshakeError> { 
        let filename = format!("local_key_{}", our_ident.to_base64()); 
        let path = noise_protocol_dir().join(PathBuf::from(filename));
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
            
            if public == peer_noise_key {
                // Valid identity, this is what we were expecting.
                Ok(())
            } else if callback_nonmatching(peer_identity, &public, peer_noise_key) { 
                // Our request to go forward with using this unfamiliar key is approved. Store the new key. 
                let mut file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(false)
                    .open(&path)?;

                file.write_all(peer_noise_key)?;
                file.flush()?;

                Ok(())
            }
            else { 
                Err(HandshakeError::IdentityChanged(peer_identity.to_base64()))
            }
        }
        else {
            warn!("Storing new noise keys for unfamiliar peer {:?}", peer_identity);
            
            let mut file = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(false)
                .open(&path)?;

            file.write_all(peer_noise_key)?;
            file.flush()?;
            Ok(())
        }
    }

    /// Header used to ensure the process of proving we own our public key can't be used to sign arbitrary things / impersonate us. 
    /// Transmitted as json
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash)]
    pub(crate) struct KeyChallenge { 
        /// Signed on a constant string to make it harder to forge this. 
        pub static_challenge_name: String,
        /// Identity of the user sending this challenge, base-64.
        pub sender_ident: String,
        /// Identity of the user receiving this challenge, base-64.
        pub receiver_ident: String,
        /// Base-64 random challenge bytes.
        pub challenge: String,
    } 

    pub const CHALLENGE_NAME: &str = "GESTALT_IDENTITY_CHALLENGE";

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
            let _read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
    
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
            -> Result<(snow::StatelessTransportState, HandshakeStepMessage, SessionId), HandshakeError> 
            where Callback: FnOnce(&NodeIdentity, &[u8], &[u8]) -> bool { 
        if input.handshake_step == 2 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 1024];

            // Read their message.
            let _read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;

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

            let handshake_hash = state.get_handshake_hash();
            let session_id = truncate_to_session_id(handshake_hash);

            // Turn handshake state into a transport
            let transport = state.into_stateless_transport_mode()?;

            Ok((transport, output, session_id))
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
        /// Contains a Base-64 encoded buf to sign.
        pub please_sign: String,
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct HandshakeMessage5 {
        /// Base-64 encoded initiator signature on responder's HandshakeMessage4 "please_sign"
        pub initiator_signature: String,
        /// Contains a Base-64 encoded buf to sign.
        pub please_sign: String,
    }
    
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct HandshakeMessage6 {
        /// Base-64 encoded responder signature on initiator's HandshakeMessage5 "please_sign"
        pub responder_signature: String,
    }

    /// Receiver-sided, receive step 3 message from initiator and finish Noise handshake. Now we do funky identity stuff, sending a buffer and asking the other side to sign it. 
    pub fn receive_last_noise<Callback>(mut state: snow::HandshakeState, input: HandshakeStepMessage, peer_gestalt_identity: &NodeIdentity, our_gestalt_identity: &NodeIdentity, callback_different_key: Callback) 
        -> Result<(snow::StatelessTransportState, HandshakeStepMessage, String, MessageCounter, SessionId), HandshakeError>
            where Callback: FnOnce(&NodeIdentity, &[u8], &[u8]) -> bool { 
        if input.handshake_step == 3 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 1024];

            // Read their message.
            let _read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;

            // Get Noise key.
            let remote_static = state.get_remote_static().ok_or(HandshakeError::MissingRemoteStatic(3))?;
            // Make sure we notice if the key changed.
            load_validate_noise_peer_key(peer_gestalt_identity, remote_static, callback_different_key)?;

            let handshake_hash = state.get_handshake_hash();
            let session_id = truncate_to_session_id(handshake_hash);
            
            // Turn handshake state into a transport
            let transport = state.into_stateless_transport_mode()?;

            // Build a HandshakeMessage asking for a signature. 
            let nonce = make_signing_nonce();
            let base64_nonce = base64::encode(&nonce); 
            let challenge = KeyChallenge {
                static_challenge_name: CHALLENGE_NAME.to_string(),
                sender_ident: our_gestalt_identity.to_base64(),
                receiver_ident: peer_gestalt_identity.to_base64(),
                challenge: base64_nonce,
            };
            let json_challenge = serde_json::to_string(&challenge)?;
            let b64_challenge = base64::encode(&json_challenge.as_bytes());
            let message = HandshakeMessage4 { 
                please_sign: b64_challenge.clone(),
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
            
            Ok((transport, step, b64_challenge, 0, session_id))
        } else { 
            Err(HandshakeError::UnexpectedStep(3, input.handshake_step))
        }
    }
    /// Receive step 4 message, produce step 5 message.
    pub fn initiator_sign_buf(state: snow::StatelessTransportState, input: HandshakeStepMessage, their_key: NodeIdentity, our_keys: &IdentityKeyPair) -> Result<(snow::StatelessTransportState, HandshakeStepMessage, String, MessageCounter), HandshakeError> { 
        if input.handshake_step == 4 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 65535];

            // Read their message.
            let read_buf_len = state.read_message(0, &bytes_input, &mut read_buf)?;

            // Get the inner message.
            let msg: HandshakeMessage4 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
            let challenge_string = msg.please_sign.clone();
            let our_signature = our_keys.sign(challenge_string.as_bytes()).map_err(HandshakeError::CannotSign)?; 
            let our_signature_bytes = our_signature.as_bytes();
            let our_signature_b64 = base64::encode(our_signature_bytes);

            // Validate header
            let resulting_string = String::from_utf8(
                base64::decode(msg.please_sign).map_err(|_| HandshakeError::BadChallengeHeader)?
                ).map_err(|_| HandshakeError::BadChallengeHeader)?;
            let challenge: KeyChallenge = serde_json::from_str(&resulting_string)
                .map_err(|_| HandshakeError::BadChallengeHeader)?;

            if challenge.static_challenge_name != CHALLENGE_NAME { 
                return Err(HandshakeError::BadChallengeHeader);
            }

            let decoded_sender_key = NodeIdentity::from_base64(&challenge.sender_ident)
                .map_err(|_| HandshakeError::BadChallengeHeader)?;
            if decoded_sender_key != their_key { 
                return Err(HandshakeError::BadChallengeHeader);
            }
            let decoded_receiver_key = NodeIdentity::from_base64(&challenge.receiver_ident)
                .map_err(|_| HandshakeError::BadChallengeHeader)?;
            if decoded_receiver_key != our_keys.public { 
                return Err(HandshakeError::BadChallengeHeader);
            }
            
            // Build a HandshakeMessage asking for a signature.
            let nonce = make_signing_nonce();
            let base64_nonce = base64::encode(&nonce); 
            let challenge = KeyChallenge {
                static_challenge_name: CHALLENGE_NAME.to_string(),
                sender_ident: our_keys.public.to_base64(),
                receiver_ident: their_key.to_base64(),
                challenge: base64_nonce,
            };
            let json_challenge = serde_json::to_string(&challenge)?;
            let b64_challenge = base64::encode(&json_challenge.as_bytes());
            let message = HandshakeMessage5 { 
                please_sign: b64_challenge.clone(),
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
            
            Ok((state, step, b64_challenge, 0))
        } else { 
            Err(HandshakeError::UnexpectedStep(4, input.handshake_step))
        }
    } 
    /// Receive step 5 message, produce step 6 message.
    pub fn responder_sign(state: snow::StatelessTransportState, input: HandshakeStepMessage, our_keys: &IdentityKeyPair, peer_identity: &NodeIdentity, our_challenge: String) -> Result<(snow::StatelessTransportState, HandshakeStepMessage, MessageCounter), HandshakeError> { 
        if input.handshake_step == 5 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 65535];

            // Read their message.
            let read_buf_len = state.read_message(0, &bytes_input, &mut read_buf)?;

            // Get the inner message.
            let msg: HandshakeMessage5 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
            
            // Validate their signature
            let their_sig = base64::decode(msg.initiator_signature.as_bytes())?;
            peer_identity.verify_signature(our_challenge.as_bytes(), &their_sig).map_err(HandshakeError::BadSignature)?;

            let challenge_string = msg.please_sign.clone();
            // Make our signature.
            let our_signature = our_keys.sign(challenge_string.as_bytes()).map_err(HandshakeError::CannotSign)?;
            let our_signature_bytes = our_signature.as_bytes();
            let our_signature_b64 = base64::encode(our_signature_bytes);

            // Validate header
            let resulting_string = String::from_utf8(
                base64::decode(msg.please_sign).map_err(|_| HandshakeError::BadChallengeHeader)?
                ).map_err(|_| HandshakeError::BadChallengeHeader)?;
            let challenge: KeyChallenge = serde_json::from_str(&resulting_string)
                .map_err(|_| HandshakeError::BadChallengeHeader)?;

            if challenge.static_challenge_name != CHALLENGE_NAME { 
                return Err(HandshakeError::BadChallengeHeader);
            }

            let decoded_sender_key = NodeIdentity::from_base64(&challenge.sender_ident)
                .map_err(|_| HandshakeError::BadChallengeHeader)?;
            if decoded_sender_key != *peer_identity { 
                return Err(HandshakeError::BadChallengeHeader);
            }
            let decoded_receiver_key = NodeIdentity::from_base64(&challenge.receiver_ident)
                .map_err(|_| HandshakeError::BadChallengeHeader)?;
            if decoded_receiver_key != our_keys.public { 
                return Err(HandshakeError::BadChallengeHeader);
            };
            
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
    pub fn initiator_final(state: snow::StatelessTransportState, input: HandshakeStepMessage, peer_identity: &NodeIdentity, our_challenge: String) -> Result<(snow::StatelessTransportState, MessageCounter), HandshakeError> { 
        if input.handshake_step == 6 {
            let bytes_input = base64::decode(input.data)?;
            let mut read_buf = [0u8; 65535];

            // Read their message.
            let read_buf_len = state.read_message(1, &bytes_input, &mut read_buf)?;
            
            // Get the inner message.
            let msg: HandshakeMessage6 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
            // Validate their signature
            let their_sig = base64::decode(msg.responder_signature)?;
            peer_identity.verify_signature(our_challenge.as_bytes(), &their_sig).map_err(HandshakeError::BadSignature)?;

            Ok((state, 1))
        } else { 
            Err(HandshakeError::UnexpectedStep(4, input.handshake_step))
        }
    }

    /// Cleaner state machine wrapper for otherwise-imperative handshake process.
    pub enum HandshakeIntitiatorState { 
        Init, 
        SentFirstAwaitSecond(snow::HandshakeState),
        SentThirdAwaitFourth(snow::StatelessTransportState, SessionId),
        SentFifthAwaitSixth(snow::StatelessTransportState, String, MessageCounter, SessionId),
        Done(snow::StatelessTransportState, MessageCounter, SessionId),
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
                HandshakeIntitiatorState::SentThirdAwaitFourth(_, _) => HandshakeIntitiatorStep::SentThirdAwaitFourth,
                HandshakeIntitiatorState::SentFifthAwaitSixth(_, _, _, _) => HandshakeIntitiatorStep::SentFifthAwaitSixth,
                HandshakeIntitiatorState::Done(_, _, _) => HandshakeIntitiatorStep::Done,
            }
        }
    }
    impl Default for HandshakeIntitiatorState {
        fn default() -> Self {
            Self::new()
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
                    let (new_state, message, sid) = initiator_reply(state, incoming, receiver_identity, callback_different_key)?;
                    self.last_state = Some(HandshakeIntitiatorState::SentThirdAwaitFourth(new_state, sid));
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeIntitiatorState::SentThirdAwaitFourth(state, sid) => { 
                    let (transport, message, nonce, seq) = initiator_sign_buf(state, incoming, *receiver_identity, &self.local_gestalt_keys)?;
                    self.last_state = Some(HandshakeIntitiatorState::SentFifthAwaitSixth(transport, nonce, seq, sid));
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeIntitiatorState::SentFifthAwaitSixth(state, nonce, _seq, sid)  => { 
                    let (transport, new_seq) = initiator_final(state, incoming, receiver_identity, nonce)?;
                    self.last_state = Some(HandshakeIntitiatorState::Done(transport, new_seq, sid));
                    Ok(HandshakeNext::Done)
                },
                HandshakeIntitiatorState::Done(_, _, _) => {
                    Err(HandshakeError::AdvanceAfterDone)
                },
            }
        }
        pub fn is_done(&self) -> bool { 
            matches!(self.last_state.as_ref().unwrap(), HandshakeIntitiatorState::Done(_,_,_))
        }
        pub fn complete(mut self) -> Result<(snow::StatelessTransportState, MessageCounter, SessionId), HandshakeError> { 
            if let HandshakeIntitiatorState::Done(transport, counter, session_id) = self.last_state.take().unwrap() { 
                Ok((transport, counter, session_id))
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
        SentFourthAwaitFifth(snow::StatelessTransportState, String, MessageCounter, SessionId),
        Done(snow::StatelessTransportState, MessageCounter, SessionId),
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
                HandshakeReceiverState::SentFourthAwaitFifth(_, _, _, _) => HandshakeReceiverStep::SentFourthAwaitFifth,
                HandshakeReceiverState::Done(_,_,_) => HandshakeReceiverStep::Done,
            }
        }
    }
    impl Default for HandshakeReceiverState {
        fn default() -> Self {
            Self::new()
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
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeReceiverState::SentSecondAwaitThird(state) => { 
                    let (new_state, message, nonce, seq, sid) = receive_last_noise(state,incoming, initiator_identity, &self.local_gestalt_keys.public, callback_different_key)?;
                    self.last_state = Some(HandshakeReceiverState::SentFourthAwaitFifth(new_state, nonce, seq, sid));
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeReceiverState::SentFourthAwaitFifth(state, nonce, _seq, sid) => {
                    let (new_state, message, seq) = responder_sign(state,  incoming, &self.local_gestalt_keys, initiator_identity, nonce)?;
                    self.last_state = Some(HandshakeReceiverState::Done(new_state, seq, sid));
                    Ok(HandshakeNext::SendMessage(message))
                },
                HandshakeReceiverState::Done(_, _, _) => {
                    Err(HandshakeError::AdvanceAfterDone)
                },
            }
        }
        pub fn is_done(&self) -> bool { 
            matches!(self.last_state.as_ref().unwrap(), HandshakeReceiverState::Done(_,_,_))
        }
        pub fn complete(mut self) -> Result<(snow::StatelessTransportState, MessageCounter, SessionId), HandshakeError> { 
            if let HandshakeReceiverState::Done(transport, counter, session_id) = self.last_state.take().unwrap() { 
                Ok((transport, counter, session_id))
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

    
#[test]
fn handshake_test() {
    let bob_gestalt_keys = IdentityKeyPair::generate_for_tests();
    let alice_gestalt_keys = IdentityKeyPair::generate_for_tests();

    let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
    let bob_noise_keys = builder.generate_keypair().unwrap();
    let alice_noise_keys = builder.generate_keypair().unwrap();

    let callback_different_key = |_node_identity: &NodeIdentity, _old_key: &[u8], _new_key: &[u8]| -> bool {
        true
    };
    
    let (bob_state, message_1) = initiate_handshake(&bob_noise_keys).unwrap();
    let (alice_state, message_2) = receive_initial(&alice_noise_keys, message_1).unwrap();
    let (bob_transport, message_3, bob_session_id) = initiator_reply(bob_state, message_2, &alice_gestalt_keys.public, callback_different_key).unwrap();
    let (alice_transport, message_4, alice_nonce, _alice_seq, alice_session_id) = receive_last_noise(alice_state, message_3, &bob_gestalt_keys.public, &alice_gestalt_keys.public, callback_different_key).unwrap();
    let (bob_transport, message_5, bob_nonce, _bob_seq) = initiator_sign_buf(bob_transport, message_4, alice_gestalt_keys.public, &bob_gestalt_keys).unwrap();
    let (alice_transport, message_6, _alice_seq) = responder_sign(alice_transport, message_5, &alice_gestalt_keys, &bob_gestalt_keys.public, alice_nonce).unwrap();
    let (bob_transport, _bob_seq) = initiator_final(bob_transport, message_6, &alice_gestalt_keys.public, bob_nonce).unwrap();

    assert_eq!(bob_session_id, alice_session_id);

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

    // This should fail because counter doesn't match
    let mut read_buf = [0u8; 1024];
    let _err = alice_transport.read_message(456, &write_buf[0..write_len], &mut read_buf).unwrap_err();
}
#[test]
fn handshake_test_state_machine() {
    let bob_gestalt_keys = IdentityKeyPair::generate_for_tests();
    let alice_gestalt_keys = IdentityKeyPair::generate_for_tests();

    let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
    let bob_noise_keys = builder.generate_keypair().unwrap();
    let alice_noise_keys = builder.generate_keypair().unwrap();
    
    let mut initiator = HandshakeIntitiator::new(bob_noise_keys, bob_gestalt_keys);
    let mut receiver = HandshakeReceiver::new(alice_noise_keys, alice_gestalt_keys);

    let callback_different_key = |_node_identity: &NodeIdentity, _old_key: &[u8], _new_key: &[u8]| -> bool {
        true
    };
    
    let first_message = initiator.send_first().unwrap();
    let mut bobs_turn = false; 

    let mut steps_counter: usize = 1; // Step 1 is first message, we just sent that. 

    let mut last_message = Some(HandshakeNext::SendMessage(first_message));
    while let HandshakeNext::SendMessage(msg) = last_message.take().unwrap() {
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
    let (_bob_transport, _bob_seq, bob_session_id) = initiator.complete().unwrap();

    assert!(receiver.is_done());
    let (_alice_transport, _alice_seq, alice_session_id) = receiver.complete().unwrap();

    assert_eq!(bob_session_id, alice_session_id);
}