use std::io::{BufReader, Read};

use std::collections::HashMap;
use log::warn;
use serde::{Deserialize, Serialize};

use crate::{world::{WorldId, World}, common::identity::IdentityKeyPair};

#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    /// What is the IP address of this server?
    pub server_ip: String,
}
impl Default for ServerConfig {
    fn default() -> Self {
        Self { server_ip: String::from("127.0.0.1") }
    }
}


#[derive(thiserror::Error, Debug)]
pub enum StartServerError {
    #[error("Could not read server config file, i/o error: {0:?}")]
    CouldntOpenConfig( #[from] std::io::Error),
    #[error("Could not parse server config file due to: {0}")]
    CouldntParseConfig(#[from] ron::Error),
    #[error("Could not initialize display: {0:?}")]
    CreateWindowError( #[from] winit::error::OsError),
}

pub const SERVER_CONFIG_FILENAME: &str = "server_config.ron";

pub fn load_server_config() -> Result<ServerConfig, StartServerError> { 
    // Open config
    let mut open_options = std::fs::OpenOptions::new();
    open_options.read(true).append(true).create(true);

    let config_maybe: Result<ServerConfig, StartServerError> = open_options
        .open(SERVER_CONFIG_FILENAME)
        .map_err(StartServerError::from)
        .and_then(|file| {
            let mut buf_reader = BufReader::new(file);
            let mut contents = String::new();
            buf_reader
                .read_to_string(&mut contents)
                .map_err(StartServerError::from)?;
            Ok(contents)
        })
        .and_then(|e| ron::from_str(e.as_str()).map_err(StartServerError::from));
    //If that didn't load, just use built-in defaults.
    Ok(match config_maybe {
        Ok(c) => c,
        Err(e) => {
            warn!(
                "Couldn't open client config, using defaults. Error was: {:?}",
                e
            );
            ServerConfig::default()
        }
    })
}

pub struct ServerNode {
    pub local_identity: IdentityKeyPair,
    pub worlds: HashMap<WorldId, World>,
}

impl ServerNode {
    pub fn new(local_identity: IdentityKeyPair) -> Self {
        ServerNode {
            local_identity,
            worlds: HashMap::new(),
        }
    }
}