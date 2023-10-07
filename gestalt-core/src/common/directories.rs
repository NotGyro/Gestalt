
//Note to self for later: std::fs::create_dir_all() will come in handy

use std::{path::PathBuf, array::from_fn};

use serde::{Serialize, Deserialize};

use super::byte_to_hex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectoryConfig {
    /// The root from which gamedata_root and engine_root branch. 
    pub overall_root: String,
    /// The root from which resources and worlds branch. 
    pub gamedata_root: String,
    /// The root from which keys, net, logs, and built_in branch. 
    pub engine_root: String,
    pub keys: String,
    pub net: String,
    /// Parent / root dir in which cache/, local/ etc live.
    pub resources: String,
    pub logs: String,
    pub worlds: String,
    /// Built-in resources directory. It's STRONGLY recommended not to place this inside
    /// the resources directory, for security. 
    pub built_in: String,
}

impl Default for DirectoryConfig {
    fn default() -> Self {
        Self {
            overall_root: String::default(), 
            gamedata_root: String::from("data"), 
            engine_root: String::from("system"),
            keys: String::from("keys"),
            net: String::from("net"),
            resources: String::from("resources"), 
            logs: String::from("logs"),
            worlds: String::from("worlds"),
            built_in: String::from("engine_resources"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct GestaltDirectories { 
    /// The root from which gamedata_root and engine_root branch. 
    pub overall_root: PathBuf,
    /// The root from which resources and worlds branch. 
    pub gamedata_root: PathBuf,
    /// Parent / root dir in which cache/, local/ etc live.
    pub resources: PathBuf,
    pub resources_cache: PathBuf,
    pub resources_cache_buckets: [PathBuf; 256],
    pub resources_local: PathBuf,
    pub worlds: PathBuf,
    pub worlds_local: PathBuf,
    pub worlds_remote: PathBuf,
    
    /// The root from which keys, net, logs, and built_in branch. 
    pub engine_root: PathBuf,
    pub keys: PathBuf,
    pub net: PathBuf,
    pub net_protocol: PathBuf,
    pub logs: PathBuf,

    /// Built-in resources directory. It's STRONGLY recommended not to place this inside
    /// the resources directory, for security. 
    pub built_in: PathBuf,
}
impl From<DirectoryConfig> for GestaltDirectories {
    fn from(conf: DirectoryConfig) -> Self {
        let overall_root = PathBuf::from(&conf.overall_root);
        let gamedata_root = overall_root.join(&conf.gamedata_root);
        std::fs::create_dir_all(&gamedata_root).unwrap();
        let resources = gamedata_root.join(&conf.resources);
        std::fs::create_dir_all(&resources).unwrap();
        let resources_cache = resources.join("cache");
        std::fs::create_dir_all(&resources_cache).unwrap();

        let resources_cache_clone = resources_cache.clone(); 
        let resources_cache_buckets = from_fn(move |i| {
            let hex_str = byte_to_hex(i as u8);
            let resl = resources_cache_clone.join(hex_str);
            std::fs::create_dir_all(&resl).unwrap();
            resl
        });
        let resources_local = resources.join("local");
        std::fs::create_dir_all(&resources_local).unwrap();

        let worlds = gamedata_root.join(&conf.worlds);
        let worlds_local = worlds.join("local");
        std::fs::create_dir_all(&worlds_local).unwrap();
        let worlds_remote = worlds.join("remote");
        std::fs::create_dir_all(&worlds_remote).unwrap();

        let engine_root = overall_root.join(&conf.engine_root);
        let keys = engine_root.join(&conf.keys);
        std::fs::create_dir_all(&keys).unwrap();
        let net = engine_root.join(&conf.net);
        std::fs::create_dir_all(&net).unwrap();
        let net_protocol = net.join("protocol"); 
        std::fs::create_dir_all(&net_protocol).unwrap();
        let logs = engine_root.join(&conf.logs);
        std::fs::create_dir_all(&logs).unwrap();
        let built_in = engine_root.join(&conf.built_in);
        std::fs::create_dir_all(&built_in).unwrap();

        // Sanity-check for security. 
        for ancestor in built_in.ancestors() { 
            if ancestor == gamedata_root { 
                panic!("Engine built-in resources directory should not be inside game-data directory!");
            }
        }

        Self {
            overall_root,
            gamedata_root,
            resources,
            resources_cache,
            resources_cache_buckets,
            resources_local,
            worlds,
            worlds_local,
            worlds_remote,
            
            engine_root,
            keys,
            net,
            net_protocol,
            logs,
            built_in,
        }
    }
}

impl Default for GestaltDirectories {
    fn default() -> Self {
        DirectoryConfig::default().into()
    }
}