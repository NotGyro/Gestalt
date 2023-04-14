use base64::engine::general_purpose::URL_SAFE as BASE_64;
use base64::Engine;
use signature::{Signer, Verifier};

use serde::{Deserialize, Serialize};

use pkcs8::{
	der::Document, AlgorithmIdentifier, DecodePrivateKey, DecodePublicKey, EncodePrivateKey,
	EncodePublicKey,
};
use std::{
	fs::{self, OpenOptions},
	io::Write,
	path::PathBuf,
};

/// The length of a ed25519 `Signature`, in bytes.
pub const SIGNATURE_LENGTH: usize = ed25519::Signature::BYTE_SIZE;

/// The length of a ed25519 `SecretKey`, in bytes.
pub const PRIVATE_KEY_LENGTH: usize = 32;

/// The length of an ed25519 `PublicKey`, in bytes.
pub const PUBLIC_KEY_LENGTH: usize = 32;

#[derive(thiserror::Error, Debug)]
pub enum DecodeIdentityError {
	#[error("error decoding a node identity from a Base-64 string: {0:?}")]
	Base64Error(#[from] base64::DecodeError),
	#[error("node identity length was incorrect. Expected 32, got {0}")]
	WrongLength(usize),
}

pub type SignatureError = ed25519::signature::Error;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct NodeIdentity([u8; PUBLIC_KEY_LENGTH]);

impl NodeIdentity {
	pub fn get_bytes(&self) -> &[u8] {
		&self.0
	}
	pub fn to_base64(&self) -> String {
		BASE_64.encode(&self.0)
	}
	pub fn from_base64(b64: &str) -> Result<Self, DecodeIdentityError> {
		let buf = BASE_64.decode(b64)?;
		Self::from_bytes(&buf)
	}
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, DecodeIdentityError> {
		if bytes.len() == PUBLIC_KEY_LENGTH {
			let mut smaller_buf = [0u8; PUBLIC_KEY_LENGTH];
			smaller_buf.copy_from_slice(&bytes[0..PUBLIC_KEY_LENGTH]);
			Ok(NodeIdentity(smaller_buf))
		} else {
			Err(DecodeIdentityError::WrongLength(bytes.len()))
		}
	}
	pub fn verify_signature(&self, message: &[u8], signature: &[u8]) -> Result<(), SignatureError> {
		let converted_key: ed25519_dalek::PublicKey = self.into();
		let converted_signature = ed25519_dalek::Signature::from_bytes(signature)?;
		converted_key.verify(message, &converted_signature)
	}
}

impl std::fmt::Debug for NodeIdentity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "NodeIdentity({})", self.to_base64())
	}
}

impl From<&NodeIdentity> for ed25519_dalek::PublicKey {
	fn from(value: &NodeIdentity) -> Self {
		ed25519_dalek::PublicKey::from_bytes(&value.0).unwrap()
	}
}

impl From<&ed25519_dalek::PublicKey> for NodeIdentity {
	fn from(value: &ed25519_dalek::PublicKey) -> Self {
		NodeIdentity(value.to_bytes())
	}
}

pub type Signature = ed25519::Signature;
/*
impl From<&Signature> for ed25519::Signature {
	fn from(value: &Signature) -> Self {
		ed25519::Signature::from_bytes(&value.0).unwrap()
	}
}
impl From<&ed25519::Signature> for Signature {
	fn from(value: &ed25519::Signature) -> Self {
		Signature(value.to_bytes().clone())
	}
}*/

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PrivateKey([u8; PRIVATE_KEY_LENGTH]);

impl From<&PrivateKey> for ed25519_dalek::SecretKey {
	fn from(value: &PrivateKey) -> Self {
		ed25519_dalek::SecretKey::from_bytes(&value.0).unwrap()
	}
}
impl From<&ed25519_dalek::SecretKey> for PrivateKey {
	fn from(value: &ed25519_dalek::SecretKey) -> Self {
		PrivateKey(value.to_bytes())
	}
}

impl PrivateKey {
	pub fn get_bytes(&self) -> &[u8] {
		&self.0
	}
}

/// The keys for this node (i.e. the node that this Gestalt executable is being run to host)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IdentityKeyPair {
	pub public: NodeIdentity,
	pub private: PrivateKey,
}

impl IdentityKeyPair {
	#[cfg(test)]
	pub fn generate_for_tests() -> Self {
		let mut rng = rand_core::OsRng::default();
		let keys_dalek = ed25519_dalek::Keypair::generate(&mut rng);
		(&keys_dalek).into()
	}
}

impl From<&IdentityKeyPair> for ed25519_dalek::Keypair {
	fn from(value: &IdentityKeyPair) -> Self {
		ed25519_dalek::Keypair {
			secret: (&value.private).into(),
			public: (&value.public).into(),
		}
	}
}
impl From<&ed25519_dalek::Keypair> for IdentityKeyPair {
	fn from(value: &ed25519_dalek::Keypair) -> Self {
		IdentityKeyPair {
			public: (&value.public).into(),
			private: (&value.secret).into(),
		}
	}
}

impl IdentityKeyPair {
	pub fn sign(&self, msg: &[u8]) -> Result<Signature, SignatureError> {
		let converted_keypair: ed25519_dalek::Keypair = self.into();
		Ok(converted_keypair.sign(msg))
	}
}

const KEYS_DIRECTORY: &str = "keys";
const PUB_KEY_FILENAME: &str = "identity_key_public.pem";
const PRIV_KEY_FILENAME: &str = "identity_key_private.pem";

#[derive(thiserror::Error, Debug, Clone)]
pub enum KeyPairLoadError {
	#[error("Wrong length for private key: expected 32 bytes and got {0}")]
	WrongLengthPrivate(usize),
	#[error("Wrong length for public key: expected 32 bytes and got {0}")]
	WrongLengthPublic(usize),
	#[error("Tried to generate a public key but we already have a public key!")]
	PubKeyExists,
	#[error("Tried to generate a private key but we already have a public key!")]
	PrivKeyExists,
}

/// Returns false if key files exist already and true if they don't exist yet and need to be made.
pub fn do_keys_need_generating() -> bool {
	let keys_directory = PathBuf::from(KEYS_DIRECTORY);
	if !keys_directory.exists() {
		fs::create_dir_all(keys_directory).unwrap();
		return true;
	}
	let pub_key_path = keys_directory.join(PathBuf::from(PUB_KEY_FILENAME));
	let priv_key_path = keys_directory.join(PathBuf::from(PRIV_KEY_FILENAME));

	if !pub_key_path.exists() {
		return true;
	} else if pub_key_path.is_dir() {
		panic!("Public key file {} cannot be a directory!", pub_key_path.display());
	}

	if !priv_key_path.exists() {
		return true;
	} else if priv_key_path.is_dir() {
		panic!("Public key file {} cannot be a directory!", priv_key_path.display());
	}

	// All sanity checks passed, keys exist so we don't need to generate them.
	false
}

pub fn generate_local_keys(
	passphrase: Option<String>,
) -> Result<IdentityKeyPair, Box<dyn std::error::Error>> {
	let mut rng = rand_core::OsRng::default();
	// Set up paths
	let keys_directory = PathBuf::from(KEYS_DIRECTORY);
	if !keys_directory.exists() {
		fs::create_dir_all(&keys_directory).unwrap();
	}
	let pub_key_path = keys_directory.join(PathBuf::from(PUB_KEY_FILENAME));
	let priv_key_path = keys_directory.join(PathBuf::from(PRIV_KEY_FILENAME));

	//Sanity check
	if pub_key_path.exists() {
		return Err(Box::new(KeyPairLoadError::PubKeyExists));
	}
	if priv_key_path.exists() {
		return Err(Box::new(KeyPairLoadError::PrivKeyExists));
	}

	//Generate
	let keys_dalek = ed25519_dalek::Keypair::generate(&mut rng);
	let keys: IdentityKeyPair = (&keys_dalek).into();

	let keypair_bytes = ed25519::pkcs8::KeypairBytes {
		secret_key: keys_dalek.secret.to_bytes(),
		public_key: Some(keys_dalek.public.to_bytes()),
	};

	let rng = pkcs8::rand_core::OsRng::default();

	//Serialize private key
	let priv_key_file_string = match passphrase {
		Some(pass) => keypair_bytes.to_pkcs8_encrypted_pem(rng, pass, pkcs8::LineEnding::default()),
		None => keypair_bytes.to_pkcs8_pem(pkcs8::LineEnding::default()),
	}?;

	let mut file = OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(false)
		.open(priv_key_path)?;

	file.write_all(priv_key_file_string.as_bytes())?;
	file.flush()?;
	drop(file);

	//Serialize public keys
	let pub_key_document = pkcs8::PublicKeyDocument::try_from(pkcs8::SubjectPublicKeyInfo {
		algorithm: AlgorithmIdentifier {
			oid: ed25519::pkcs8::ALGORITHM_OID,
			parameters: None,
		},
		subject_public_key: &keys.public.0,
	})?;

	pub_key_document.write_public_key_pem_file(pub_key_path, pkcs8::LineEnding::default())?;

	Ok(keys)
}

pub fn does_private_key_need_passphrase() -> Result<bool, Box<dyn std::error::Error>> {
	let keys_directory = PathBuf::from(KEYS_DIRECTORY);
	let priv_key_path = keys_directory.join(PathBuf::from(PRIV_KEY_FILENAME));

	let private_key_string = fs::read_to_string(priv_key_path)?;

	Ok(private_key_string.contains("ENCRYPTED"))
}

pub fn load_local_identity_keys(
	passphrase: Option<String>,
) -> Result<IdentityKeyPair, Box<dyn std::error::Error>> {
	let keys_directory = PathBuf::from(KEYS_DIRECTORY);
	let pub_key_path = keys_directory.join(PathBuf::from(PUB_KEY_FILENAME));
	let priv_key_path = keys_directory.join(PathBuf::from(PRIV_KEY_FILENAME));

	//Private key
	let private_key_string = fs::read_to_string(priv_key_path)?;

	let private_key_bytes = match passphrase {
		Some(pass) => {
			ed25519::pkcs8::KeypairBytes::from_pkcs8_encrypted_pem(&private_key_string, pass)
		}
		None => ed25519::pkcs8::KeypairBytes::from_pkcs8_pem(&private_key_string),
	}?;

	// Public key
	let public_key_string = fs::read_to_string(pub_key_path)?;

	let public_key_document = pkcs8::PublicKeyDocument::from_public_key_pem(&public_key_string)?;
	let public_key_info = public_key_document.decode();

	let mut public_key_bytes: [u8; PUBLIC_KEY_LENGTH] = [0; PUBLIC_KEY_LENGTH];
	if public_key_info.subject_public_key.len() != PUBLIC_KEY_LENGTH {
		return Err(Box::new(KeyPairLoadError::WrongLengthPublic(
			public_key_info.subject_public_key.len(),
		)));
	}
	public_key_bytes.copy_from_slice(&public_key_info.subject_public_key[0..PUBLIC_KEY_LENGTH]);

	public_key_info
		.algorithm
		.assert_algorithm_oid(ed25519::pkcs8::ALGORITHM_OID)?;

	Ok(IdentityKeyPair {
		public: NodeIdentity(public_key_bytes),
		private: PrivateKey(private_key_bytes.secret_key),
	})
}
