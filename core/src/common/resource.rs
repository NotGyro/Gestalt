use super::identity::NodeId;
use std::{cmp::PartialEq, hash::Hash};

/// 32-byte Sha256 hash
pub type ResourceHash = [u8; 32];

#[derive(Clone, Debug)]
pub struct ResourceAddress {
    /// Which version of the ResourceAddress struct is this?
    pub version: u8,
    /// The author who issued this
    pub origin: NodeId,
    pub hash: ResourceHash,
    pub name: String,
}

impl Hash for ResourceAddress {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.origin.hash(state);
        self.hash.hash(state);

        //Ignored:
        //self.version.hash(state);
        //self.name.hash(state);
    }
}

impl PartialEq for ResourceAddress {
    fn eq(&self, other: &Self) -> bool {
        // Elide name.
        // TODO: Figure out how to compare two RA's of different origins
        self.origin == other.origin && self.hash == other.hash
        //The naive form of this would be self.version == other.version && self.origin == other.origin && self.hash == other.hash && self.name == other.name
        //but we want equality to be entirely based on origin and hash.
    }
}

impl Eq for ResourceAddress {}
