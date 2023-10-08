use crate::common::{FastHashMap, new_fast_hash_map};
use crate::common::identity::NodeIdentity;
use crate::message::{
	MpscChannel, 
	MpscSender,
};

use base64::Engine;
use ed25519::Signature;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::fmt::Debug;
use std::sync::Arc;
use std::{cmp::PartialEq, hash::Hash};

use base64::engine::general_purpose::URL_SAFE as BASE_64;

//use string_cache::DefaultAtom as Atom;

pub mod image;
pub mod retrieval;
//pub mod module; //Beware of redundant names.

pub const CURRENT_RESOURCE_ID_FORMAT: u8 = 1;

/// Content-addressed identifier for a Gestalt resource.
/// String representation starts with a version number for the
/// ResourceId structure, then a `_` delimeter, then the size (number of bytes)
/// in the resource, then the 32-byte Sha256-512 hash encoded in base-64.
/// For example, `1_2048_J1kVZSSu8LHZzw25mTnV5lhQ8Zqt9qU6V1twg5lq2e6NzoUA` would be a version 1 ResourceID.
#[repr(C)]
#[derive(Copy, Clone, PartialOrd, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceId {
	/// Which version of the ResourceId struct is this?
	pub version: u8,
	/// Length in bytes of the resource.
	pub length: u64,
	/// 32-byte Sha256-512 hash
	pub hash: [u8; 32],
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ParseResourceIdError {
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
const SEP: char = '_';

#[derive(thiserror::Error, Debug, Clone)]
pub enum VerifyResourceError {
	#[error("Hash does not match Resource ID")]
	HashesDontMatch,
	#[error("Expected a length of {0} bytes for this resource but we got a length of {1}")]
	WrongLength(u64, u64),
}
impl ResourceId {
	/// Make a ResourceId. Use from_buf() if you have a buffer fully loaded into memory already.
	/// ResourceId::new(), on the other hand, is ideal for if you have a
	pub fn new(length: usize, hash: [u8; 32]) -> Self {
		ResourceId {
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
		ResourceId {
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

	pub fn parse(value: &str) -> Result<Self, ParseResourceIdError> {
		if !value.contains(SEP) {
			return Err(ParseResourceIdError::NoSeparator(value.to_string()));
		}

		let fields: Vec<&str> = value.split(SEP).collect();
		if fields.len() != 3 {
			return Err(ParseResourceIdError::TooManySeparators(value.to_string()));
		}

		let version = (*fields.get(0).unwrap())
			.parse::<u8>()
			.map_err(|_| ParseResourceIdError::VersionNotNumber(value.to_string()))?;
		if version != CURRENT_RESOURCE_ID_FORMAT {
			return Err(ParseResourceIdError::UnrecognizedVersion(value.to_string(), version));
		}

		let length = (*fields.get(1).unwrap())
			.parse::<u64>()
			.map_err(|_| ParseResourceIdError::VersionNotNumber(value.to_string()))?;

		let bytes = BASE_64.decode(fields.get(2).unwrap())?;
		if bytes.len() != 32 {
			return Err(ParseResourceIdError::BufferWrongSize(value.to_string(), bytes.len()));
		}

		let mut hash: [u8; 32] = [0; 32];
		hash.copy_from_slice(&bytes[0..32]);
		Ok(ResourceId {
			version,
			length,
			hash,
		})
	}
}

impl std::fmt::Display for ResourceId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}{}{}{}{}", self.version, SEP, self.length, SEP, BASE_64.encode(&self.hash))
	}
}

impl std::fmt::Debug for ResourceId {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "ResourceId:({})", self)
	}
}

// For use with serde
pub mod resourceid_base64_string {
	use serde::{
		de::{self, Visitor},
		Deserializer, Serializer,
	};
	use std::fmt;

	use super::*;

	pub fn serialize<S>(val: &ResourceId, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(val.to_string().as_str())
	}

	struct ResourceIdStringVisitor;

	impl<'de> Visitor<'de> for ResourceIdStringVisitor {
		type Value = ResourceId;

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			formatter
				.write_str(&format!("a resource ID string following the form format_version{}size_in_bytes{}hash_of_resource ", SEP, SEP))
		}

		fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
		where
			E: de::Error,
		{
			ResourceId::parse(v).map_err(E::custom)
		}
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<ResourceId, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_string(ResourceIdStringVisitor {})
	}
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub enum ResourceKind {
	/// A "Manifest" is any kind of declarative config structure as a resource.
	/// TileDef, ArtDef, ModelDef, animations, and other such things go here.
	Manifest,
	/// Modules can run code and establish namespaces. As such, they act like
	/// a manifest in that they can have dependencies, but they're special.
	ModuleManifest,
	/// Plain Old Data is exactly what it sounds like. We have a reference to a
	/// file, the file gets loaded by the system as a buffer of bytes. For example,
	/// images, models, sound clips, common voxel models exported in some voxel library,
	/// that kind of thing.
	PlainOldData,
}

/// Used to keep track of a resource locally
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceInfo {
	/// Which resource?
	pub id: ResourceId,
	/// What did the "creator" user call this resource?
	pub filename: String,
	/// Which user claims to have "made" this resource? Who signed it, who is the authority on it?
	pub creator: NodeIdentity,
	/// What broad category of things does this resource fall into?
	pub kind: ResourceKind,
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
pub enum ResourceLoadError {
	#[error("While trying to retrieve resource {0:?}, a network error was encountered: {1}")]
	Network(ResourceId, String),
	#[error("Error loading resource {0:?} from disk: {1}")]
	Disk(ResourceId, String),
	#[error("Tried to access a resource {0:?}, which cannot be found (is not indexed) locally or on any connected server.")]
	NotFound(ResourceId),
	#[error("Timed out while attempting to fetch resource {0:?}.")]
	Timeout(ResourceId),
	#[error("Message-passing error while trying to load resource {0:?}: {1}.")]
	ChannelError(ResourceId, String),
}

impl Eq for ResourceInfo {}

#[derive(Debug)]
pub enum ResourceStatus<E> where E: Debug { 
	NotInitiated,
	Pending,
	Errored(E),
	Ready
}

impl<E> Clone for ResourceStatus<E> where E : Clone + Debug {
    fn clone(&self) -> Self {
        match self {
            Self::NotInitiated => Self::NotInitiated,
            Self::Pending => Self::Pending,
            Self::Errored(e) => Self::Errored(e.clone()),
            Self::Ready => Self::Ready
        }
    }
}

pub enum ResourcePoll<T, E> where T: Send, E: Debug {
	NotInitiated,
	Pending,
	///Encountered a problem while trying to load this resource.
	Errored(E),
	Ready(T)
}

impl<T, E> From<&ResourcePoll<T, E>> for ResourceStatus<E>
where
	T: Send,
{
	fn from(r: &ResourcePoll<T, E>) -> Self {
		match r {
			ResourcePoll::NotInitiated => ResourceStatus::NotInitiated,
			ResourcePoll::Pending => ResourceStatus::Pending,
			ResourcePoll::Errored(e) => ResourceStatus::Errored(e),
			ResourcePoll::Ready(_) => ResourceStatus::Ready
		}
	}
}

impl<T, E> From<Result<T, E>> for ResourcePoll<T, E> where T: Send, E: Debug {
	fn from(r: Result<T, E>) -> Self {
		match r {
			Ok(v) => Self::Ready(v),
			Err(e) => Self::Errored(e),
		}
	}
}
impl<T, E> From<Option<Result<T, E>>> for ResourcePoll<T, E>
where
	T: Send, E: Debug
{
	fn from(r: Option<Result<T, E>>) -> Self {
		match r {
			Some(Ok(v)) => Self::Ready(v),
			Some(Err(e)) => Self::Errored(e),
			None => Self::Pending,
		}
	}
}

impl<T, E> From<ResourcePoll<T, E>> for Option<Result<T, E>>
where
	T: Send, E: Debug,
{
	fn from(r: ResourcePoll<T, E>) -> Self {
		match r {
			ResourcePoll::Ready(v) => Some(Ok(v)),
			ResourcePoll::Errored(e) => Some(Err(e)),
			ResourcePoll::Pending => None,
			ResourcePoll::NotInitiated => None
		}
	}
}

pub struct ResourceLoadResult<T: Send + Sync, E: std::error::Error + Debug> {
	pub id: ResourceId, 
	pub result: ResourcePoll<T, E>,
}

pub(in self) fn resource_id_to_prefix(resource: &ResourceId) -> usize { 
	#[cfg(target_endian = "little")] { 
		resource.hash[31] as usize
	}

	#[cfg(target_endian = "big")] {
		resource.hash[0] as usize
	}
}
// Intended to be used as a const (global)
struct ResourceStorage<T: Send + Sized> {
	buckets: once_cell::sync::Lazy<[tokio::sync::RwLock<FastHashMap<ResourceId, T>>; 256]>,
}

impl<T> ResourceStorage<T> where T: Send + Sized + Clone { 
	pub const fn new() -> Self { 
		Self { 
			buckets: once_cell::sync::Lazy::new(|| { 
				std::array::from_fn(| _i | {
					tokio::sync::RwLock::new(new_fast_hash_map())
				})
			})
		}
	}
	pub async fn get(&self, id: &ResourceId) -> Option<T> { 
		let guard = self.buckets[resource_id_to_prefix(id)].read().await;
		guard.get(id).cloned()
	}
	pub fn get_blocking(&self, id: &ResourceId) -> Option<T> { 
		let guard = self.buckets[resource_id_to_prefix(id)].blocking_read();
		guard.get(id).cloned()
	}
	pub async fn insert(&self, id: ResourceId, value: T) -> Option<T> { 
		let mut guard = self.buckets[resource_id_to_prefix(&id)].write().await;
		guard.insert(id, value)
	}
	pub fn insert_blocking(&self, id: ResourceId, value: T) -> Option<T> { 
		let mut guard = self.buckets[resource_id_to_prefix(&id)].blocking_write();
		guard.insert(id, value)
	}
	pub async fn remove(&self, id: &ResourceId) -> Option<T> { 
		let mut guard = self.buckets[resource_id_to_prefix(&id)].write().await;
		guard.remove(&id)
	}
	pub fn remove_blocking(&self, id: &ResourceId) -> Option<T> { 
		let mut guard = self.buckets[resource_id_to_prefix(&id)].blocking_write();
		guard.remove(&id)
	}
	pub async fn update(&self, id: &ResourceId, new: T) {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].write().await;
		let reference = guard.get_mut(id);
		match reference { 
			Some(inner) => *inner = new,
			None => _ = guard.insert(id.clone(), new),
		}
	}
	pub fn update_blocking(&self, id: &ResourceId, new: T) {
		let mut guard = self.buckets[resource_id_to_prefix(&id)].blocking_write();
		let reference = guard.get_mut(id);
		match reference { 
			Some(inner) => *inner = new,
			None => _ = guard.insert(id.clone(), new),
		}
	}
}

#[derive(Debug)]
pub(in self) struct ResourceFetch {
	pub resources: Vec<ResourceId>,
	pub expected_source: NodeIdentity,
	/// If this field contains a Some value, this is treated as a resource to be loaded
	/// into memory, and then onto disk after that.
	/// If this field contains a None value, this is treated as a pre-load, and the resource
	/// is only saved to disk and not retained in memory.
	pub return_channel: Option<MpscSender<ResourceFetchResponse>>,
}

pub(in self) struct ResourceFetchResponse { 
	pub id: ResourceId,
	pub data: Result<Arc<Vec<u8>>, ResourceLoadError>,
}

global_channel!(MpscChannel, RESOURCE_FETCH, ResourceFetch, 65536);

impl Into<ResourceLoadResult<Arc<Vec<u8>>, ResourceLoadError>> for ResourceFetchResponse {
    fn into(self) -> ResourceLoadResult<Arc<Vec<u8>>, ResourceLoadError> {
        match self.data {
            Ok(val) => ResourceLoadResult { id: self.id, result: ResourcePoll::Ready(val) },
            Err(e) => ResourceLoadResult { id: self.id, result: ResourcePoll::Errored(e) },
        }
    }
}

static RESOURCE_METADATA: ResourceStorage<ResourceInfo> = ResourceStorage::new();

pub fn update_global_resource_metadata(id: &ResourceId, info: ResourceInfo) {
	RESOURCE_METADATA.update_blocking(id, info);
}

pub fn get_resource_metadata(id: &ResourceId) -> Option<ResourceInfo> {
	RESOURCE_METADATA.get_blocking(id)
}

#[derive(Clone)]
pub enum ResourceIdOrMeta {
	Id(ResourceId),
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

	let rid1 = ResourceId::from_buf(&buf1);
	let rid2 = ResourceId::from_buf(&buf2);

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

	let rid1 = ResourceId::from_buf(&buf1);

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