use serde::{Serialize, Deserialize};
use uuid::Uuid;
use ustr::*;
//use super::message::EventBus;

use crate::common::network::Identity;

/// A Resource is identified by the Blake3 Hash of its contents / file.
pub type ResourceId = [u8; 32];

/// Resource link IDs are univerally unique. Resource link names can be duplicate, however.
pub type ResourceLinkId = Uuid;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum ResourceDependency {
    Direct(ResourceId),
    Link(ResourceLinkId),
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourceDescriptor {
    pub id: ResourceId,
    pub resource_type: Ustr,
    pub author: Identity,
    pub deps: Vec<ResourceDependency>,
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub enum ResourceLinkTarget {
    File((String, ResourceId)),
    Directory((String,Vec<ResourceLinkId>)),
    Alias(ResourceLinkId),
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
/// Nodes in a directory / file structure naming and organizing resources.
pub struct ResourceLink {
    pub id: ResourceLinkId,
    pub revision: u64,
    pub target: ResourceLinkTarget,
}
/*
pub struct ResourceSystem { 
    //Yes, I know this technically means we're hashing it twice. To be optimized later, I suppose.
    resources: HashMap<ResourceId, 
}*/