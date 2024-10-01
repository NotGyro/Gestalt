//! Core traits for a structure for loading resources in (mostly) synchronous code,
//! and an raw-bytes implementation of it for use as an `inner` in type-specific implementations.

use futures::Future;
use log::error;

use crate::{
	common::identity::NodeIdentity,
	message::{
		MessageReceiver, MessageReceiverAsync, MpscChannel, MpscReceiver, MpscSender, SenderSubscribe,
	},
};

use super::{
	channels::RESOURCE_FETCH,
	retrieval::{ResourceFetch, ResourceFetchResponse},
	ResourceError, ResourceLocation, ResourcePoll, ResourceRetrievalError,
};
use std::{fmt::Debug, sync::Arc};

pub trait ResourceProvider<T> {
	type ParseError: Debug;

	/// Returns the subset of these resources that are ready *now.*
	/// If it returns an empty vec, that means all resources are pending.
	fn request_batch(
		&mut self,
		resources: Vec<ResourceLocation>,
		expected_source: NodeIdentity,
	) -> Vec<Result<(ResourceLocation, T), ResourceError<Self::ParseError>>>;
	fn request_one(
		&mut self,
		resource: ResourceLocation,
		expected_source: NodeIdentity,
	) -> Option<Result<T, ResourceError<Self::ParseError>>> {
		self.request_batch(vec![resource], expected_source)
			.pop()
			.map(|value| value.map(|result| result.1))
	}

	/// Request that we download files, except that there isn't any immediate need to use them
	/// (i.e. retrieve the files but do not send them along a channel to this ResourceProvider)
	fn preload_batch(&mut self, resources: Vec<ResourceLocation>, expected_source: NodeIdentity);
	fn preload_one(&mut self, resource: ResourceLocation, expected_source: NodeIdentity) {
		self.preload_batch(vec![resource], expected_source)
	}

	fn recv_poll(&mut self) -> ResourcePoll<T, Self::ParseError>;
	fn recv_wait(
		&mut self,
	) -> impl Future<Output = Result<(ResourceLocation, T), ResourceError<Self::ParseError>>> + '_;

	/// Poll until there are no remaining results.
	fn recv_poll_all(&mut self) -> Vec<ResourcePoll<T, Self::ParseError>> {
		let mut next = self.recv_poll();
		let mut buf = vec![];
		while !next.is_none() {
			match next {
				ResourcePoll::Ready(id, val) => buf.push(ResourcePoll::Ready(id, val)),
				ResourcePoll::Err(e) => {
					if let ResourceError::Channel(recv_err) = e {
						// Return early - we won't be getting any more results out of this one.
						buf.push(ResourcePoll::Err(ResourceError::Channel(recv_err)));
						return buf;
					} else {
						buf.push(ResourcePoll::Err(e));
					}
				}
				ResourcePoll::None => {
					unreachable!("Unreachable due to \"while next != ResourcePoll::None\" above.")
				}
			}
			next = self.recv_poll(); // Set up next iteration of the loop.
		}
		return buf;
	}
}

pub struct RawResourceProvider {
	fetch_sender: MpscSender<ResourceFetch>,
	return_receiver: MpscReceiver<ResourceFetchResponse>,
	return_template: MpscSender<ResourceFetchResponse>,
}
impl RawResourceProvider {
	pub fn new(return_channel_capacity: usize) -> Self {
		let return_channel = MpscChannel::new(return_channel_capacity);
		Self {
			fetch_sender: RESOURCE_FETCH.sender_subscribe(),
			return_receiver: return_channel.take_receiver().unwrap(),
			return_template: return_channel.sender_subscribe(),
		}
	}

	fn request_inner(
		&self,
		resources: Vec<ResourceLocation>,
		expected_source: NodeIdentity,
		return_channel: Option<MpscSender<ResourceFetchResponse>>,
	) -> Vec<Result<(ResourceLocation, Arc<Vec<u8>>), ResourceError<ResourceRetrievalError>>> {
		let resl = self.fetch_sender.blocking_send(ResourceFetch {
			resources: resources
				.iter()
				.map(|value| match value {
						ResourceLocation::Caid(_) => todo!(),
						ResourceLocation::Local(_) => todo!(),
						ResourceLocation::Link(_) => todo!(),
					})
				.collect(),
			expected_source,
			return_channel,
		});
		if let Err(e) = resl { 
			error!("Unable to fulfil resource requests: Send erorr {e:?}");
		}
		vec![]
	}

	async fn recv_wait_inner(
		&mut self,
	) -> Result<(ResourceLocation, Arc<Vec<u8>>), ResourceError<ResourceRetrievalError>> {
		match self.return_receiver.recv_wait().await {
			Ok(value) => {
				match value.data {
					// This will need to change when archives are implemented
					Ok(v) => Ok((value.id, v)),
					Err(e) => Err(ResourceError::Retrieval(e)),
				}
			}
			Err(e) => Err(ResourceError::Channel(e)),
		}
	}
}

impl ResourceProvider<Arc<Vec<u8>>> for RawResourceProvider {
	type ParseError = ResourceRetrievalError;

	//God I hate this return type signature, I should probably simplify it somehow.
	fn request_batch(
		&mut self,
		resources: Vec<ResourceLocation>,
		expected_source: NodeIdentity,
	) -> Vec<Result<(ResourceLocation, Arc<Vec<u8>>), ResourceError<ResourceRetrievalError>>> {
		self.request_inner(resources, expected_source, Some(self.return_template.clone()))
	}

	fn preload_batch(&mut self, resources: Vec<ResourceLocation>, expected_source: NodeIdentity) {
		self.request_inner(resources, expected_source, None);
	}

	fn recv_poll(&mut self) -> ResourcePoll<Arc<Vec<u8>>, Self::ParseError> {
		match self.return_receiver.recv_poll() {
			Ok(Some(v)) => match v.data {
				Ok(value) => ResourcePoll::Ready(v.id, value),
				Err(e) => ResourcePoll::Err(ResourceError::Retrieval(e)),
			},
			Ok(None) => ResourcePoll::None,
			Err(e) => ResourcePoll::Err(ResourceError::Channel(e)),
		}
	}

	fn recv_wait(
		&mut self,
	) -> impl Future<
		Output = Result<(ResourceLocation, Arc<Vec<u8>>), ResourceError<ResourceRetrievalError>>,
	> + '_ {
		self.recv_wait_inner()
	}
}
