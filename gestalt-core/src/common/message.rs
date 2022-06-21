use std::collections::VecDeque;
use std::error::Error;
use std::fmt::Debug;
use std::result::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use hashbrown::{HashMap, HashSet};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::script::ModuleId;
use crate::world::WorldId;
use gestalt_names::gestalt_atom::GestaltAtom;

/// Runtime type identifier for a type of message.
#[derive(Clone, Debug, PartialEq)]
pub enum MsgTypeId { 
    BuiltIn(std::any::TypeId),
    Dynamic(GestaltAtom),
}
pub type ChannelId = Uuid;

/// A message in the form it exists as on the bus. 
#[derive(Clone, Debug, PartialEq)]
pub struct EncodedMessage {
    //Message type
    pub ty: MsgTypeId,
    //Message data
    pub data: Vec<u8>,
}

pub type RawMessageSender = tokio::sync::broadcast::Sender<EncodedMessage>; 
pub type RawMessageReceiver = tokio::sync::broadcast::Receiver<EncodedMessage>;

pub fn create_channel(capacity: usize) -> (RawMessageSender, RawMessageReceiver) { 
    tokio::sync::broadcast::channel(capacity)
}

pub trait Message : Clone + Debug + Send + Sync + Serialize + DeserializeOwned {

}

pub enum ChannelDomain {
    Global,
    World(WorldId),
    //Entity(WorldId, EntityId),
    //Module(ModuleId),
    //WorldModule(WorldId, ModuleId),
}

/// Used for typesafe wrappers over the underlying runtime-typed channel object. This is for the Sender<T> end. 
pub trait ChannelAccepts<T> {

}

pub struct MessageBus { 

}