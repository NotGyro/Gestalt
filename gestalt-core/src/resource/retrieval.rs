// ! Async-sided resource system code, primarily consisting of the "actually go grab the file"
// ! logic of querying the disk cache and then, if no cached file is found, attempting to fetch
// ! it from a presently-connected server.

use std::{fmt::Debug, path::PathBuf, sync::Arc};
use std::io::Error as IoError;

use futures::TryFutureExt;
use log::{error, info, trace, warn};
use tokio::io::AsyncReadExt;

use crate::resource::image::load_images;
use crate::{
	common::{
		directories::GestaltDirectories,
		identity::{IdentityKeyPair, NodeIdentity, PublicKey},
	}, message::{MessageReceiverAsync, MpscReceiver, MpscSender, QuitReceiver}, net::SelfNetworkRole, resource::ResourceLocation, BuildSubset, MessageSender, MpscChannel, RecvError, SenderSubscribe, SubsetBuilder
};

use super::path_for_resource;
use super::{channels::{FetchResponse, ResourceSysChannels, RetrieverChannels}, resource_id_to_prefix, Caid, ResourceFilelike, ResourceInfo, ResourceRetrievalError};

static LOCK_SUFFIX: &'static str = ".lock";

#[derive(thiserror::Error, Debug, Clone)]
pub enum ResourceSysError {
	#[error(
		"Could not launch resource system - \
        Resource fetch request channel has already been claimed. \
        It is possible launch_resource_system() has been invoked twice."
	)]
	NoFetchReceiver,
	#[error(
		"A resource retrieval channel has been closed: {0}"
	)]
	ChannelClosed(#[from] RecvError),
	#[error("Could not retrieve a resource: {0}")]
	RetrievalError(#[from] ResourceRetrievalError),
}

pub struct ResourceFetch<T> where T: Debug {
	pub resources: Vec<ResourceLocation>,
	pub expected_source: NodeIdentity,
	/// Channel to send the loaded bytes back to. 
	/// 
	/// If this field is Some this is treated as a resource to be loaded
	/// and sent to this channel, and then loaded onto disk after that.
	/// If this field is true, this is treated as a pre-load, and the resource
	/// is only saved to disk and not retained in memory.
	pub return_channel: Option<MpscSender<FetchResponse<T>>>,
}

impl<T> Debug for ResourceFetch<T> where T: Debug {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("ResourceFetch").field("resources", &self.resources).field("expected_source", &self.expected_source).finish()
	}
}

pub struct ResourceSystemConfig {
	/// Size of file-chunks to read off of the disk and send off to the loader.
	/// Too low and it'll incur significant message-passing overhead, too high and
	/// time that could have been spent processing the stream will be spent waiting
	/// for I/O
	pub file_buffer_size: usize,
}

/// Initializes the asynchronous end (i.e. most of it) of the resource-loading system.
pub async fn launch_resource_system(
	role: SelfNetworkRole,
	self_identity: IdentityKeyPair,
	directories: Arc<GestaltDirectories>,
	channels: ResourceSysChannels,
	conf: Arc<ResourceSystemConfig>,
) -> Result<(), ResourceSysError> {
	resource_system_main(role, self_identity, channels, directories, conf).await?;
	Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum FileLoadError {
	#[error("File i/o error: {0}")]
	DiskFile(#[from] IoError),
	#[error("Could not send a file on a channel for resource {0:?}")]
	NoSendChannel(ResourceLocation),
}

#[derive(Debug)]
pub enum InternalTransfer {
	Metadata(Box<ResourceInfo>),
	Buffer(ResourceLocation, Vec<u8>),
	FinalBuffer(ResourceLocation, Result<Vec<u8>, ResourceRetrievalError>),
}

type ResourcesOnDisk = Vec<(ResourceLocation, PathBuf)>;

struct FilePresenceReport {
	on_disk: ResourcesOnDisk, 
	not_on_disk: Vec<ResourceLocation>,
}

/// Checks which files are present on disk
async fn are_files_cached(
	mut resources: Vec<ResourceLocation>,
	expected_source: NodeIdentity,
	self_identity: NodeIdentity,
	directories: Arc<GestaltDirectories>,
	conf: Arc<ResourceSystemConfig>,
) -> FilePresenceReport {
	let mut on_disk = Vec::new();
	let mut not_on_disk = Vec::new();
	for resource in resources.drain(..) {
		let maybe_file_path =
			path_for_resource(&resource, &expected_source, &self_identity, directories.clone());
		match maybe_file_path {
			ResourceFilelike::File(path) => {
				match tokio::fs::try_exists(path.clone()).await { 
					Ok(is_on_disk) => {
						if is_on_disk {
							on_disk.push((resource, path));
						}
						else { 
							not_on_disk.push(resource);
						}
					}
					Err(e) => {
						error!("Unable to check for cached resource {resource:?} as file {path:?}: {e}, skipping.");
					}
				}
			},
			ResourceFilelike::Internal(_) => todo!("Internal resources routing through the resource system is not yet implemented"),
		}
	}
	FilePresenceReport {
		on_disk,
		not_on_disk
	}
}

async fn load_from_files(
	mut resources: ResourcesOnDisk,
	channel: MpscSender<InternalTransfer>,
	conf: Arc<ResourceSystemConfig>,
) -> Result<(), FileLoadError> {
	for (resource, path) in resources.drain(..) {
		match tokio::fs::OpenOptions::new()
			.read(true)
			.open(path.clone())
			.await
		{
			// TODO - stream so it can decode while I/O is busy. use conf.file_buffer_size
			Ok(mut file) => {
				let mut buffer = Vec::new();
				if let Err(e) = file.read_to_end(&mut buffer).await {
					error!(
						"Error when attempting to read file {0:?} into memory: {1}",
						path, e
					);
					channel.send(InternalTransfer::FinalBuffer(resource.clone(), Err(ResourceRetrievalError::Disk(resource.clone(), format!("{}", e)))))
						.map_err(|_e| FileLoadError::NoSendChannel(resource.clone()))?;
				} else {
					channel.send(InternalTransfer::FinalBuffer(resource.clone(), Ok(buffer))).map_err(|_e| FileLoadError::NoSendChannel(resource.clone()))?;
				}
			}
			Err(e) => match e.kind() {
				_ => { 
					error!(
						"Failed to load file {0:?} at location {1:?} due to error {2:?}.",
						&resource, &path, e
					);
					return Err(e.into());
				},
			},
		}
	}
	Ok(())
}

/// Mainloop for the resource-loading system.
async fn resource_system_main(
	role: SelfNetworkRole,
	self_identity: IdentityKeyPair,
	channels: ResourceSysChannels,
	directories: Arc<GestaltDirectories>,
	conf: Arc<ResourceSystemConfig>,
) -> Result<(), ResourceSysError> {
	let request_receivers: RetrieverChannels = channels.build_subset(SubsetBuilder::new({}))
		.map_err(|_e| ResourceSysError::NoFetchReceiver)?;

	resource_retriever_main(role, self_identity, request_receivers, directories, conf).await?; 
	Ok(())
}

/// Non-preload - fetch from disk, or fetch over net and then cache.
async fn full_load_bytes(
	role: SelfNetworkRole, 
	self_identity: PublicKey,
	directories: Arc<GestaltDirectories>,
	resources: Vec<ResourceLocation>,
	expected_source: NodeIdentity, 
	internal_sender: MpscSender<InternalTransfer>,
	conf: Arc<ResourceSystemConfig>,
) -> Result<(), ResourceSysError> {
	let FilePresenceReport { on_disk, not_on_disk } = are_files_cached(
			resources,
			expected_source.clone(),
			self_identity.clone(),
			directories.clone(),
			conf.clone()
		).await;
	// Load what files we can from disk. 
	tokio::spawn(load_from_files(on_disk, internal_sender.sender_subscribe(), conf));
	if !not_on_disk.is_empty() {
		warn!("Network retrieval not yet implemented, cannot get: \n{not_on_disk:#?}");
	}
	Ok(())
}

async fn resource_retriever_main(
	role: SelfNetworkRole, 
	self_identity: IdentityKeyPair, 
	channels: RetrieverChannels,
	directories: Arc<GestaltDirectories>,
	conf: Arc<ResourceSystemConfig>,
) -> Result<(), ResourceSysError> {
	let mut quit_reciever = QuitReceiver::new();
	let RetrieverChannels { mut image_requests } = channels;
	
	loop {
		tokio::select! {
			// =========: Image :=========
			image_fetch_maybe = image_requests.recv_wait() => {
				let ResourceFetch { resources, expected_source, return_channel } = image_fetch_maybe?;
				// Initiate a retrieval
				let internal_transfer_channel: MpscChannel<InternalTransfer> = MpscChannel::new(4096);
				let internal_receiver = internal_transfer_channel.take_receiver().unwrap();
				let internal_sender = internal_transfer_channel.sender_subscribe();

				match return_channel {
					Some(channel) => { 
						// Non-preload, fully fetch & parse.
						let role_clone = role.clone(); 
						let identity_clone = self_identity.public.clone(); 
						let directories_clone = directories.clone();
						let conf_clone = conf.clone();
						let resources_clone = resources.clone();
						// Set up to load our bytes.
						let _join_handle_load = tokio::spawn(
							full_load_bytes(
								role_clone,
								identity_clone,
								directories_clone,
								resources_clone,
								expected_source,
								internal_sender,
								conf_clone
							)
						);
						let _join_handle_decode = tokio::spawn(
							load_images(
								resources, 
								internal_receiver, 
								channel
							)
						);
					},
					None => { 
						todo!("Preloads not yet implemented!");
					}
				}
			}
			// ====: Engine shutdown :====
			quit_ready_indicator = quit_reciever.wait_for_quit() => {
				info!("Shutting down resource loading system.");
				quit_ready_indicator.notify_ready();
				break;
			}
		}
	}
	Ok(())
}