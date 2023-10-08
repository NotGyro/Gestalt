use image::{ImageError, RgbaImage};

use super::{ResourceId, ResourceLoadError};

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

pub const ID_ERROR_TEXTURE: ResourceId = ResourceId {
	version: 0,
	length: 0,
	hash: [2; 32],
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

