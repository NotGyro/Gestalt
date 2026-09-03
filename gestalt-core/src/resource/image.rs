use std::{collections::HashMap, io::Cursor, sync::Arc};

use image::{DynamicImage, ImageError, RgbaImage, ImageReader};
use log::error;

use crate::{MessageReceiverAsync, MessageSender, MpscReceiver, MpscSender};

use super::{
	channels::FetchResponse, retrieval::InternalTransfer, Caid, ResourceError, ResourceLocation, ResourceRetrievalError
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
	#[error("Channel for image load request can no longer be polled.")]
	ChannelDead,
	#[error("Unable to send requested image to the part of the program that needs it.")]
	SenderDead,
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

pub(super) async fn load_images(mut expected: Vec<ResourceLocation>, mut bytes_in: MpscReceiver<InternalTransfer>, images_out: MpscSender<FetchResponse<DynamicImage>>) -> Result<(), LoadImageError> {
	let mut in_progress = HashMap::new();
	for resource in expected.drain(..) {
		let buffer: Vec<u8> = Vec::new();
		in_progress.insert(resource, buffer);
	}
	while in_progress.len() > 0 {
		match bytes_in.recv_wait().await {
			Ok(msg) => {
				match msg {
					// Metadata, potentially useful for guessing at file type.
					InternalTransfer::Metadata(_resource_info) => todo!("Metadata not yet implemented"),
					// WIP buffer. Currently pretty useless as there's just about nothing
					// that works by streaming a single non-video image.
					InternalTransfer::Buffer(resource_location, mut buf) => {
						in_progress.entry(resource_location)
							.and_modify(|val| val.append(&mut buf))
							.or_insert(buf);
					},
					// Last buffer, actually process our image here.
					InternalTransfer::FinalBuffer(resource_location, buf_maybe) => {
						match buf_maybe { 
							Ok(mut final_buffer) => {
								let bytes = match in_progress.remove(&resource_location) {
									Some(mut value) => {
										value.append(&mut final_buffer);
										value
									},
									None => {
										final_buffer
									}
								};
								let reader = ImageReader::new(Cursor::new(bytes))
									.with_guessed_format()
									.expect("Cursor io should never fail");
								// Actually parse as an image
								let resl = reader.decode()
									.map(|image| Arc::new(image))
									.map_err(|e| ResourceRetrievalError::DecodeError(
										format!("Resource {resource_location:?} failed to decode due to {e}")
									));
								//if let Ok(image) = resl {
								// TODO: Interact with the global here.
								//}
								images_out.send(FetchResponse { id: resource_location, resource: resl })
									.map_err(|_e| LoadImageError::SenderDead)?;
							}
							// We have been sent an error from the other end.
							Err(e) => {
								in_progress.remove(&resource_location);
								error!("Failed to retrieve {resource_location:?} due to {e}");
								images_out.send(FetchResponse { id: resource_location, resource: Err(e) })
									.map_err(|_e| LoadImageError::SenderDead)?;
							}
						}
					},
				}
			},
			Err(e) => { 
				error!("Channel for image load request can no longer be polled due to {e}, cannot load following resources: {expected:#?}");
				return Err(LoadImageError::ChannelDead);
			}
		}
	}
	Ok(())
}