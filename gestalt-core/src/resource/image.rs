use std::sync::Arc;

use gestalt_names::gestalt_atom::GestaltAtom;
use image::{ImageError, RgbaImage};

use crate::common::identity::NodeIdentity;

use super::{ResourceId, ResourceLoadError, ResourceProvider, RawResourceProvider, ResourcePoll, ResourceResult, ArchiveFileIndex};

pub const ID_MISSING_TEXTURE: ResourceId = ResourceId {
	version: 0,
	length: 0,
	hash: [1; 32],
};
pub const ID_PENDING_TEXTURE: ResourceId = ResourceId {
	version: 0,
	length: 0,
	hash: [2; 32],
};

pub const ID_ERROR_TEXTURE: ResourceId = ResourceId {
	version: 0,
	length: 0,
	hash: [3; 32],
};

#[derive(thiserror::Error, Debug)]
pub enum LoadImageError {
	#[error("Error while retrieving an image: {0:?}")]
	Retrieval(#[from] ResourceLoadError),
	#[error("Error while decoding or transcoding an image: {0:?}")]
	EncodeDecodeError(#[from] ImageError),
	#[error("Tried to access a image named {0}, which does not appear to exist.")]
	DoesNotExist(String),
}

pub type InternalImage = RgbaImage;

pub struct ImageCoord { 
	pub x: u32, 
	pub y: u32,
}

pub struct ImageRect { 
	/// Upper-left, positive Y is down and positive X is right. Image-space coordinates.
	pub origin: ImageCoord,
	pub width: u32,
	pub height: u32,
}

/// Which part of the underlying ResourceId are we seeking to use for our texture?
/// For use as the T in SubResource<T>
pub enum SubImageIndex {
	/// A portion of an image file
	Section(ImageRect),
	ArchiveFile(ArchiveFileIndex),
	/// A portion of an image file in an archive.
	ArchiveFileSection(ArchiveFileIndex, ImageRect),
}
pub struct ImageProvider { 
	inner: RawResourceProvider,
}

impl ImageProvider { 
	pub fn new(return_channel_capacity: usize) -> Self {
		Self {
			inner: RawResourceProvider::new(return_channel_capacity)
		}
	}

	async fn recv_wait_inner(&mut self) -> ResourcePoll<InternalImage, LoadImageError> { 
		match self.inner.recv_wait().await {
			ResourcePoll::Ready(id, buf) => {
				match image::load_from_memory(buf.as_slice()) {
					Ok(image) => ResourcePoll::Ready(id, image.into_rgba8()),
					Err(e) => ResourcePoll::ResourceError(id, e.into()),
				}
			},
			ResourcePoll::ChannelError(e) => ResourcePoll::ChannelError(e),
			ResourcePoll::RetrievalError(e) => ResourcePoll::RetrievalError(e),
			ResourcePoll::ResourceError(id, e) => ResourcePoll::RetrievalError(e),
			ResourcePoll::None => ResourcePoll::None,
		}
	}
}

impl ResourceProvider<InternalImage> for ImageProvider {
    type Error = LoadImageError;

    fn request_batch(&mut self, resources: Vec<ResourceId>, expected_source: NodeIdentity) -> super::ResourceResult<InternalImage, Self::Error> {
        match self.inner.request_batch(resources, expected_source) {
            ResourceResult::NotInitiated => todo!(),
            ResourceResult::Pending => todo!(),
            ResourceResult::Err(e) => ResourceResult::Err(LoadImageError::Retrieval(e)),
            ResourceResult::Ready(buf) => todo!(),
        }
    }

    fn preload_batch(&mut self, resources: Vec<ResourceId>, expected_source: NodeIdentity) {
        self.inner.preload_batch(resources, expected_source)
    }

    fn recv_poll(&mut self) -> super::ResourcePoll<InternalImage, Self::Error> {
        match self.inner.recv_poll() {
            ResourcePoll::Ready(id, buf) => {
				match image::load_from_memory(buf.as_slice()) {
					Ok(image) => ResourcePoll::Ready(id, image.into_rgba8()),
					Err(e) => ResourcePoll::ResourceError(id, e.into()),
				}
			},
            ResourcePoll::ChannelError(_) => todo!(),
            ResourcePoll::RetrievalError(_) => todo!(),
            ResourcePoll::ResourceError(_, _) => todo!(),
            ResourcePoll::None => todo!(),
        }
    }

    fn recv_wait(&mut self) -> impl futures::Future<Output = super::ResourcePoll<InternalImage, Self::Error>> + '_ {
        self.recv_wait_inner()
    }
}