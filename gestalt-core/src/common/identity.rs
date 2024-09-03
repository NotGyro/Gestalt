use aes_gcm::{
	aead::{Aead, AeadCore, KeyInit},
	Aes256Gcm,
};

use argon2::Argon2;
use base64::engine::general_purpose::URL_SAFE as BASE_64;
use base64::Engine;
use rand::Rng;
use rand_core::CryptoRngCore;
use serde_with_macros::serde_as;
use signature::{Signer, Verifier};

use serde::{Deserialize, Serialize};

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
	pub fn verify_signature(
		&self,
		message: &[u8],
		signature: &[u8; 64],
	) -> Result<(), SignatureError> {
		let converted_key: ed25519_dalek::VerifyingKey = self.into();
		let converted_signature = ed25519_dalek::Signature::from_bytes(signature);
		converted_key.verify(message, &converted_signature)
	}
}

impl std::fmt::Debug for NodeIdentity {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "NodeIdentity({})", self.to_base64())
	}
}

impl From<&NodeIdentity> for ed25519_dalek::VerifyingKey {
	fn from(value: &NodeIdentity) -> Self {
		ed25519_dalek::VerifyingKey::from_bytes(&value.0).unwrap()
	}
}

impl From<&ed25519_dalek::VerifyingKey> for NodeIdentity {
	fn from(value: &ed25519_dalek::VerifyingKey) -> Self {
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
		value.0.clone()
	}
}
impl From<&ed25519_dalek::SecretKey> for PrivateKey {
	fn from(value: &ed25519_dalek::SecretKey) -> Self {
		PrivateKey(value.clone())
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
		let keys_dalek = ed25519_dalek::SigningKey::generate(&mut rng);
		(&keys_dalek).into()
	}
}

impl From<&IdentityKeyPair> for ed25519_dalek::SigningKey {
	fn from(value: &IdentityKeyPair) -> Self {
		ed25519_dalek::SigningKey::from_bytes(&value.private.0)
	}
}
impl From<&ed25519_dalek::SigningKey> for IdentityKeyPair {
	fn from(value: &ed25519_dalek::SigningKey) -> Self {
		IdentityKeyPair {
			public: (&value.verifying_key()).into(),
			private: (&value.to_bytes()).into(),
		}
	}
}

impl IdentityKeyPair {
	pub fn sign(&self, msg: &[u8]) -> Result<Signature, SignatureError> {
		let converted_keypair: ed25519_dalek::SigningKey = self.into();
		Ok(converted_keypair.sign(msg))
	}
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone)]
pub struct KeyFileEncryption {
	/// Is the private key (and only the private key) encrypted by a passcode?
	/// Note that the public key will always be visible
	pub passcode_encrypted: bool,
	#[serde_as(as = "serde_with::base64::Base64")]
	pub nonce: Vec<u8>,
	#[serde_as(as = "serde_with::base64::Base64")]
	pub salt: Vec<u8>,
}
impl KeyFileEncryption {
	fn make_argon<'a>() -> Argon2<'a> {
		let params = argon2::ParamsBuilder::default()
			.output_len(32)
			.build()
			.expect("Failed to build argon2 params string!");
		// Argon2 with default params (Argon2id v19)
		Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::default(), params)
	}
	pub(crate) fn passphrase_to_hash(
		&self,
		passphrase: &str,
	) -> Result<[u8; 32], KeyPairLoadError> {
		let argon2 = Self::make_argon();
		// Hash password to PHC string ($argon2id$v=19$...)
		let mut out: [u8; 32] = [0; 32];
		argon2.hash_password_into(passphrase.as_bytes(), &self.salt, &mut out)?;
		Ok(out)
	}
	/// Returns (Self, nonce)
	pub(crate) fn generate<R>(rng: &mut R) -> Result<(Self, [u8; 12]), KeyPairLoadError>
	where
		R: Rng + CryptoRngCore,
	{
		let mut salt_bytes: [u8; 16] = [0; 16];
		rng.fill(&mut salt_bytes);
		let salt_vec = Vec::from(salt_bytes);

		let nonce_bytes: [u8; 12] = Aes256Gcm::generate_nonce(rng).into();
		let nonce_vec = Vec::from(nonce_bytes);

		Ok((
			KeyFileEncryption {
				passcode_encrypted: true,
				nonce: nonce_vec,
				salt: salt_vec,
			},
			nonce_bytes,
		))
	}
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct KeyFile {
	pub encryption: Option<KeyFileEncryption>,
	#[serde_as(as = "serde_with::base64::Base64")]
	pub private_key: Vec<u8>,
	#[serde_as(as = "serde_with::base64::Base64")]
	pub public_key: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub enum KeyFileVersion {
	Ed25519WithAes256GcmPassHashArgon2,
}

// Allow dead_code since there is only one version right now, but soon there may be more.
#[allow(dead_code)]
#[derive(Serialize, Deserialize, Clone)]
pub struct VersionedKeyFile {
	pub version: KeyFileVersion,
	#[serde(flatten)]
	pub key_file: KeyFile,
}

#[derive(thiserror::Error, Debug)]
pub enum KeyPairLoadError {
	#[error("Wrong length for private key: expected 32 bytes and got {0}")]
	WrongLengthPrivate(usize),
	#[error("Wrong length for public key: expected 32 bytes and got {0}")]
	WrongLengthPublic(usize),
	#[error("Wrong length for the passcode-encryption nonce for the private key: expected 12 bytes and got {0}")]
	WrongLengthNonce(usize),
	#[error("Wrong length for the passcode-encryption salt for the private key: expected 16 bytes and got {0}")]
	WrongLengthSalt(usize),
	#[error("Tried to generate a public key but we already have a public key!")]
	PubKeyExists,
	#[error("Tried to generate a private key but we already have a public key!")]
	PrivKeyExists,
	#[error("Loaded a keyfile, but our private key does not match our public key")]
	PrivPubMismatch,
	#[error("Keyfile needs passphrase but no passphrase was provided")]
	NoPassphrase,
	#[error("Passphrase decryption failed: {0}")]
	FailedDecryption(aes_gcm::Error),
	#[error("Passphrase hasher error: {0}")]
	HasherError(password_hash::Error),
	#[error("Passphrase hasher error in argon2: {0}")]
	ArgonHashError(argon2::Error),
	#[error("Hasher produced no output")]
	NoHashOut,
}
impl From<aes_gcm::Error> for KeyPairLoadError {
	fn from(value: aes_gcm::Error) -> Self {
		KeyPairLoadError::FailedDecryption(value)
	}
}
impl From<password_hash::Error> for KeyPairLoadError {
	fn from(value: password_hash::Error) -> Self {
		KeyPairLoadError::HasherError(value)
	}
}
impl From<argon2::Error> for KeyPairLoadError {
	fn from(value: argon2::Error) -> Self {
		KeyPairLoadError::ArgonHashError(value)
	}
}
impl KeyFile {
	pub fn needs_passphrase(&self) -> bool {
		self.encryption
			.as_ref()
			.is_some_and(|v| v.passcode_encrypted)
	}
	pub fn try_read(self, passphrase: Option<&str>) -> Result<IdentityKeyPair, KeyPairLoadError> {
		let priv_key_buf: Vec<u8> = if self.needs_passphrase() {
			// Needs_passphrase also checks this.
			let encryption = self.encryption.unwrap();
			// Sanity-check nonce len
			if encryption.nonce.len() != 12 {
				return Err(KeyPairLoadError::WrongLengthNonce(encryption.nonce.len()));
			}

			let passphrase = passphrase.ok_or(KeyPairLoadError::NoPassphrase)?;
			let passphrase_byte_hash = encryption.passphrase_to_hash(passphrase)?;
			let mut nonce: [u8; 12] = [0; 12];
			nonce.copy_from_slice(&encryption.nonce);
			let nonce = nonce.into(); // Required for GenericArray type.

			let pass_key = aes_gcm::Key::<Aes256Gcm>::from_slice(&passphrase_byte_hash);

			// Only possible Err() value here is a InvalidLength and we're giving it a fixed-size 32-byte key.
			let cipher = Aes256Gcm::new_from_slice(&pass_key).unwrap();

			cipher.decrypt(&nonce, self.private_key.as_ref())?
		} else {
			self.private_key
		};
		if priv_key_buf.len() != PRIVATE_KEY_LENGTH {
			return Err(KeyPairLoadError::WrongLengthPrivate(priv_key_buf.len()));
		}
		if self.public_key.len() != PUBLIC_KEY_LENGTH {
			return Err(KeyPairLoadError::WrongLengthPublic(self.public_key.len()));
		}

		let mut priv_key_bytes: [u8; PRIVATE_KEY_LENGTH] = [0; PRIVATE_KEY_LENGTH];
		priv_key_bytes.copy_from_slice(&priv_key_buf);

		let mut pub_key_bytes: [u8; PUBLIC_KEY_LENGTH] = [0; PUBLIC_KEY_LENGTH];
		pub_key_bytes.copy_from_slice(&self.public_key);

		// Check to make sure ed25519_dalek thinks our public key matches our private key.
		let dalek_keys = ed25519_dalek::SigningKey::from_bytes(&priv_key_bytes);
		if &pub_key_bytes != dalek_keys.verifying_key().as_bytes() {
			return Err(KeyPairLoadError::PrivPubMismatch);
		}

		let private = PrivateKey(priv_key_bytes);
		let public = NodeIdentity(pub_key_bytes);
		Ok(IdentityKeyPair { public, private })
	}
}

impl VersionedKeyFile {
	pub fn needs_passphrase(&self) -> bool {
		self.key_file.needs_passphrase()
	}
	pub fn try_read(self, passphrase: Option<&str>) -> Result<IdentityKeyPair, KeyPairLoadError> {
		self.key_file.try_read(passphrase)
	}
}

/// Returns false if key files exist already and true if they don't exist yet and need to be made.
pub fn do_keys_need_generating(keys_directory: PathBuf, expected_filename: &str) -> bool {
	if !keys_directory.exists() {
		fs::create_dir_all(keys_directory).unwrap();
		return true;
	}
	let key_path = keys_directory.join(PathBuf::from(expected_filename));

	if !key_path.exists() {
		return true;
	} else if key_path.is_dir() {
		panic!("Public key file {} cannot be a directory!", key_path.display());
	}

	// All sanity checks passed, keys exist so we don't need to generate them.
	false
}

pub fn generate_local_keys(
	passphrase: Option<&str>,
) -> Result<(IdentityKeyPair, VersionedKeyFile), Box<dyn std::error::Error>> {
	let mut rng = rand_core::OsRng::default();

	//Generate
	let keys_dalek = ed25519_dalek::SigningKey::generate(&mut rng);
	let keys: IdentityKeyPair = (&keys_dalek).into();

	//Serialize private key
	let (encryption, private_key_bytes) = match passphrase {
		Some(pass) => {
			// Passphrase hashing / argon2 stuff goes here.
			let (encryption, nonce) = KeyFileEncryption::generate(&mut rng)?;
			let pass_hash = encryption.passphrase_to_hash(pass)?;

			let key: &aes_gcm::Key<Aes256Gcm> = (&pass_hash).into();
			let cipher = Aes256Gcm::new(&key);
			let ciphertext = cipher.encrypt((&nonce).into(), keys.private.0.as_slice())?;
			(Some(encryption), ciphertext)
		}
		None => (None, Vec::from(&keys.private.0)),
	};

	let key_file = VersionedKeyFile {
		version: KeyFileVersion::Ed25519WithAes256GcmPassHashArgon2,
		key_file: KeyFile {
			encryption,
			private_key: private_key_bytes,
			public_key: Vec::from(&keys.public.0),
		},
	};
	Ok((keys, key_file))
}

pub fn gen_and_save_keys(
	passphrase: Option<&str>,
	keys_directory: PathBuf,
	keys_filename: &str,
) -> Result<IdentityKeyPair, Box<dyn std::error::Error>> {
	// Set up paths
	let keys_directory = PathBuf::from(keys_directory);
	if !keys_directory.exists() {
		fs::create_dir_all(&keys_directory).unwrap();
	}
	let key_path = keys_directory.join(PathBuf::from(keys_filename));

	//Sanity check
	if key_path.exists() {
		return Err(Box::new(KeyPairLoadError::PrivKeyExists));
	}

	let mut file = OpenOptions::new()
		.create(true)
		.write(true)
		.truncate(false)
		.open(key_path)?;

	let (keys, keyfile_data) = generate_local_keys(passphrase)?;

	let keyfile_string = toml::to_string_pretty(&keyfile_data)?;

	file.write_all(keyfile_string.as_bytes())?;
	file.flush()?;
	drop(file);

	Ok(keys)
}

pub fn load_keyfile(
	keys_directory: PathBuf,
	keys_filename: &str,
) -> Result<VersionedKeyFile, Box<dyn std::error::Error>> {
	let keys_directory = PathBuf::from(keys_directory);
	let key_path = keys_directory.join(PathBuf::from(keys_filename));

	let keyfile_string = fs::read_to_string(key_path)?;

	let keyfile: VersionedKeyFile = toml::from_str(&keyfile_string)?;
	Ok(keyfile)
}
