use base64::Engine;
use semver::Version;
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::fs::OpenOptions;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use log::{error, info, trace};
use rand::Rng;
use serde::{Deserialize, Serialize};
use snow::params::NoiseParams;

use crate::common::identity::{DecodeIdentityError, IdentityKeyPair};
use crate::common::identity::NodeIdentity;
use crate::message::{BroadcastReceiver, BroadcastSender, MessageReceiverAsync};
use lazy_static::lazy_static;

use super::preprotocol::HandshakeStepMessage;
use super::{MessageCounter, SessionId};

use base64::engine::general_purpose::URL_SAFE as BASE_64;

pub const PROTOCOL_VERSION: Version = Version::new(0, 0, 1);
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
	#[error("network i/o error: {0:?}")]
	NetIoError(std::io::Error),
	#[error("i/o error from loading stored keys: {0:?}")]
	ProtocolStoreIoError(std::io::Error),
	#[error("could not parse peer key file as a MessagePack message: {0:?}")]
	ProtocolStoreDecodeError(#[from] rmp_serde::decode::Error),
	#[error("could not write peer keys as a MessagePack message: {0:?}")]
	ProtocolStoreEncodeError(#[from] rmp_serde::encode::Error),
	/// Expected, received.
	#[error(
		"Unexpected step in handshake process - expected {0}, got a handshake step message at {1}"
	)]
	UnexpectedStep(u8, u8),
	#[error(
		"Attempted to send a Gestalt handshake message on the Handshake channel before the Noise handshake was done"
	)]
	SendBeforeNoiseDone,
	#[error("Protocol key for node {0} changed, and the handler for this situation denied accepting the new key.")]
	IdentityChanged(String),
	#[error("Wrong-size protocol key received - these must be 32 bytes long and we received a {0}-byte key.")]
	ProtocolKeyWrongSize(usize),
	#[error("Remote static noise key was expected at handshake step {0}, but it was not present!")]
	MissingRemoteStatic(u8),
	#[error(
		"Unable to sign a buffer so that we can confirm our identity to a peer in a handshake: {0}"
	)]
	CannotSign(ed25519_dalek::SignatureError),
	#[error("Gestalt signature sent to us by a peer did not pass validation. The other side's attempt to sign the nonce we gave it resulted in a signature which seems invalid.")]
	BadSignature(ed25519_dalek::SignatureError),
	#[error("Handshake messages were sent in the wrong order")]
	WrongOrder,
	#[error(
		"Called send_first() on a handshake initiator more than once, or after calling advance()"
	)]
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
	#[error("Timeout in handshake: {0}.")]
	Timeout(#[from] tokio::time::error::Elapsed),
	#[error("Mismatch channel closed")]
	MismatchChannelClosed,
	#[error("Mismatch channel instances were used already")]
	NoMismatchChannels,
	#[error("Bad signature length. Expected 64 bytes, got: {0}")]
	SignatureLengthWrong(usize),
}

fn buf_to_64(buf: &Vec<u8>) -> Result<[u8; 64], usize> {
	if buf.len() != 64 {
		return Err(buf.len());
	}
	let mut out: [u8; 64] = [0; 64];
	out.copy_from_slice(&buf);
	Ok(out)
}

pub(super) fn noise_protocol_dir(protocol_store_dir: &PathBuf) -> PathBuf {
	const SUB_DIR: &str = "noise/";
	let path = protocol_store_dir.join(PathBuf::from(SUB_DIR));
	if !path.exists() {
		std::fs::create_dir_all(&path).unwrap();
	}
	path
}

fn peer_dir(noise_dir: &PathBuf) -> PathBuf {
	const SUB_DIR: &str = "peers/";
	let path = noise_dir.join(PathBuf::from(SUB_DIR));
	if !path.exists() {
		std::fs::create_dir_all(&path).unwrap();
	}
	path
}

// TODO: When MpscChannels are implemented, use that instead.
// This is a devilishly messy way of doing it and I hate it too, but
// there's not really another good way to do this. Every other way I've
// tried results in my async fn futures not being Send.
pub type NewProtocolKeyReporter = BroadcastSender<NodeIdentity>;
pub type NewProtocolKeyApprover = BroadcastReceiver<(NodeIdentity, bool)>;

// I'm aware that right now I could just write this to / from the file as a series of 32-byte keys
// but we may need to extend it later, so I'm using this as a struct and serializing / deserializing
// with Serde.
#[derive(Serialize, Deserialize, Clone)]
struct PeerKeyFile {
	pub keys: HashSet<[u8; 32]>,
}

pub async fn load_noise_local_keys(
	noise_dir: PathBuf,
	our_ident: NodeIdentity,
) -> Result<snow::Keypair, HandshakeError> {
	let filename = format!("local_key_{}", our_ident.to_base64());
	let path = noise_dir.join(PathBuf::from(filename));
	let keypair = if path.exists() {
		let mut private = [0u8; 32];
		let mut public = [0u8; 32];

		let mut file = OpenOptions::new()
			.create(false)
			.read(true)
			.open(&path)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;

		file.read_exact(&mut private)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		file.read_exact(&mut public)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;

		snow::Keypair {
			private: private.to_vec(),
			public: public.to_vec(),
		}
	} else {
		info!("Generating our noise-protocol keypair, which had not yet been initialized.");
		let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
		let keypair = builder.generate_keypair().unwrap();

		let mut file = OpenOptions::new()
			.create(true)
			.write(true)
			.truncate(false)
			.open(&path)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;

		file.write_all(&keypair.private)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		file.write_all(&keypair.public)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		file.flush()
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		drop(file);
		keypair
	};
	Ok(keypair)
}

async fn await_key_approver_response(
	identity: NodeIdentity,
	mut approver: NewProtocolKeyApprover,
) -> Result<bool, HandshakeError> {
	// I really really do hate having to implement it like this. Very messy. Hopefully I figure out a better system for this soon.
	loop {
		match approver.recv_wait().await {
			Ok(resp) => {
				// Matches this peer's identity.
				if &resp.0 == &identity {
					return Ok(resp.1);
				}
			}
			_ => {
				return Err(HandshakeError::IdentityChanged(identity.to_base64()));
			}
		}
	}
}

async fn load_validate_noise_peer_key(
	noise_dir: PathBuf,
	peer_identity: NodeIdentity,
	received_noise_key: [u8; 32],
	report_mismatch: NewProtocolKeyReporter,
	mismatch_approver: NewProtocolKeyApprover,
) -> Result<(), HandshakeError> {
	async fn ld_keys(peer_filepath: PathBuf) -> Result<PeerKeyFile, HandshakeError> {
		let mut file = OpenOptions::new()
			.create(false)
			.read(true)
			.open(&peer_filepath)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;

		// An empty buffer is fine, AsyncReadExt::read_to_end() appends to a buffer and does not assume
		// you know the size of the file ahead of time.
		let mut buf = Vec::new();
		let read_amt = file
			.read_to_end(&mut buf)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		buf.truncate(read_amt);

		// from_read() doesn't work here because we're using async i/o instead.
		rmp_serde::from_slice(&buf).map_err(HandshakeError::ProtocolStoreDecodeError)
	}
	async fn create_new_peer_key_file(
		peer_filepath: PathBuf,
		peer_identity: NodeIdentity,
		received_noise_key: [u8; 32],
	) -> Result<(), HandshakeError> {
		trace!("Storing new noise keys for unfamiliar peer {:?}", peer_identity);

		let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&peer_filepath).await
            .map_err(| e| {
                let peer_print = peer_identity.to_base64();
                error!("Unable to store peer public key for {} due to IO error: {:?}. A session with this peer cannot be initialized.", peer_print, &e);
                HandshakeError::ProtocolStoreIoError(e)
            })?;
		let new_keys_struct = PeerKeyFile {
			keys: HashSet::from([received_noise_key]),
		};
		let writebuf = rmp_serde::to_vec_named(&new_keys_struct)
            .map_err(| e| {
                let peer_print = peer_identity.to_base64();
                error!("Unable to store peer public key for {} due to IO error: {:?}. A session with this peer cannot be initialized.", peer_print, &e);
                HandshakeError::ProtocolStoreEncodeError(e)
            })?;
		file.write_all(&writebuf)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		file.flush()
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		Ok(())
	}
	async fn write_changed_peer_key_file(
		peer_filepath: PathBuf,
		keys: PeerKeyFile,
	) -> Result<(), HandshakeError> {
		// Convert to msgpack
		let writebuf =
			rmp_serde::to_vec_named(&keys).map_err(HandshakeError::ProtocolStoreEncodeError)?;
		let temp_filepath = peer_filepath.with_extension(".pending");

		let mut file = OpenOptions::new()
			.create(true)
			.write(true)
			.truncate(true)
			.open(&temp_filepath)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;

		file.write_all(&writebuf)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		file.flush()
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;

		// Swap our pending file in
		let bkfile = peer_filepath.with_extension(".bk");
		tokio::fs::rename(&peer_filepath, &bkfile)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		tokio::fs::rename(&temp_filepath, &peer_filepath)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;
		tokio::fs::remove_file(bkfile)
			.await
			.map_err(HandshakeError::ProtocolStoreIoError)?;

		Ok(())
	}
	let peer_filepath = peer_dir(&noise_dir).join(PathBuf::from(peer_identity.to_base64()));
	if peer_filepath.exists() {
		match ld_keys(peer_filepath.clone()).await {
			Ok(mut key_struct) => {
				match key_struct.keys.contains(&received_noise_key) {
					true => Ok(()),
					false => {
						info!("Unrecognized peer key, prompting for approval.");
						// Check with the rest of the engine - are we okay with this new Noise protocol key?
						report_mismatch.send(peer_identity.clone()).unwrap();
						// I really really do hate having to implement it like this.
						match await_key_approver_response(peer_identity.clone(), mismatch_approver)
							.await?
						{
							true => {
								info!(
									"New peer key accepted for {}, adding to their list of recognized keys.",
									peer_identity.to_base64()
								);
								// Actually adding the key is such a tiny operation compared to all of the serialization and file i/o surrounding it!
								key_struct.keys.insert(received_noise_key);

								let write_change_resl =
									write_changed_peer_key_file(peer_filepath, key_struct).await;
								if let Err(e) = write_change_resl {
									// This one is special - we have a new key, the user approved it, but also we cannot
									// write it to the file for some reason. So, log that we cannot write it, but allow
									// the session to continue.
									error!("Unable to store new (approved) peer public key for {} due to IO error: {:?}. This key will be unrecognized the next time it is seen.", peer_identity.to_base64(), &e);
								}
								Ok(())
							}
							false => {
								Err(HandshakeError::IdentityChanged(peer_identity.to_base64()))
							}
						}
					}
				}
			}
			Err(e) => {
				error!("Unable to load existing key due to an error. Treating it as an unrecognized key, creating a new file if callback returns true. The error was: {:?}", &e);
				report_mismatch.send(peer_identity.clone()).unwrap();
				match await_key_approver_response(peer_identity.clone(), mismatch_approver).await? {
					true => {
						// Delete broken file
						tokio::fs::remove_file(&peer_filepath).await.unwrap(); //TODO: propagate error rather than using unwrap() here.
						create_new_peer_key_file(
							peer_filepath.clone(),
							peer_identity.clone(),
							received_noise_key.clone(),
						)
						.await
					}
					false => Err(HandshakeError::IdentityChanged(peer_identity.to_base64())),
				}
			}
		}
	} else {
		//New peer. It is expected and understood that the key for someone we've never met before will be new.
		create_new_peer_key_file(peer_filepath, peer_identity, received_noise_key).await
	}
}

/// Header to ask for a signature to prove identity.
/// More data here than just a byte buffer, so our public key can't be used to sign arbitrary things / impersonate us.
/// Transmitted as json
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Hash)]
pub(crate) struct KeyChallenge {
	/// Signed on a constant string (needs to always equal ['CHALLENGE_NAME']) to make it harder to forge this.
	pub static_challenge_name: String,
	/// Identity of the user sending this challenge, base-64.
	pub sender_ident: String,
	/// Identity of the user receiving this challenge, base-64.
	pub receiver_ident: String,
	/// Session ID (not truncated, full handshake hash), base-64.
	pub session_id: String,
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
// * Step 4: Responder sends a "nonce" buffer for the initiator to sign with its Gestalt identity key. It also sends its own Gestalt identity key.
// * Step 5: Initiator sends its public key and its signature, and also a nonce buffer for the responder to sign.
// * Step 6: Responder replies with signature on the step 5 buffer.
// * Step 7: The initiator receives the signature, verifies, completing the handshake (if both Gestalt identities verified)

/// Initiator-sided, step 1. Noise protocol  "-> e"  message.
pub fn initiate_handshake(
	local_noise_keys: snow::Keypair,
) -> Result<(snow::HandshakeState, HandshakeStepMessage), HandshakeError> {
	let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
	let mut noise = builder
		.local_private_key(&local_noise_keys.private)
		.build_initiator()?;

	// Generate "-> e" message
	let mut first_message_buf = [0u8; 1024];
	let wrote_len = noise.write_message(&[], &mut first_message_buf)?;
	trace!("Wrote a handshake initiator message which is {} bytes long", wrote_len);
	// Encode
	let msg = HandshakeStepMessage {
		handshake_step: 1,
		data: BASE_64.encode(&first_message_buf[0..wrote_len]),
	};

	Ok((noise, msg))
}

/// Receiver-sided, step 2. Noise protocol "<- e, ee, s, es" message.
/// In the XX pattern, step 2 and on are ciphertext. Only step 1 is cleartext.
/// However, this does not enjoy the security of a completed connection.
///
/// This is good enough that we can transmit our Gestalt node identity to the initiator,
/// in the payload field of our handshake message.
pub fn receive_initial(
	local_noise_keys: snow::Keypair,
	our_gestalt_identity: NodeIdentity,
	input: HandshakeStepMessage,
) -> Result<(snow::HandshakeState, HandshakeStepMessage), HandshakeError> {
	if input.handshake_step == 1 {
		let bytes_input = BASE_64.decode(input.data)?;
		let mut read_buf = [0u8; 1024];

		let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
		let mut state = builder
			.local_private_key(&local_noise_keys.private)
			.build_responder()?;

		// Read their message.
		let _read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;

		// Generate "e, ee, s, es" message
		let mut write_buf = [0u8; 1024];
		let wrote_len = state.write_message(our_gestalt_identity.get_bytes(), &mut write_buf)?;
		trace!("Wrote a handshake responder message which is {} bytes long", wrote_len);
		// Encode
		let msg = HandshakeStepMessage {
			handshake_step: 2,
			data: BASE_64.encode(&write_buf[0..wrote_len]),
		};

		Ok((state, msg))
	} else {
		Err(HandshakeError::UnexpectedStep(1, input.handshake_step))
	}
}

/// Initiator-sided, step 3. Noise protocol "s, se ->" message.
pub async fn initiator_reply(
	mut state: snow::HandshakeState,
	input: HandshakeStepMessage,
	noise_dir: PathBuf,
	our_gestalt_identity: NodeIdentity,
	report_mismatch: NewProtocolKeyReporter,
	mismatch_approver: NewProtocolKeyApprover,
) -> Result<
	(snow::StatelessTransportState, HandshakeStepMessage, NodeIdentity, Vec<u8>),
	HandshakeError,
> {
	if input.handshake_step == 2 {
		let bytes_input = BASE_64.decode(input.data)?;
		let mut read_buf = [0u8; 1024];

		// Read their message.
		let read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
		// Check for the server's public key here.
		let peer_gestalt_identity = NodeIdentity::from_bytes(&read_buf[0..read_buf_len])?;

		// Send our "s, se ->"
		let mut send_buf = [0u8; 1024];
		let wrote_len = state.write_message(our_gestalt_identity.get_bytes(), &mut send_buf)?;
		trace!("Wrote a response which is {} bytes long", wrote_len);

		// Format it
		let output = HandshakeStepMessage {
			handshake_step: 3,
			data: BASE_64.encode(&send_buf[0..wrote_len]),
		};

		// Get Noise key.
		let remote_static = state
			.get_remote_static()
			.ok_or(HandshakeError::MissingRemoteStatic(2))?;
		// Validate size.
		if remote_static.len() != 32 {
			return Err(HandshakeError::ProtocolKeyWrongSize(remote_static.len()));
		}
		let mut protocol_key: [u8; 32] = [0u8; 32];
		protocol_key.copy_from_slice(remote_static);

		// Make sure we notice if the key changed.
		load_validate_noise_peer_key(
			noise_dir,
			peer_gestalt_identity.clone(),
			protocol_key,
			report_mismatch,
			mismatch_approver,
		)
		.await?;

		let handshake_hash = state.get_handshake_hash().to_vec();

		// Turn handshake state into a transport
		let transport = state.into_stateless_transport_mode()?;

		Ok((transport, output, peer_gestalt_identity, handshake_hash))
	} else {
		Err(HandshakeError::UnexpectedStep(2, input.handshake_step))
	}
}

fn make_signing_nonce() -> [u8; 32] {
	let mut rng = rand_core::OsRng::default();
	rng.r#gen()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HandshakeMessage4 {
	/// Contains json structure of a [`KeyChallenge`] for the peer to sign.
	pub please_sign: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HandshakeMessage5 {
	/// Base-64 encoded initiator signature on responder's HandshakeMessage4 "please_sign"
	pub initiator_signature: String,
	/// Contains json structure of a [`KeyChallenge`] for the peer to sign.
	pub please_sign: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HandshakeMessage6 {
	/// Base-64 encoded responder signature on initiator's HandshakeMessage5 "please_sign"
	pub responder_signature: String,
}

/// Receiver-sided, receive step 3 message from initiator and finish Noise handshake. Now we do funky identity stuff, sending a buffer and asking the other side to sign it.
pub async fn receive_last_noise(
	mut state: snow::HandshakeState,
	input: HandshakeStepMessage,
	noise_dir: PathBuf,
	our_gestalt_identity: NodeIdentity,
	report_mismatch: NewProtocolKeyReporter,
	mismatch_approver: NewProtocolKeyApprover,
) -> Result<
	(
		snow::StatelessTransportState,
		HandshakeStepMessage,
		String,
		MessageCounter,
		NodeIdentity,
		Vec<u8>,
	),
	HandshakeError,
> {
	if input.handshake_step == 3 {
		let bytes_input = BASE_64.decode(input.data)?;
		let mut read_buf = [0u8; 1024];

		// Read their message.
		let read_buf_len = state.read_message(&bytes_input, &mut read_buf)?;
		// Check for the client's public key here.
		let peer_gestalt_identity = NodeIdentity::from_bytes(&read_buf[0..read_buf_len])?;

		// Get Noise key.
		let remote_static = state
			.get_remote_static()
			.ok_or(HandshakeError::MissingRemoteStatic(3))?;
		// Validate size.
		if remote_static.len() != 32 {
			return Err(HandshakeError::ProtocolKeyWrongSize(remote_static.len()));
		}
		let mut protocol_key: [u8; 32] = [0u8; 32];
		protocol_key.copy_from_slice(remote_static);
		// Make sure we notice if the key changed.
		load_validate_noise_peer_key(
			noise_dir,
			peer_gestalt_identity.clone(),
			protocol_key,
			report_mismatch,
			mismatch_approver,
		)
		.await?;

		let handshake_hash = state.get_handshake_hash().to_vec();

		// Turn handshake state into a transport
		let transport = state.into_stateless_transport_mode()?;

		// Build a HandshakeMessage asking for a signature.
		let nonce = make_signing_nonce();
		let base64_nonce = BASE_64.encode(&nonce);
		let challenge = KeyChallenge {
			static_challenge_name: CHALLENGE_NAME.to_string(),
			sender_ident: our_gestalt_identity.to_base64(),
			receiver_ident: peer_gestalt_identity.to_base64(),
			session_id: BASE_64.encode(&handshake_hash),
			challenge: base64_nonce,
		};
		let json_challenge = serde_json::to_string(&challenge)?;
		let message = HandshakeMessage4 {
			please_sign: json_challenge.clone(),
		};
		let json_message = serde_json::to_string(&message)?;

		let mut buf = vec![0u8; 65535];
		let encoded_length = transport.write_message(0, json_message.as_bytes(), &mut buf)?;
		let buf_bytes = &buf[0..encoded_length];
		let encoded_message = BASE_64.encode(&buf_bytes);

		let step = HandshakeStepMessage {
			handshake_step: 4,
			data: encoded_message,
		};

		Ok((transport, step, json_challenge, 0, peer_gestalt_identity, handshake_hash))
	} else {
		Err(HandshakeError::UnexpectedStep(3, input.handshake_step))
	}
}
/// Receive step 4 message, produce step 5 message.
pub fn initiator_sign_buf(
	state: snow::StatelessTransportState,
	input: HandshakeStepMessage,
	their_key: &NodeIdentity,
	our_keys: &IdentityKeyPair,
	handshake_hash: &Vec<u8>,
) -> Result<
	(snow::StatelessTransportState, HandshakeStepMessage, String, MessageCounter),
	HandshakeError,
> {
	if input.handshake_step == 4 {
		let bytes_input = BASE_64.decode(input.data)?;
		let mut read_buf = [0u8; 65535];

		// Read their message.
		let read_buf_len = state.read_message(0, &bytes_input, &mut read_buf)?;

		// Get the inner message.
		let msg: HandshakeMessage4 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
		let challenge_string = msg.please_sign.clone();
		let our_signature = our_keys
			.sign(challenge_string.as_bytes())
			.map_err(HandshakeError::CannotSign)?;
		let our_signature_bytes = our_signature.to_bytes();
		let our_signature_b64 = BASE_64.encode(our_signature_bytes);

		// Validate header
		let challenge: KeyChallenge = serde_json::from_str(&challenge_string)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if challenge.static_challenge_name != CHALLENGE_NAME {
			return Err(HandshakeError::BadChallengeHeader);
		}
		// Double-check to make sure their key is in the response.
		let decoded_sender_key = NodeIdentity::from_base64(&challenge.sender_ident)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if &decoded_sender_key != their_key {
			return Err(HandshakeError::BadChallengeHeader);
		}
		// Does it sign over our key?
		let public_key_claim = NodeIdentity::from_base64(&challenge.receiver_ident)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if &public_key_claim != &our_keys.public {
			return Err(HandshakeError::BadChallengeHeader);
		}
		// Does it sign over the handshake hash?
		let decoded_session_id = BASE_64
			.decode(&challenge.session_id)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if &decoded_session_id != handshake_hash {
			return Err(HandshakeError::BadChallengeHeader);
		}

		// Build a HandshakeMessage asking for a signature.
		let nonce = make_signing_nonce();
		let base64_nonce = BASE_64.encode(&nonce);
		let challenge = KeyChallenge {
			static_challenge_name: CHALLENGE_NAME.to_string(),
			sender_ident: our_keys.public.to_base64(),
			session_id: BASE_64.encode(handshake_hash),
			challenge: base64_nonce,
			receiver_ident: their_key.to_base64(),
		};
		let json_challenge = serde_json::to_string(&challenge)?;
		let message = HandshakeMessage5 {
			please_sign: json_challenge.clone(),
			initiator_signature: our_signature_b64,
		};
		let json_message = serde_json::to_string(&message)?;

		let mut buf = vec![0u8; 65535];
		let encoded_length = state.write_message(0, json_message.as_bytes(), &mut buf)?;
		let buf_bytes = &buf[0..encoded_length];
		let encoded_message = BASE_64.encode(&buf_bytes);

		let step = HandshakeStepMessage {
			handshake_step: 5,
			data: encoded_message,
		};

		Ok((state, step, json_challenge, 0))
	} else {
		Err(HandshakeError::UnexpectedStep(4, input.handshake_step))
	}
}
/// Receive step 5 message, produce step 6 message.
pub fn responder_sign(
	state: snow::StatelessTransportState,
	input: HandshakeStepMessage,
	our_keys: &IdentityKeyPair,
	peer_identity: &NodeIdentity,
	our_challenge: String,
	handshake_hash: &Vec<u8>,
) -> Result<(snow::StatelessTransportState, HandshakeStepMessage, MessageCounter), HandshakeError> {
	if input.handshake_step == 5 {
		let bytes_input = BASE_64.decode(input.data)?;
		let mut read_buf = [0u8; 65535];

		// Read their message.
		let read_buf_len = state.read_message(0, &bytes_input, &mut read_buf)?;

		// Get the inner message.
		let msg: HandshakeMessage5 = serde_json::from_slice(&read_buf[0..read_buf_len])?;

		// Validate their signature
		let their_sig = BASE_64.decode(msg.initiator_signature.as_bytes())?;
		let their_sig =
			buf_to_64(&their_sig).map_err(|length| HandshakeError::SignatureLengthWrong(length))?;
		peer_identity
			.verify_signature(our_challenge.as_bytes(), &their_sig)
			.map_err(HandshakeError::BadSignature)?;

		let challenge_string = msg.please_sign.clone();
		// Make our signature.
		let our_signature = our_keys
			.sign(challenge_string.as_bytes())
			.map_err(HandshakeError::CannotSign)?;
		let our_signature_bytes = our_signature.to_bytes();
		let our_signature_b64 = BASE_64.encode(our_signature_bytes);

		// Validate their header
		let challenge: KeyChallenge = serde_json::from_str(&challenge_string)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if challenge.static_challenge_name != CHALLENGE_NAME {
			return Err(HandshakeError::BadChallengeHeader);
		}
		// Double-check to make sure their key is in the response.
		let decoded_sender_key = NodeIdentity::from_base64(&challenge.sender_ident)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if &decoded_sender_key != peer_identity {
			return Err(HandshakeError::BadChallengeHeader);
		}
		// Does it sign over our key?
		let public_key_claim = NodeIdentity::from_base64(&challenge.receiver_ident)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if &public_key_claim != &our_keys.public {
			return Err(HandshakeError::BadChallengeHeader);
		}
		// Does it sign over the handshake hash?
		let decoded_session_id = BASE_64
			.decode(&challenge.session_id)
			.map_err(|_| HandshakeError::BadChallengeHeader)?;
		if &decoded_session_id != handshake_hash {
			return Err(HandshakeError::BadChallengeHeader);
		}

		// Build a HandshakeMessage sending our signature.
		let message = HandshakeMessage6 {
			responder_signature: our_signature_b64,
		};
		let json_message = serde_json::to_string(&message)?;

		let mut buf = vec![0u8; 65535];
		let encoded_length = state.write_message(1, json_message.as_bytes(), &mut buf)?;
		let buf_bytes = &buf[0..encoded_length];
		let encoded_message = BASE_64.encode(&buf_bytes);

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
pub fn initiator_final(
	state: snow::StatelessTransportState,
	input: HandshakeStepMessage,
	peer_identity: &NodeIdentity,
	our_challenge: String,
) -> Result<(snow::StatelessTransportState, MessageCounter), HandshakeError> {
	if input.handshake_step == 6 {
		let bytes_input = BASE_64.decode(input.data)?;
		let mut read_buf = [0u8; 65535];

		// Read their message.
		let read_buf_len = state.read_message(1, &bytes_input, &mut read_buf)?;

		// Get the inner message.
		let msg: HandshakeMessage6 = serde_json::from_slice(&read_buf[0..read_buf_len])?;
		// Validate their signature
		let their_sig = BASE_64.decode(msg.responder_signature)?;
		let their_sig =
			buf_to_64(&their_sig).map_err(|length| HandshakeError::SignatureLengthWrong(length))?;
		peer_identity
			.verify_signature(our_challenge.as_bytes(), &their_sig)
			.map_err(HandshakeError::BadSignature)?;

		Ok((state, 1))
	} else {
		Err(HandshakeError::UnexpectedStep(4, input.handshake_step))
	}
}

/// Cleaner state machine wrapper for otherwise-imperative handshake process.
pub enum HandshakeIntitiatorState {
	Init,
	SentFirstAwaitSecond(snow::HandshakeState),
	SentThirdAwaitFourth(snow::StatelessTransportState, NodeIdentity, Vec<u8>),
	SentFifthAwaitSixth(
		snow::StatelessTransportState,
		String,
		MessageCounter,
		NodeIdentity,
		Vec<u8>,
	),
	Done(snow::StatelessTransportState, MessageCounter, NodeIdentity, Vec<u8>),
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
			HandshakeIntitiatorState::SentFirstAwaitSecond(_) => {
				HandshakeIntitiatorStep::SentFirstAwaitSecond
			}
			HandshakeIntitiatorState::SentThirdAwaitFourth(_, _, _) => {
				HandshakeIntitiatorStep::SentThirdAwaitFourth
			}
			HandshakeIntitiatorState::SentFifthAwaitSixth(_, _, _, _, _) => {
				HandshakeIntitiatorStep::SentFifthAwaitSixth
			}
			HandshakeIntitiatorState::Done(_, _, _, _) => HandshakeIntitiatorStep::Done,
		}
	}
}
impl Default for HandshakeIntitiatorState {
	fn default() -> Self {
		Self::new()
	}
}

// Clone is not implemented on keypair - presumably for defense-in-depth reasons.
// However, if you're, say, trying to make an async method whose future is Send
// and therefore you don't want it to have to keep references alive,
// that becomes a bit of a problem.
fn clone_keypair(keys: &snow::Keypair) -> snow::Keypair {
	snow::Keypair {
		public: keys.public.clone(),
		private: keys.private.clone(),
	}
}
pub struct HandshakeInitiator {
	/// Technically this will never be None, I'm just convincing the borrow checker to work with me here.
	last_state: Option<HandshakeIntitiatorState>,
	pub noise_dir: PathBuf,
	pub local_noise_keys: snow::Keypair,
	pub local_gestalt_keys: IdentityKeyPair,
	report_mismatch: Option<NewProtocolKeyReporter>,
	mismatch_approver: Option<NewProtocolKeyApprover>,
}
impl HandshakeInitiator {
	pub fn new(
		noise_dir: PathBuf,
		local_noise_keys: snow::Keypair,
		local_gestalt_keys: IdentityKeyPair,
		report_mismatch: NewProtocolKeyReporter,
		mismatch_approver: NewProtocolKeyApprover,
	) -> Self {
		HandshakeInitiator {
			last_state: Some(HandshakeIntitiatorState::Init),
			local_noise_keys,
			local_gestalt_keys,
			noise_dir,
			report_mismatch: Some(report_mismatch),
			mismatch_approver: Some(mismatch_approver),
		}
	}
	pub fn send_first(&mut self) -> Result<HandshakeStepMessage, HandshakeError> {
		if self.last_state.as_ref().unwrap().get_step() == HandshakeIntitiatorStep::Init {
			let (state, message) = initiate_handshake(clone_keypair(&self.local_noise_keys))?;
			self.last_state = Some(HandshakeIntitiatorState::SentFirstAwaitSecond(state));
			Ok(message)
		} else {
			Err(HandshakeError::FirstAfterInit)
		}
	}
	pub async fn advance(
		&mut self,
		incoming: HandshakeStepMessage,
	) -> Result<HandshakeNext, HandshakeError> {
		match self.last_state.take().unwrap() {
			HandshakeIntitiatorState::Init => Err(HandshakeError::WrongOrder),
			HandshakeIntitiatorState::SentFirstAwaitSecond(state) => {
				let report_mismatch = self
					.report_mismatch
					.take()
					.ok_or(HandshakeError::NoMismatchChannels)?;
				let mismatch_approver = self
					.mismatch_approver
					.take()
					.ok_or(HandshakeError::NoMismatchChannels)?;
				let (new_state, message, peer_identity, sid) = initiator_reply(
					state,
					incoming,
					self.noise_dir.clone(),
					self.local_gestalt_keys.public.clone(),
					report_mismatch,
					mismatch_approver,
				)
				.await?;
				self.last_state = Some(HandshakeIntitiatorState::SentThirdAwaitFourth(
					new_state,
					peer_identity,
					sid,
				));
				Ok(HandshakeNext::SendMessage(message))
			}
			HandshakeIntitiatorState::SentThirdAwaitFourth(state, peer_id, sid) => {
				let (transport, message, nonce, seq) =
					initiator_sign_buf(state, incoming, &peer_id, &self.local_gestalt_keys, &sid)?;
				self.last_state = Some(HandshakeIntitiatorState::SentFifthAwaitSixth(
					transport, nonce, seq, peer_id, sid,
				));
				Ok(HandshakeNext::SendMessage(message))
			}
			HandshakeIntitiatorState::SentFifthAwaitSixth(state, nonce, _seq, peer_id, sid) => {
				let (transport, new_seq) = initiator_final(state, incoming, &peer_id, nonce)?;
				self.last_state =
					Some(HandshakeIntitiatorState::Done(transport, new_seq, peer_id, sid));
				Ok(HandshakeNext::Done)
			}
			HandshakeIntitiatorState::Done(_, _, _, _) => Err(HandshakeError::AdvanceAfterDone),
		}
	}
	pub fn is_done(&self) -> bool {
		matches!(self.last_state.as_ref().unwrap(), HandshakeIntitiatorState::Done(_, _, _, _))
	}
	pub fn complete(
		mut self,
	) -> Result<
		(snow::StatelessTransportState, MessageCounter, NodeIdentity, SessionId),
		HandshakeError,
	> {
		if let HandshakeIntitiatorState::Done(transport, counter, peer_id, session_id) =
			self.last_state.take().unwrap()
		{
			Ok((transport, counter, peer_id, truncate_to_session_id(&session_id)))
		} else {
			Err(HandshakeError::CompleteBeforeDone)
		}
	}
}
/// Cleaner state machine wrapper for otherwise-imperative handshake process.
pub enum HandshakeReceiverState {
	Init,
	SentSecondAwaitThird(snow::HandshakeState),
	SentFourthAwaitFifth(snow::StatelessTransportState, String, MessageCounter, Vec<u8>),
	Done(snow::StatelessTransportState, MessageCounter, Vec<u8>),
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
			HandshakeReceiverState::SentSecondAwaitThird(_) => {
				HandshakeReceiverStep::SentSecondAwaitThird
			}
			HandshakeReceiverState::SentFourthAwaitFifth(_, _, _, _) => {
				HandshakeReceiverStep::SentFourthAwaitFifth
			}
			HandshakeReceiverState::Done(_, _, _) => HandshakeReceiverStep::Done,
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
	pub noise_dir: PathBuf,
	pub local_noise_keys: snow::Keypair,
	pub local_gestalt_keys: IdentityKeyPair,
	pub peer_public_key: Option<NodeIdentity>,
	report_mismatch: Option<NewProtocolKeyReporter>,
	mismatch_approver: Option<NewProtocolKeyApprover>,
}
impl HandshakeReceiver {
	pub fn new(
		noise_dir: PathBuf,
		local_noise_keys: snow::Keypair,
		local_gestalt_keys: IdentityKeyPair,
		report_mismatch: NewProtocolKeyReporter,
		mismatch_approver: NewProtocolKeyApprover,
	) -> Self {
		HandshakeReceiver {
			last_state: Some(HandshakeReceiverState::Init),
			noise_dir,
			local_noise_keys,
			local_gestalt_keys,
			peer_public_key: None,
			report_mismatch: Some(report_mismatch),
			mismatch_approver: Some(mismatch_approver),
		}
	}
	pub async fn advance(
		&mut self,
		incoming: HandshakeStepMessage,
	) -> Result<HandshakeNext, HandshakeError> {
		match self.last_state.take().unwrap() {
			HandshakeReceiverState::Init => {
				let (state, message) = receive_initial(
					clone_keypair(&self.local_noise_keys),
					self.local_gestalt_keys.public.clone(),
					incoming,
				)?;
				self.last_state = Some(HandshakeReceiverState::SentSecondAwaitThird(state));
				Ok(HandshakeNext::SendMessage(message))
			}
			HandshakeReceiverState::SentSecondAwaitThird(state) => {
				let report_mismatch = self
					.report_mismatch
					.take()
					.ok_or(HandshakeError::NoMismatchChannels)?;
				let mismatch_approver = self
					.mismatch_approver
					.take()
					.ok_or(HandshakeError::NoMismatchChannels)?;
				let (new_state, message, nonce, seq, peer_identity, sid) = receive_last_noise(
					state,
					incoming,
					self.noise_dir.clone(),
					self.local_gestalt_keys.public.clone(),
					report_mismatch,
					mismatch_approver,
				)
				.await?;
				self.last_state =
					Some(HandshakeReceiverState::SentFourthAwaitFifth(new_state, nonce, seq, sid));
				self.peer_public_key = Some(peer_identity);
				Ok(HandshakeNext::SendMessage(message))
			}
			HandshakeReceiverState::SentFourthAwaitFifth(state, nonce, _seq, sid) => {
				let (new_state, message, seq) = responder_sign(
					state,
					incoming,
					&self.local_gestalt_keys,
					self.peer_public_key
						.as_ref()
						.ok_or(HandshakeError::NoIdentity)?,
					nonce,
					&sid,
				)?;
				self.last_state = Some(HandshakeReceiverState::Done(new_state, seq, sid));
				Ok(HandshakeNext::SendMessage(message))
			}
			HandshakeReceiverState::Done(_, _, _) => Err(HandshakeError::AdvanceAfterDone),
		}
	}
	pub fn is_done(&self) -> bool {
		matches!(self.last_state.as_ref().unwrap(), HandshakeReceiverState::Done(_, _, _))
	}
	pub fn complete(
		mut self,
	) -> Result<
		(snow::StatelessTransportState, MessageCounter, NodeIdentity, SessionId),
		HandshakeError,
	> {
		if let HandshakeReceiverState::Done(transport, counter, session_id) =
			self.last_state.take().unwrap()
		{
			Ok((
				transport,
				counter,
				self.peer_public_key
					.as_ref()
					.ok_or(HandshakeError::NoIdentity)?
					.clone(),
				truncate_to_session_id(&session_id),
			))
		} else {
			Err(HandshakeError::CompleteBeforeDone)
		}
	}
	/// A return value of None implies we don't know it yet.
	pub fn get_peer_identity(&self) -> Option<&NodeIdentity> {
		self.peer_public_key.as_ref()
	}
}
pub enum HandshakeNext {
	SendMessage(HandshakeStepMessage),
	Done,
}

#[cfg(test)]
pub async fn approver_no_mismatch(
	mut receiver: BroadcastReceiver<NodeIdentity>,
	_sender: BroadcastSender<(NodeIdentity, bool)>,
) {
	match receiver.recv_wait().await {
		Ok(_) => {
			panic!("New connection with an unrecognized key - this test should never evaluate the mismatch code path.")
		}
		Err(_) => {
			info!("Dropped mismatch sender before using receiver - as it should be in this test.")
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::{message::{BroadcastChannel, SenderSubscribe}, ReceiverSubscribe};

	#[tokio::test]
	async fn handshake_test() {
		let noise_dir = tempfile::tempdir().unwrap();

		let bob_gestalt_keys = IdentityKeyPair::generate_for_tests();
		let alice_gestalt_keys = IdentityKeyPair::generate_for_tests();

		let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
		let bob_noise_keys = builder.generate_keypair().unwrap();
		let alice_noise_keys = builder.generate_keypair().unwrap();

		// Mismatch reporter stuff.
		let mismatch_report_channel = BroadcastChannel::new(1024);
		let mismatch_approve_channel = BroadcastChannel::new(1024);
		let mismatch_report_receiver = mismatch_report_channel.receiver_subscribe();
		let mismatch_approve_sender = mismatch_approve_channel.sender_subscribe();
		tokio::spawn(approver_no_mismatch(mismatch_report_receiver, mismatch_approve_sender));

		let (bob_state, message_1) = initiate_handshake(clone_keypair(&bob_noise_keys)).unwrap();
		let (alice_state, message_2) = receive_initial(
			clone_keypair(&alice_noise_keys),
			alice_gestalt_keys.public.clone(),
			message_1,
		)
		.unwrap();
		let (bob_transport, message_3, bob_copy_alice_identity, bob_session_id) = initiator_reply(
			bob_state,
			message_2,
			PathBuf::from(noise_dir.path()),
			bob_gestalt_keys.public.clone(),
			mismatch_report_channel.sender_subscribe(),
			mismatch_approve_channel.receiver_subscribe(),
		)
		.await
		.unwrap();
		let (
			alice_transport,
			message_4,
			alice_nonce,
			_alice_seq,
			alice_copy_bob_identity,
			alice_session_id,
		) = receive_last_noise(
			alice_state,
			message_3,
			PathBuf::from(noise_dir.path()),
			alice_gestalt_keys.public.clone(),
			mismatch_report_channel.sender_subscribe(),
			mismatch_approve_channel.receiver_subscribe(),
		)
		.await
		.unwrap();
		let (bob_transport, message_5, bob_nonce, _bob_seq) = initiator_sign_buf(
			bob_transport,
			message_4,
			&bob_copy_alice_identity,
			&bob_gestalt_keys,
			&bob_session_id,
		)
		.unwrap();
		let (alice_transport, message_6, _alice_seq) = responder_sign(
			alice_transport,
			message_5,
			&alice_gestalt_keys,
			&alice_copy_bob_identity,
			alice_nonce,
			&alice_session_id,
		)
		.unwrap();
		let (bob_transport, _bob_seq) =
			initiator_final(bob_transport, message_6, &bob_copy_alice_identity, bob_nonce).unwrap();

		assert_eq!(bob_session_id, alice_session_id);

		// Try sending a message!
		let mut write_buf = [0u8; 1024];
		let write_len = bob_transport
			.write_message(1337, "Hello!".as_bytes(), &mut write_buf)
			.unwrap();

		let mut read_buf = [0u8; 1024];
		let read_len = alice_transport
			.read_message(1337, &write_buf[0..write_len], &mut read_buf)
			.unwrap();
		let read_result = String::from_utf8_lossy(&read_buf[0..read_len]).to_string();
		assert!(read_result.as_str() == "Hello!");

		// Now let's try messing up on purpose.
		let mut write_buf = [0u8; 1024];
		let write_len = bob_transport
			.write_message(123, "This should fail!".as_bytes(), &mut write_buf)
			.unwrap();

		// This should fail because counter doesn't match
		let mut read_buf = [0u8; 1024];
		let _err = alice_transport
			.read_message(456, &write_buf[0..write_len], &mut read_buf)
			.unwrap_err();
	}

	#[tokio::test]
	async fn handshake_state_machine_test() {
		let noise_dir = tempfile::tempdir().unwrap();

		let bob_gestalt_keys = IdentityKeyPair::generate_for_tests();
		let alice_gestalt_keys = IdentityKeyPair::generate_for_tests();

		let builder: snow::Builder<'_> = snow::Builder::new(NOISE_PARAMS.clone());
		let bob_noise_keys = builder.generate_keypair().unwrap();
		let alice_noise_keys = builder.generate_keypair().unwrap();

		// Mismatch reporter stuff.
		let mismatch_report_channel = BroadcastChannel::new(1024);
		let mismatch_approve_channel = BroadcastChannel::new(1024);
		let mismatch_report_receiver = mismatch_report_channel.receiver_subscribe();
		let mismatch_approve_sender = mismatch_approve_channel.sender_subscribe();
		tokio::spawn(approver_no_mismatch(mismatch_report_receiver, mismatch_approve_sender));

		let mut initiator = HandshakeInitiator::new(
			PathBuf::from(noise_dir.path()),
			bob_noise_keys,
			bob_gestalt_keys,
			mismatch_report_channel.sender_subscribe(),
			mismatch_approve_channel.receiver_subscribe(),
		);
		let mut receiver = HandshakeReceiver::new(
			PathBuf::from(noise_dir.path()),
			alice_noise_keys,
			alice_gestalt_keys,
			mismatch_report_channel.sender_subscribe(),
			mismatch_approve_channel.receiver_subscribe(),
		);

		// Done with init, let's actually do some hand-shaking

		let first_message = initiator.send_first().unwrap();
		let mut bobs_turn = false;

		let mut steps_counter: usize = 1; // Step 1 is first message, we just sent that.

		let mut last_message = Some(HandshakeNext::SendMessage(first_message));
		while let HandshakeNext::SendMessage(msg) = last_message.take().unwrap() {
			if bobs_turn {
				last_message = Some(initiator.advance(msg).await.unwrap());
			} else {
				last_message = Some(receiver.advance(msg).await.unwrap())
			}
			steps_counter += 1;
			if steps_counter > 7 {
				panic!("Too many steps!");
			}
			bobs_turn = !bobs_turn;
		}
		// Breaking this loop requires encountering a HandshakeNext::Done

		assert!(initiator.is_done());
		let (_bob_transport, _bob_seq, bob_copy_alice_ident, bob_session_id) =
			initiator.complete().unwrap();

		assert!(receiver.is_done());
		let (_alice_transport, _alice_seq, alice_copy_bob_ident, alice_session_id) =
			receiver.complete().unwrap();

		assert_eq!(bob_session_id, alice_session_id);

		assert_eq!(bob_copy_alice_ident, alice_gestalt_keys.public);
		assert_eq!(alice_copy_bob_ident, bob_gestalt_keys.public);
	}
}
