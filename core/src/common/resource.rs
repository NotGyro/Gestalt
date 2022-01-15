use super::identity::{NodeIdentity, Signature};

use serde::{Serialize, Deserialize};
use std::{cmp::PartialEq, hash::Hash};
use sha2::Digest;

pub const CURRENT_RESOURCE_ID_FORMAT: u8 = 1;

/// Content-addressed identifier for a Gestalt resource. 
/// String representation starts with a version number for the
/// ResourceId structure, then a `:` delimeter, then the size (number of bytes)
/// in the resource, then the 32-byte Sha256-512 hash encoded in base-64.
/// For example, `1:2048:J1kVZSSu8LHZzw25mTnV5lhQ8Zqt9qU6V1twg5lq2e6NzoUA` would be a version 1 ResourceID.
#[repr(C)]
#[derive(Copy, Clone, PartialOrd, Serialize, Deserialize)]
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
    #[error("tried to parse {0} into a ResourceId but it contained no ':' separator.")]
    NoSeparator(String),
    #[error("tried to parse {0} into a ResourceId but was a non-2 number ':' separators")]
    TooManySeparators(String),
    #[error("string `{0}` is not a valid resource ID because it contains whitespace")]
    ContainsWhitespace(String),
    #[error("couldn't parse a ResourceId: base64 error {0:?}")]
    Base64Parse(#[from] base64::DecodeError),
    #[error("expected to parse a 32-byte hash out of the base64 in {0} but the byte buffer we got was {1} bytes in length")]
    BufferWrongSize(String, usize),
    #[error("could not parse {0} as a resource ID, version (1st field) number needs to be parseable as an integer")]
    VersionNotNumber(String),
    #[error("could not parse {0} as a resource ID, length (2nd field, byte-size) needs to be parseable as an integer")]
    SizeNotNumber(String),
    #[error("could not parse {0} as a resource ID, did not recognize ResourceId format {1}. Most likely this was sent by a newer version of the Gestalt Engine")]
    UnrecognizedVersion(String, u8),
}

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
            return Err(VerifyResourceError::WrongLength( self.length, buf.len() as u64 ));
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
        if !value.contains(':') {
            return Err(ParseResourceIdError::NoSeparator(value.to_string()));
        }

        let fields: Vec<&str> = value.split(':').collect();
        if fields.len() != 3 { 
            return Err(ParseResourceIdError::TooManySeparators(value.to_string()));
        }

        let version = u8::from_str_radix(*fields.get(0).unwrap(), 10)
            .map_err(|_| ParseResourceIdError::VersionNotNumber(value.to_string()))?;  
        if version != CURRENT_RESOURCE_ID_FORMAT { 
            return Err(ParseResourceIdError::UnrecognizedVersion(value.to_string(), version));
        }

        let length = u64::from_str_radix(*fields.get(1).unwrap(), 10)
            .map_err(|_| ParseResourceIdError::VersionNotNumber(value.to_string()))?;  

        let bytes = base64::decode(fields.get(2).unwrap() )?;
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

impl Hash for ResourceId {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.length.hash(state);
        self.hash.hash(state);
    }
}

impl PartialEq for ResourceId {
    fn eq(&self, other: &Self) -> bool {
        //Ignore version here
        // TODO: Figure out how to compare two RId's of different origin
        (self.length == other.length) && (self.hash == other.hash)
    }
}

impl Eq for ResourceId {}

impl std::fmt::Display for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.version, self.length, base64::encode(&self.hash))
    }
}

impl std::fmt::Debug for ResourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ResourceId:({})", self.to_string())
    }
}

// For use with serde
pub mod resourceid_base64_string { 
    use std::fmt;
    use serde::{Serializer, Deserializer, de::{Visitor, self}};

    use super::*;

    pub fn serialize<S>(val: &ResourceId, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        serializer.serialize_str(val.to_string().as_str())
    }

    struct ResourceIdVisitor;

    impl<'de> Visitor<'de> for ResourceIdVisitor {
        type Value = ResourceId;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a resource ID string following the form format_version:size_in_bytes:hash_of_resource ")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where E: de::Error, {
            ResourceId::parse(v).map_err(E::custom)
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ResourceId, D::Error> where D: Deserializer<'de> {
        deserializer.deserialize_string(ResourceIdVisitor{})
    }
}

/// Used to keep track of a resource locally
#[derive(Clone, Debug, Serialize, Deserialize, PartialOrd)]
pub struct ResourceDescriptor {
    /// Which resource?
    #[serde(with = "crate::common::resource::resourceid_base64_string")]
    pub id: ResourceId,
    /// Which user did we get this resource from? Who signed it, who is the authority on it?
    pub origin: NodeIdentity,
    /// What did the "origin" user call this resource?
    pub name: String,
    /// Name of origin user and friends who made this resource.
    pub authors: String,
    /// Signature verifying our binary blob as good, signed with the public key from NodeIdentity.
    pub signature: Signature,
}

impl Hash for ResourceDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.origin.hash(state);
    }
}

impl PartialEq for ResourceDescriptor {
    fn eq(&self, other: &Self) -> bool {
        //Elide name.
        self.origin == other.origin && self.id == other.id
        //The naive form of this would be self.id == other.id && self.origin == other.origin && self.name == other.name
        //but we want equality to be entirely based on origin and hash.
    }
}

impl Eq for ResourceDescriptor {}

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
    
    let rid1= ResourceId::from_buf(&buf1);
    let rid2= ResourceId::from_buf(&buf2);

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

    const BUF_SIZE:usize = 2048;

    let mut buf1: [u8; BUF_SIZE] = [0; BUF_SIZE]; 

    {
        rng.fill(&mut buf1);
    }
    drop(rng);
    
    let rid1= ResourceId::from_buf(&buf1);

    let stringified = rid1.to_string();

    let b64hash = base64::encode(&rid1.hash);

    //Our hash should be in here
    assert!(stringified.contains(&b64hash));

    let format_string = format!("{}", CURRENT_RESOURCE_ID_FORMAT);
    assert!(stringified.starts_with(&format_string));

    let after_split: Vec<&str> = stringified.split(":").collect();

    assert_eq!(after_split.len(), 3); 
    assert_eq!(u64::from_str_radix(after_split.get(1).unwrap(), 10).unwrap(), BUF_SIZE as u64);
}