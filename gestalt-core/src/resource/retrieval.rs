// ! Async-sided resource system code, primarily consisting of the "actually go grab the file"
// ! logic of querying the disk cache and then, if no cached file is found, attempting to fetch
// ! it from a presently-connected server.

use std::{sync::Arc, path::PathBuf};

use log::{info, trace, error};
use tokio::{io::AsyncReadExt};

use crate::{
    net::SelfNetworkRole, 
    common::{identity::{IdentityKeyPair, NodeIdentity}, directories::GestaltDirectories},
    message::{MpscReceiver, QuitReceiver, MessageReceiverAsync, MpscSender}
};

use super::{RESOURCE_FETCH, ResourceFetch, resource_id_to_prefix, ResourceId, ResourceLoadError, ResourceFetchResponse};

static LOCK_SUFFIX: &'static str = ".lock";

#[derive(thiserror::Error, Debug, Clone)]
pub enum ResourceSysError {
	#[error("Could not launch resource system - \
        Resource fetch request channel has already been claimed. \
        It is possible launch_resource_system() has been invoked twice.")]
	NoFetchReceiver,
}

/// Initializes the asynchronous end (i.e. most of it) of the resource-loading system. 
pub async fn launch_resource_system(role: SelfNetworkRole, self_identity: IdentityKeyPair,
    directories: Arc<GestaltDirectories>)
        -> Result<(), ResourceSysError> {
    // Claim ownership over resource-retrieval request channels for the engine internals.
    let fetch_receiver = RESOURCE_FETCH.take_receiver().ok_or(ResourceSysError::NoFetchReceiver)?;
    resource_system_main(role, self_identity, fetch_receiver, directories).await?;
    Ok(())
}

fn path_for_resource(id: &ResourceId, origin_identity: &NodeIdentity, self_identity: &NodeIdentity,
        directories: Arc<GestaltDirectories>) -> PathBuf {
    let parent_dir: PathBuf = {
        if &origin_identity == &self_identity {
            directories.resources_local.clone()
        }
        else {
            directories.resources_cache_buckets[resource_id_to_prefix(&id)]
        }
    };
    let path = parent_dir.join(id.to_string());
    path
}

async fn load_from_file(resources: Vec<ResourceId>, expected_source: NodeIdentity,
        self_identity: NodeIdentity, directories: Arc<GestaltDirectories>, 
        channel: Option<MpscSender<ResourceFetchResponse>>) {
    
    let mut not_on_disk = Vec::new();
    for resource in resources {
        let path = path_for_resource(&resource, 
            &expected_source, &self_identity.public, 
            directories.clone());
        match tokio::fs::OpenOptions::new().read(true).open(path).await {
            Ok(file) => {
                // Is this a non-preload?
                if let Some(chan) = channel { 
                    let mut buffer = Vec::new();
                    if let Err(e) = file.read_to_end(&mut buffer).await {
                        error!("Error when attempting to read file {0} into memory: {1:?}", path, e);
                        chan.send(
                            Result::Err(
                                ResourceLoadError::Disk(resource.clone(), format!("{0:?}", e))
                            )
                        );
                    }
                    else {
                        chan.send(Result::Ok(Arc::new(buffer)));
                    }
                }
                else {
                    trace!("Attempted to pre-load resource {0:?} which \
                        is already present on disk - ignoring.", &resource);
                }
            },
            Err(e) => {
                match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        not_on_disk.push(resource.clone());
                    },
                    _ => error!("Failed to load file {0:?} at location {1} due to error {2:?}.", 
                            &resource, &path, e),
                }
            },
        }
        // TODO! Network retrieval goes here.
        if !not_on_disk.is_empty() {
            todo!("File retrieval over the network is not yet implemented, cannot retrieve: {:?}",
                &not_on_disk);
        }
    }

    
}

/// Mainloop for the resource-loading system.
async fn resource_system_main(role: SelfNetworkRole, self_identity: IdentityKeyPair,
    resource_fetch_receiver: MpscReceiver<ResourceFetch>, directories: Arc<GestaltDirectories>) 
        -> Result<(), ResourceSysError> {
    let mut quit_reciever = QuitReceiver::new();
    loop { 
        tokio::select! {
            resource_fetch_maybe = resource_fetch_receiver.recv_wait() => { 
                let resource_fetch_cmds = resource_fetch_maybe.unwrap();
                for resource_fetch_cmd in resource_fetch_cmds {
                    tokio::spawn(async move {
                        // Network retrieval is invoked in a failure case of an attempt to load from
                        // a file, when the file is not found. So, network handling will not be inside
                        // this function, but invoked inside load_from_file().
                        // Hopefully there will be a performance benefit from not having to touch
                        // the file twice.
                        load_from_file(resource_fetch_cmd.resources,
                            resource_fetch_cmd.expected_source, self_identity.public.clone(), 
                            directories.clone(), resource_fetch_cmd.return_channel).await
                    });
                }
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