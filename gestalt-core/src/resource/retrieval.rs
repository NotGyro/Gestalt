// ! Async-sided resource system code, primarily consisting of the "actually go grab the file"
// ! logic of querying the disk cache and then, if no cached file is found, attempting to fetch
// ! it from a presently-connected server.

use log::info;

use crate::{net::SelfNetworkRole, common::identity::IdentityKeyPair, message::{MpscReceiver, QuitReceiver, MessageReceiverAsync}};

use super::{RESOURCE_FETCH, ResourceFetch};

#[derive(thiserror::Error, Debug, Clone)]
pub enum ResourceSysError {
	#[error("Could not launch resource system - \
        Resource fetch request channel has already been claimed. \
        It is possible launch_resource_system() has been invoked twice.")]
	NoFetchReceiver,
}

/// Initializes the asynchronous end (i.e. most of it) of the resource-loading system. 
pub async fn launch_resource_system(role: SelfNetworkRole, self_identity: IdentityKeyPair)
        -> Result<(), ResourceSysError> {
    // Claim ownership over resource-retrieval request channels for the engine internals.
    let fetch_receiver = RESOURCE_FETCH.take_receiver().ok_or(ResourceSysError::NoFetchReceiver)?;
    resource_system_main(role, self_identity, fetch_receiver).await?;
    Ok(())
}

/// Mainloop for the resource-loading system.
pub async fn resource_system_main(role: SelfNetworkRole, self_identity: IdentityKeyPair,
        resource_fetch_receiver: MpscReceiver<ResourceFetch>) -> Result<(), ResourceSysError> {
    let mut quit_reciever = QuitReceiver::new();
    loop { 
        tokio::select! {
            resource_fetch_cmd = resource_fetch_receiver.recv_wait() => { 
                
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