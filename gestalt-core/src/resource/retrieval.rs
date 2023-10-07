// ! Async-sided resource system code, primarily consisting of the "actually go grab the file"
// ! logic of querying the disk cache and then, if no cached file is found, attempting to fetch
// ! it from a presently-connected server.

use std::{sync::Arc, path::PathBuf};

use log::{info, trace, error};
use tokio::{sync::oneshot, io::AsyncReadExt};

use crate::{net::SelfNetworkRole, common::{identity::{IdentityKeyPair, NodeIdentity}, directories::GestaltDirectories}, message::{MpscReceiver, QuitReceiver, MessageReceiverAsync}};

use super::{RESOURCE_FETCH, ResourceFetch, resource_id_to_prefix, ResourceId, GeneralResourceLoadError};

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

fn path_for_resource(id: ResourceId, origin_identity: NodeIdentity, self_identity: NodeIdentity,
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

type ReturnChannel = oneshot::Sender<Result<Arc<Vec<u8>>, GeneralResourceLoadError>>;

async fn load_from_file(id: ResourceId, origin_identity: NodeIdentity, self_identity: NodeIdentity,
        directories: Arc<GestaltDirectories>, path: PathBuf, channel: Option<ReturnChannel>) { 
    let file = match tokio::fs::OpenOptions::new().read(true).open(path).await {
        Ok(file) => {
            if let Some(chan) = channel { 
                let mut buffer = Vec::new();
                if let Err(e) = file.read_to_end(&mut buffer).await {
                    error!("Error when attempting to read file {0} into memory: {1:?}", path, e);
                    chan.send(
                        Result::Err(
                            GeneralResourceLoadError::Disk(id.clone(), format!("{0:?}", e))
                        )
                    );
                }
                else {
                    chan.send(Result::Ok(Arc::new(buffer)));
                }
            }
            else {
                trace!("Attempted to pre-load resource {0:?} which \
                    is already present on disk - ignoring.", &id);
            }
        },
        Err(e) => {
            match e.kind() {
                std::io::ErrorKind::NotFound => {
                    // TODO! Network retrieval goes here.
                    todo!("File retrieval over the network is not yet implemented.")
                },
                _ => error!("Failed to load file {0:?} at location {1} due to error {2:?}.", 
                        &id, &path, e),
            }
        },
    };
    
}

/// Mainloop for the resource-loading system.
async fn resource_system_main(role: SelfNetworkRole, self_identity: IdentityKeyPair,
    resource_fetch_receiver: MpscReceiver<ResourceFetch>, directories: Arc<GestaltDirectories>) 
        -> Result<(), ResourceSysError> {
    let mut quit_reciever = QuitReceiver::new();
    loop { 
        tokio::select! {
            resource_fetch_cmd = resource_fetch_receiver.recv_wait() => { 

                let path = path_for_resource(resource_fetch_cmd.resource_id.clone(), 
                    resource_fetch_cmd.expected_source.clone(), self_identity.public.clone(), 
                    directories.clone());
                
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