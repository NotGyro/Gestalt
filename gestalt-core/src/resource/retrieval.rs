// ! Async-sided resource system code, primarily consisting of the "actually go grab the file"
// ! logic of querying the disk cache and then, if no cached file is found, attempting to fetch
// ! it from a presently-connected server.

use std::{path::PathBuf, sync::Arc};

use log::{error, info, trace};
use tokio::io::AsyncReadExt;

use crate::{
	common::{
		directories::GestaltDirectories,
		identity::{IdentityKeyPair, NodeIdentity},
	},
	message::{MessageReceiverAsync, MpscReceiver, MpscSender, QuitReceiver},
	net::SelfNetworkRole, resource::ResourceLocation, MessageSender,
};

use super::{resource_id_to_prefix, Caid, ResourceFilelike, ResourceRetrievalError};

static LOCK_SUFFIX: &'static str = ".lock";

#[derive(thiserror::Error, Debug, Clone)]
pub enum ResourceSysError {
	#[error(
		"Could not launch resource system - \
        Resource fetch request channel has already been claimed. \
        It is possible launch_resource_system() has been invoked twice."
	)]
	NoFetchReceiver,
}

#[derive(Debug)]
pub struct ResourceFetch {
	pub resources: Vec<ResourceLocation>,
	pub expected_source: NodeIdentity,
	// If this field contains a Some value, this is treated as a resource to be loaded
	// into memory, and then onto disk after that.
	// If this field contains a None value, this is treated as a pre-load, and the resource
	// is only saved to disk and not retained in memory.
	//pub return_channel: Option<MpscSender<ResourceFetchResponse>>,
}

#[derive(Debug)]
pub struct ResourceFetchResponse {
	pub id: ResourceLocation,
	pub data: Result<Arc<Vec<u8>>, ResourceRetrievalError>,
}

/// Initializes the asynchronous end (i.e. most of it) of the resource-loading system.
pub async fn launch_resource_system(
	role: SelfNetworkRole,
	self_identity: IdentityKeyPair,
	directories: Arc<GestaltDirectories>,
) -> Result<(), ResourceSysError> {
	// Claim ownership over resource-retrieval request channels for the engine internals.
	let fetch_receiver = todo!(); //= RESOURCE_FETCH
	//	.take_receiver()
	//	.ok_or(ResourceSysError::NoFetchReceiver)?;
	resource_system_main(role, self_identity, fetch_receiver, directories).await?;
	Ok(())
}

fn path_for_resource(
	id: &ResourceLocation,
	_origin_identity: &NodeIdentity,
	_self_identity: &NodeIdentity,
	directories: Arc<GestaltDirectories>,
) -> ResourceFilelike {
	match id.file_name() {
		ResourceFilelike::File(file_name) => {
			let parent_dir = match id {
				ResourceLocation::Caid(caid) => { 
					&directories.resources_cache_buckets[resource_id_to_prefix(caid)]
				},
				ResourceLocation::Local(local_res) => match local_res {
						super::LocalResource::User(user) => &directories.resources_user,
						super::LocalResource::Internal(_) =>  unreachable!("id.file_name() on an internal resource should never return ResourceFilelike::File()"),
					},
				ResourceLocation::Link(_link) => todo!(),
			};
			ResourceFilelike::File(parent_dir.join(file_name))
		},
		ResourceFilelike::Internal(internal) => ResourceFilelike::Internal(internal),
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum FileLoadError {
	#[error("Could not send a file on a channel for resource {0:?}")]
	NoSendChannel(ResourceLocation),
}

async fn load_from_file(
	mut resources: Vec<ResourceLocation>,
	expected_source: NodeIdentity,
	self_identity: NodeIdentity,
	directories: Arc<GestaltDirectories>,
	channel: Option<MpscSender<ResourceFetchResponse>>,
) -> Result<(), FileLoadError> {
	let mut not_on_disk = Vec::new();
	for resource in resources.drain(..) {
		let maybe_file_path =
			path_for_resource(&resource, &expected_source, &self_identity, directories.clone());
		match maybe_file_path {
			ResourceFilelike::File(path) => {
					
				match tokio::fs::OpenOptions::new()
					.read(true)
					.open(path.clone())
					.await
				{
					Ok(mut file) => {
						// Is this a non-preload?
						if let Some(ref chan) = channel {
							let mut buffer = Vec::new();
							if let Err(e) = file.read_to_end(&mut buffer).await {
								error!(
									"Error when attempting to read file {0:?} into memory: {1:?}",
									path, e
								);
								chan.send(ResourceFetchResponse {
									id: resource.clone(),
									data: Err(ResourceRetrievalError::Disk(
										resource.clone(),
										format!("{0:?}", e),
									)),
								}).map_err(|e| FileLoadError::NoSendChannel(resource.clone()))?;
							} else {
								chan.send(ResourceFetchResponse {
									id: resource.clone(),
									data: Result::Ok(Arc::new(buffer)),
								}).map_err(|_e| FileLoadError::NoSendChannel(resource.clone()))?;
							}
						} else {
							trace!(
								"Attempted to pre-load resource {0:?} which \
								is already present on disk - ignoring.",
								&resource
							);
						}
					}
					Err(e) => match e.kind() {
						std::io::ErrorKind::NotFound => {
							not_on_disk.push(resource.clone());
						}
						_ => error!(
							"Failed to load file {0:?} at location {1:?} due to error {2:?}.",
							&resource, &path, e
						),
					},
				}
				// TODO! Network retrieval goes here.
				if !not_on_disk.is_empty() {
					todo!(
						"File retrieval over the network is not yet implemented, cannot retrieve: {:?}",
						&not_on_disk
					);
				}
			},
			ResourceFilelike::Internal(_) => todo!(),
		}
	}
	Ok(())
}

/// Mainloop for the resource-loading system.
async fn resource_system_main(
	role: SelfNetworkRole,
	self_identity: IdentityKeyPair,
	mut resource_fetch_receiver: MpscReceiver<ResourceFetch>,
	directories: Arc<GestaltDirectories>,
) -> Result<(), ResourceSysError> {
	let mut quit_reciever = QuitReceiver::new();
	loop {
		// Should be cheap because it's an ARC.
		let dir_clone = directories.clone();
		tokio::select! {
			resource_fetch_maybe = resource_fetch_receiver.recv_wait() => {
				let resource_fetch_cmd = resource_fetch_maybe.unwrap();
				tokio::spawn(async move {
					// Network retrieval is invoked in a failure case of an attempt to load from
					// a file, when the file is not found. So, network handling will not be inside
					// this function, but invoked inside load_from_file().
					// Hopefully there will be a performance benefit from not having to touch
					// the file twice.
					//load_from_file(resource_fetch_cmd.resources,
					//	resource_fetch_cmd.expected_source, self_identity.public.clone(),
					//	dir_clone, resource_fetch_cmd.return_channel).await
					todo!()
				});
			}
			quit_ready_indicator = quit_reciever.wait_for_quit() => {
				info!("Shutting down resource loading system.");
				quit_ready_indicator.notify_ready();
				break;
			}
		}
	}

	Ok(())
}
