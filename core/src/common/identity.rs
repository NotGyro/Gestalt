use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct NodeIdentity(/* TODO, will be public key (for verifying signatures) */);

pub type Signature = ();
