
//Note to self for later: std::fs::create_dir_all() will come in handy

use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryConfig { 
    pub keys_dir: String,
    pub net_dir: String,
    /// Parent / root dir in which builtin/, cache/, etc live.
    pub resources_dir: String,
    pub logs_dir: String,
    pub worlds_dir: String,
}