use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Hash)]
pub struct NodeId(Uuid);
