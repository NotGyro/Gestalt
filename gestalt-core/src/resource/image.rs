use std::error::Error;

use image::{ImageError, RgbaImage};

use super::{ResourceId, ResourceInfo, ResourceStatus};

pub const ID_MISSING_TEXTURE: ResourceId = ResourceId {
	version: 0,
	length: 0,
	hash: [0; 32],
};
pub const ID_PENDING_TEXTURE: ResourceId = ResourceId {
	version: 0,
	length: 0,
	hash: [1; 32],
};

#[derive(thiserror::Error, Debug)]
pub enum RetrieveImageError {
	#[error("While trying to retrieve a image, a network error was encountered: {0:?}")]
	Network(Box<dyn Error>),
	#[error("Error loading image from disk: {0:?}")]
	Disk(#[from] std::io::Error),
	#[error("Error while decoding or transcoding an image: {0:?}")]
	EncodeDecodeError(#[from] ImageError),
	#[error("Tried to access a image named {0}, which does not appear to exist.")]
	DoesNotExist(String),
}

pub type InternalImage = RgbaImage;

/// Anything from which we can load images.
pub trait ImageProvider {
	fn load_image(&mut self, image: &ResourceId) -> ResourceStatus<&InternalImage, RetrieveImageError>;
	fn get_metadata(&self, image: &ResourceId) -> Option<&ResourceInfo>;
}
