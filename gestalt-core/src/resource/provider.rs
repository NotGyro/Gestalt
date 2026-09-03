//! Core traits for a structure for loading resources in (mostly) synchronous code,
//! and an raw-bytes implementation of it for use as an `inner` in type-specific implementations.

use futures::Future;
use log::error;

use crate::{
	common::identity::NodeIdentity,
	message::{
		MessageReceiver, MessageReceiverAsync, MpscChannel, MpscReceiver, MpscSender, SenderSubscribe,
	}, MessageSender,
};

use super::{
	channels::FetchResponse, retrieval::ResourceFetch, ResourceError, ResourceLocation, ResourcePoll, ResourceRetrievalError
};
use std::{collections::HashMap, fmt::Debug, sync::Arc};

pub struct LoadedResource<T> { 
	pub id: ResourceLocation,
	pub resource: Arc<T>,
}

pub trait ResourceProvider<T> {
	type ParseError: Debug;

	/// Returns cached resources immediately and begins retrieval for all other resources.
	fn request_batch(&mut self, request: Vec<ResourceLocation>, expected_source: &NodeIdentity) -> Result<Vec<LoadedResource<T>>, Self::ParseError>;
    /// Returns cached resource immediately or begins retrieval if it is not cached.
	fn request_one(&mut self, request: ResourceLocation, expected_source: &NodeIdentity) -> Result<Option<LoadedResource<T>>, Self::ParseError> {
		self.request_batch(vec![request], expected_source).map(|mut v| v.drain(..).next())
	}
 
	/// Request that we download files, except that there isn't any immediate need to use them
	/// (i.e. retrieve the files but do not send them along a channel to this ResourceProvider)
	fn preload_batch(&mut self, resources: Vec<ResourceLocation>, expected_source: &NodeIdentity) -> Result<(), Self::ParseError>;
	fn preload_one(&mut self, resource: ResourceLocation, expected_source: &NodeIdentity) -> Result<(), Self::ParseError> {
		self.preload_batch(vec![resource], expected_source)
	}
}

pub struct ResourceCache<T> {
	resources: HashMap<ResourceLocation, T>,
}