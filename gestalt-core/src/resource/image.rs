use futures::Future;
use image::{ImageError, RgbaImage};

use crate::common::identity::NodeIdentity;

use super::{
	provider::{RawResourceProvider, ResourceProvider},
	ResourceError, Caid, ResourceId, ResourcePoll, ResourceRetrievalError,
};

pub const ID_MISSING_TEXTURE: Caid = Caid {
	version: 0,
	length: 0,
	hash: [1; 32],
};
pub const ID_PENDING_TEXTURE: Caid = Caid {
	version: 0,
	length: 0,
	hash: [2; 32],
};

pub const ID_ERROR_TEXTURE: Caid = Caid {
	version: 0,
	length: 0,
	hash: [3; 32],
};

#[derive(thiserror::Error, Debug)]
pub enum LoadImageError {
	#[error("Error while decoding or transcoding an image: {0:?}")]
	EncodeDecodeError(#[from] ImageError),
	#[error("Tried to access a image named {0}, which does not appear to exist.")]
	DoesNotExist(String),
}

impl From<ResourceError<ResourceRetrievalError>> for ResourceError<LoadImageError> {
	fn from(value: ResourceError<ResourceRetrievalError>) -> Self {
		match value {
			ResourceError::Channel(e) => Self::Channel(e),
			ResourceError::Retrieval(e) => Self::Retrieval(e),
			ResourceError::Parse(_, e) => Self::Retrieval(e),
		}
	}
}

pub type InternalImage = RgbaImage;

pub struct ImageProvider {
	inner: RawResourceProvider,
}

impl ImageProvider {
	pub fn new(return_channel_capacity: usize) -> Self {
		Self {
			inner: RawResourceProvider::new(return_channel_capacity),
		}
	}

	async fn recv_wait_inner(
		&mut self,
	) -> Result<(ResourceId, InternalImage), ResourceError<LoadImageError>> {
		match self.inner.recv_wait().await {
			Ok((id, buf)) => match image::load_from_memory(buf.as_slice()) {
				Ok(image) => Ok((id, image.into_rgba8())),
				Err(e) => Err(ResourceError::Parse(id, e.into())),
			},
			Err(e) => Err(e.into()),
		}
	}
}

impl ResourceProvider<InternalImage> for ImageProvider {
	type ParseError = LoadImageError;

	/// Returns the subset of these resources that are ready *now.*
	/// If it returns an empty vec, that means all resources are pending.
	fn request_batch(
		&mut self,
		resources: Vec<ResourceId>,
		expected_source: NodeIdentity,
	) -> Vec<Result<(ResourceId, InternalImage), ResourceError<LoadImageError>>> {
		self.inner
			.request_batch(resources, expected_source)
			.iter()
			.map(|value| match value {
				Ok(buf) => todo!(),
				Err(_) => todo!(),
			})
			.collect()
	}
	/// Request that we download files, except that there isn't any immediate need to use them
	/// (i.e. retrieve the files but do not send them along a channel to this ResourceProvider)
	fn preload_batch(&mut self, resources: Vec<ResourceId>, expected_source: NodeIdentity) {
		self.inner.preload_batch(resources, expected_source)
	}

	fn recv_poll(&mut self) -> ResourcePoll<InternalImage, Self::ParseError> {
		match self.inner.recv_poll() {
			ResourcePoll::Ready(id, buf) => match image::load_from_memory(buf.as_slice()) {
				Ok(image) => ResourcePoll::Ready(id, image.into_rgba8()),
				Err(e) => ResourcePoll::Err(super::ResourceError::Parse(id, e.into())),
			},
			ResourcePoll::Err(e) => ResourcePoll::Err(e.into()),
			ResourcePoll::None => ResourcePoll::None,
		}
	}

	fn recv_wait(
		&mut self,
	) -> impl Future<Output = Result<(ResourceId, InternalImage), ResourceError<Self::ParseError>>> + '_
	{
		self.recv_wait_inner()
	}
}
