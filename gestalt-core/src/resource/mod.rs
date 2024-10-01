use crate::common::identity::{NodeIdentity, PublicKey};
use crate::common::{new_fast_hash_map, FastHashMap};
use crate::message::RecvError;

use base64::Engine;
use ed25519::Signature;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::error::Error;
use std::fmt::Debug;
use std::path::PathBuf;
use std::{cmp::PartialEq, hash::Hash};

use base64::engine::general_purpose::URL_SAFE as BASE_64;

use self::channels::RESOURCE_FETCH;

//use string_cache::DefaultAtom as Atom;

pub mod channels;
pub mod image;
pub mod provider;
pub mod retrieval;
//pub mod module; //Beware of redundant names.

pub const CURRENT_RESOURCE_ID_FORMAT: u8 = 1;

/// Content-addressed identifier (CAID) for a Gestalt resource.
/// String representation starts with a version number for the
/// ResourceId structure, then a `.` delimeter, then the size (number of bytes)
/// in the resource, then the 32-byte Sha256-512 hash encoded in base-64.
/// For example, `1.2048.J1kVZSSu8LHZzw25mTnV5lhQ8Zqt9qU6V1twg5lq2e6NzoUA` would be a version 1 ResourceID.
#[repr(C)]
#[derive(Copy, Clone, PartialOrd, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Caid {
	/// Which version of the ResourceId struct is this?
	pub version: u8,
	/// Length in bytes of the resource.
	pub length: u64,
	/// 32-byte Sha256-512 hash
	pub hash: [u8; 32],
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ParseCaidError {
	#[error("tried to parse {0} into a ResourceId but it contained no separator.")]
	NoSeparator(String),
	#[error("tried to parse {0} into a ResourceId but was a greater-than-3 number of separators")]
	TooManySeparators(String),
	#[error("string `{0}` is not a valid resource ID because it contains whitespace")]
	ContainsWhitespace(String),
	#[error("couldn't parse a ResourceId: base64 error {0:?}")]
	Base64Parse(#[from] base64::DecodeError),
	#[error(
		"expected to parse a 32-byte hash out of the base64 in {0} but the byte buffer we got was {1} bytes in length"
	)]
	BufferWrongSize(String, usize),
	#[error("could not parse {0} as a resource ID, version (1st field) number needs to be parseable as an integer")]
	VersionNotNumber(String),
	#[error("could not parse {0} as a resource ID, length (2nd field, byte-size) needs to be parseable as an integer")]
	SizeNotNumber(String),
	#[error("could not parse {0} as a resource ID, did not recognize ResourceId format {1}. Most likely this was sent by a newer version of the Gestalt Engine")]
	UnrecognizedVersion(String, u8),
}
const SEP: char = '.';

#[derive(thiserror::Error, Debug, Clone)]
pub enum VerifyResourceError {
	#[error("Hash does not match Resource ID")]
	HashesDontMatch,
	#[error("Expected a length of {0} bytes for this resource but we got a length of {1}")]
	WrongLength(u64, u64),
}
impl Caid {
	/// Make a ResourceId. Use from_buf() if you have a buffer fully loaded into memory already.
	/// ResourceId::new(), on the other hand, is ideal for if you have a
	pub fn new(length: usize, hash: [u8; 32]) -> Self {
		Caid {
			version: CURRENT_RESOURCE_ID_FORMAT,
			length: length as u64,
			hash,
		}
	}
	/// Generate a ResourceID for a buffer which is fully loaded into memory.
	pub fn from_buf(buf: &[u8]) -> Self {
		// Make a hash
		let mut hasher = sha2::Sha512_256::new();
		hasher.update(buf);
		let buffer_hash = hasher.finalize();
		// Done, here's a ResourceId
		Caid {
			version: CURRENT_RESOURCE_ID_FORMAT,
			length: buf.len() as u64,
			hash: buffer_hash.into(),
		}
	}
	pub fn verify(&self, buf: &[u8]) -> Result<(), VerifyResourceError> {
		//Correct length?
		if buf.len() as u64 != self.length {
			return Err(VerifyResourceError::WrongLength(self.length, buf.len() as u64));
		}
		//Check hash
		let mut hasher = sha2::Sha512_256::new();
		hasher.update(buf);
		let buffer_hash = hasher.finalize();

		if buffer_hash != self.hash.into() {
			return Err(VerifyResourceError::HashesDontMatch);
		}

		//Matches description!
		Ok(())
	}

	pub fn parse(value: &str) -> Result<Self, ParseCaidError> {
		if !value.contains(SEP) {
			return Err(ParseCaidError::NoSeparator(value.to_string()));
		}

		let fields: Vec<&str> = value.split(SEP).collect();
		if fields.len() != 3 {
			return Err(ParseCaidError::TooManySeparators(value.to_string()));
		}

		let version = (*fields.get(0).unwrap())
			.parse::<u8>()
			.map_err(|_| ParseCaidError::VersionNotNumber(value.to_string()))?;
		if version != CURRENT_RESOURCE_ID_FORMAT {
			return Err(ParseCaidError::UnrecognizedVersion(value.to_string(), version));
		}

		let length = (*fields.get(1).unwrap())
			.parse::<u64>()
			.map_err(|_| ParseCaidError::VersionNotNumber(value.to_string()))?;

		let bytes = BASE_64.decode(fields.get(2).unwrap())?;
		if bytes.len() != 32 {
			return Err(ParseCaidError::BufferWrongSize(value.to_string(), bytes.len()));
		}

		let mut hash: [u8; 32] = [0; 32];
		hash.copy_from_slice(&bytes[0..32]);
		Ok(Caid {
			version,
			length,
			hash,
		})
	}
}

impl std::fmt::Display for Caid {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}{}{}{}{}", self.version, SEP, self.length, SEP, BASE_64.encode(&self.hash))
	}
}

impl std::fmt::Debug for Caid {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "CAID:({})", self)
	}
}

// For use with serde
pub mod caid_base64_string {
	use serde::{
		de::{self, Visitor},
		Deserializer, Serializer,
	};
	use std::fmt;

	use super::*;

	pub fn serialize<S>(val: &Caid, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(val.to_string().as_str())
	}

	struct CaidStringVisitor;

	impl<'de> Visitor<'de> for CaidStringVisitor {
		type Value = Caid;

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			formatter
				.write_str(&format!("a content-addressing ID string following the form format_version{}size_in_bytes{}hash_of_resource ", SEP, SEP))
		}

		fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
		where
			E: de::Error,
		{
			Caid::parse(v).map_err(E::custom)
		}
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Caid, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_string(CaidStringVisitor {})
	}
}

// Todo! 
#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ResourceLinkProvenance { 
	LinkKey {
		link_key: PublicKey,
	}, 
	/// A resource link whose authority has been transferred to a specific node (server or user)
	NodeAuth {
		original_link_key: PublicKey,
		new_author: NodeIdentity,
		/// New revision increment representing ownership change
		as_of: u64,
		/// Signs "[new_author, as_of]" bytes (packed)
		sig_from_original: Signature,
		/// Signs "[original_link_key, as_of]" bytes (packed)
		sig_from_new: Signature,
	}
}
// Todo! 
#[repr(C)]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceLinkFull { 
	pub link_key: ResourceLinkProvenance,
	/// Revision number of the resource i.e. 1 is initial binding.
	pub revision: u64,
	// Todo: string-interner strings for this maybe!! 
	pub alias: String,
	/// Signs [link_key.current_public_key, revision, ] with identity corresponding to link_key.current_public_key
	pub sig: Signature,
}

// Todo! 
#[repr(C)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum LinkProvenanceShort { 
	LinkKey(PublicKey),
	NodeAuth(PublicKey),
}
// Todo! 
#[repr(C)]
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct ResourceLinkShort {
	pub auth: LinkProvenanceShort,
	pub revision: u64,
	// Todo: string-interner strings for this maybe!!
	pub alias: String,
}

#[repr(C)]
#[derive(Clone, PartialOrd, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum LocalResource {
	User(PathBuf),
	// TODO: interned strings instead of plain-old strings
	Internal(String),
}

#[repr(C)]
#[derive(Clone, PartialOrd, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum ResourceLocation { 
	#[serde(rename = "CAID")]
	Caid(Caid),
	Local(LocalResource),
	/// UNSTABLE API, DO NOT USE
	Link(ResourceLinkShort),
}

impl ResourceLocation {
	/// Intended for internal / engine use - does not necessarily correspond to metadata / original filename.
	pub(crate) fn file_name(&self) -> ResourceFilelike {
		match self {
			ResourceLocation::Caid(id) => ResourceFilelike::File(PathBuf::from(id.to_string())),
			ResourceLocation::Local(loc) => match loc {
				LocalResource::User(file) => ResourceFilelike::File(file.clone()),
				LocalResource::Internal(internal) => ResourceFilelike::Internal(internal.clone()),
			},
			ResourceLocation::Link(_) => todo!(),
		}
	}
}

pub(crate) enum ResourceFilelike {
	File(PathBuf),
	Internal(String),
}

/// Used to keep track of a resource locally
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceInfo {
	/// Which resource?
	pub id: Caid,
	/// What did the "creator" user call this resource?
	pub filename: String,
	/// Which user claims to have "made" this resource? Who signed it, who is the authority on it?
	pub creator: NodeIdentity,
	/// Expected type. MIME Type for PlainOldData, @{ManifestType} for manifest types e.g. @Module
	pub resource_type: String,
	/// Name of creator user and friends who made this resource.
	pub authors: String,
	/// What does the author have to say about this one?
	pub description: Option<String>,
	// /// Is there anything else we need to make use of this resource? I love recursion.
	// pub dependencies: Option<Vec<Box<ResourceInfo>>>,
	/// Signature verifying our binary blob (referred to by ResourceId) as good, signed with the public key from creator's NodeIdentity.
	pub signature: Signature,
}

impl Hash for ResourceInfo {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
		self.creator.hash(state);
	}
}

impl PartialEq for ResourceInfo {
	fn eq(&self, other: &Self) -> bool {
		//Elide name.
		self.creator == other.creator && self.id == other.id
		//The naive form of this would be self.id == other.id && self.origin == other.origin && self.name == other.name
		//but we want equality to be entirely based on origin and hash.
	}
}

/// Any resource-loading error that pertains to fetching the raw resource bytes in the first place,
/// and not to parsing or processing any specific file type.
#[derive(thiserror::Error, Debug, Clone)]
pub enum ResourceRetrievalError {
	#[error("While trying to retrieve resource {0:?}, a network error was encountered: {1}")]
	Network(ResourceLocation, String),
	#[error("Error loading resource {0:?} from disk: {1}")]
	Disk(ResourceLocation, String),
	#[error("Tried to access a resource {0:?}, which cannot be found (is not indexed) locally or on any connected server.")]
	NotFound(ResourceLocation),
	#[error("Timed out while attempting to fetch resource {0:?}.")]
	Timeout(Caid),
	#[error("Failed to verify resource {0:?} due to error {1:?}.")]
	Verification(Caid, VerifyResourceError),
	#[error("Message-passing error while trying to load resource {0:?}: {1}.")]
	ChannelError(ResourceLocation, String),
}

pub enum ResourceError<E>
where
	E: Debug,
{
	Channel(RecvError),
	Retrieval(ResourceRetrievalError),
	Parse(ResourceLocation, E),
}

impl<E> Clone for ResourceError<E>
where
	E: Debug + Clone,
{
	fn clone(&self) -> Self {
		match self {
			Self::Channel(arg0) => Self::Channel(arg0.clone()),
			Self::Retrieval(arg0) => Self::Retrieval(arg0.clone()),
			Self::Parse(arg0, arg1) => Self::Parse(arg0.clone(), arg1.clone()),
		}
	}
}

impl<E> Debug for ResourceError<E>
where
	E: Error + Clone,
{
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::Channel(recv) => f.write_fmt(format_args!(
				"Error encountered while polling a channel\
				to retrieve resources: {0:?}",
				recv
			)),
			Self::Retrieval(e) => e.fmt(f),
			Self::Parse(id, e) => f.write_fmt(format_args!(
				"While attempting to parse / load resource ID {0:?},\
				an error was encountered: {1}",
				id, e
			)),
		}
	}
}

impl<E> From<RecvError> for ResourceError<E>
where
	E: Debug,
{
	fn from(value: RecvError) -> Self {
		Self::Channel(value)
	}
}

impl<E> From<ResourceRetrievalError> for ResourceError<E>
where
	E: Debug,
{
	fn from(value: ResourceRetrievalError) -> Self {
		Self::Retrieval(value)
	}
}

impl Eq for ResourceInfo {}

pub const ID_ERRORED_RESOURCE: Caid = Caid {
	version: 0,
	length: 0,
	hash: [0; 32],
};

#[derive(Debug)]
pub enum ResourceStatus<E>
where
	E: Debug,
{
	NotInitiated,
	Pending,
	Errored(E),
	Ready,
}

impl<E> Clone for ResourceStatus<E>
where
	E: Clone + Debug,
{
	fn clone(&self) -> Self {
		match self {
			Self::NotInitiated => Self::NotInitiated,
			Self::Pending => Self::Pending,
			Self::Errored(e) => Self::Errored(e.clone()),
			Self::Ready => Self::Ready,
		}
	}
}

pub(self) fn resource_id_to_prefix(resource: &Caid) -> usize {
	#[cfg(target_endian = "little")]
	{
		resource.hash[31] as usize
	}

	#[cfg(target_endian = "big")]
	{
		resource.hash[0] as usize
	}
}
// Intended to be used as a const (global)
struct ResourceStorage<T: Send + Sized> {
	buckets: once_cell::sync::Lazy<[tokio::sync::RwLock<FastHashMap<Caid, T>>; 256]>,
}

impl<T> ResourceStorage<T>
where
	T: Send + Sized + Clone,
{
	pub const fn new() -> Self {
		Self {
			buckets: once_cell::sync::Lazy::new(|| {
				std::array::from_fn(|_i| tokio::sync::RwLock::new(new_fast_hash_map()))
			}),
		}
	}
	pub async fn get(&self, id: &Caid) -> Option<T> {
		let guard = self.buckets[resource_id_to_prefix(id)].read().await;
		guard.get(id).cloned()
	}
	pub fn get_blocking(&self, id: &Caid) -> Option<T> {
		let guard = self.buckets[resource_id_to_prefix(id)].blocking_read();
		guard.get(id).cloned()
	}
	pub async fn insert(&self, id: Caid, value: T) -> Option<T> {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].write().await;
		guard.insert(id, value)
	}
	pub fn insert_blocking(&self, id: Caid, value: T) -> Option<T> {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].blocking_write();
		guard.insert(id, value)
	}
	pub async fn remove(&self, id: &Caid) -> Option<T> {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].write().await;
		guard.remove(&id)
	}
	pub fn remove_blocking(&self, id: &Caid) -> Option<T> {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].blocking_write();
		guard.remove(&id)
	}
	pub async fn update(&self, id: &Caid, new: T) {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].write().await;
		let reference = guard.get_mut(id);
		match reference {
			Some(inner) => *inner = new,
			None => _ = guard.insert(id.clone(), new),
		}
	}
	pub fn update_blocking(&self, id: &Caid, new: T) {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].blocking_write();
		let reference = guard.get_mut(id);
		match reference {
			Some(inner) => *inner = new,
			None => _ = guard.insert(id.clone(), new),
		}
	}
}

pub enum ResourcePoll<T, E>
where
	E: Debug,
{
	Ready(ResourceLocation, T),
	Err(ResourceError<E>),
	/// End of stream, the channel is empty. If you are polling in a loop you can stop polling.
	None,
}

impl<T, E> ResourcePoll<T, E>
where
	E: Debug,
{
	pub fn is_none(&self) -> bool {
		match self {
			ResourcePoll::Ready(_, _) => false,
			ResourcePoll::Err(_) => false,
			ResourcePoll::None => true,
		}
	}
}

static RESOURCE_METADATA: ResourceStorage<ResourceInfo> = ResourceStorage::new();

pub fn update_global_resource_metadata(id: &Caid, info: ResourceInfo) {
	RESOURCE_METADATA.update_blocking(id, info);
}

pub fn get_resource_metadata(id: &Caid) -> Option<ResourceInfo> {
	RESOURCE_METADATA.get_blocking(id)
}

#[derive(Clone)]
pub enum ResourceIdOrMeta {
	Id(Caid),
	Meta(ResourceInfo),
}
impl ResourceIdOrMeta {
	pub fn short_name(&self) -> String {
		match self {
			ResourceIdOrMeta::Id(id) => format!("ResourceId {} (metadata not found)", id),
			ResourceIdOrMeta::Meta(meta) => {
				format!("{} (from user {:?})", meta.filename, meta.creator)
			}
		}
	}
}

impl std::fmt::Debug for ResourceIdOrMeta {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			ResourceIdOrMeta::Id(id) => write!(f, "ResourceId {} (metadata not found)", id),
			ResourceIdOrMeta::Meta(m) => write!(f, "{:?}", m),
		}
	}
}

macro_rules! resource_debug {
	($rid:expr) => {{
		use crate::resource::*;
		let rid_eval = ($rid).clone();
		let ridom = match get_resource_metadata(&rid_eval) {
			Some(m) => ResourceIdOrMeta::Meta(m.clone()),
			None => ResourceIdOrMeta::Id(rid_eval.clone()),
		};
		ridom.short_name()
	}};
}

#[test]
fn resource_id_generate() {
	use rand::rngs::OsRng;
	use rand::Rng;

	let mut rng = OsRng::default();

	let mut buf1: [u8; 1024] = [0; 1024];
	let mut buf2: [u8; 1024] = [0; 1024];

	{
		rng.fill(&mut buf1);
		rng.fill(&mut buf2);
	}

	let rid1 = Caid::from_buf(&buf1);
	let rid2 = Caid::from_buf(&buf2);

	assert_eq!(rid1.length, 1024);
	assert_eq!(rid2.length, 1024);

	assert_eq!(rid1.version, CURRENT_RESOURCE_ID_FORMAT);
	assert_eq!(rid2.version, CURRENT_RESOURCE_ID_FORMAT);

	//These should not be equal.
	assert_ne!(rid1, rid2);
}

#[test]
fn resource_id_to_string() {
	use rand::rngs::OsRng;
	use rand::Rng;

	let mut rng = OsRng::default();

	const BUF_SIZE: usize = 2048;

	let mut buf1: [u8; BUF_SIZE] = [0; BUF_SIZE];

	{
		rng.fill(&mut buf1);
	}

	let rid1 = Caid::from_buf(&buf1);

	let stringified = rid1.to_string();

	let b64hash = BASE_64.encode(&rid1.hash);

	//Our hash should be in here
	assert!(stringified.contains(&b64hash));

	let format_string = format!("{}", CURRENT_RESOURCE_ID_FORMAT);
	assert!(stringified.starts_with(&format_string));

	let after_split: Vec<&str> = stringified.split(SEP).collect();

	assert_eq!(after_split.len(), 3);
	assert_eq!(after_split.get(1).unwrap().parse::<u64>().unwrap(), BUF_SIZE as u64);
}
