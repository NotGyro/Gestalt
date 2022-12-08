use std::{path::PathBuf, fs::OpenOptions, io::{BufReader, BufWriter}};

use log::trace;
use serde::{Serialize, Deserialize};
use uuid::Uuid;

use super::{WorldId, ChunkCoord, ChunkPos, TileId};

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub enum StoredWorldRole { 
    /// Any world this node owns, authoritatively, and can retain and publish changes to it.
    Local,
    /// The cache of a remote world, i.e. stored copies of a server's world so that a 
    /// client can have them on hand instantly when reconnecting.
    RemoteCached,
    /// Non-authoritative durable copy of a remote world - Doesn't get erased by 
    /// cache limits, but we can't publish changes to it.
    RemoteBackup
}

pub fn path_worlds(base_dir: &PathBuf) -> PathBuf {
    const WORLD_DIR: &str = "worlds/";
    let world_root = base_dir.join(WORLD_DIR);
    if !world_root.exists() { 
        std::fs::create_dir(&world_root).unwrap();
    }
    world_root
}

/// Gets the path to the root directory for per-world data (voxel chunks, entities, etc)
pub fn path_for_world(base_dir: &PathBuf, world_id: &WorldId, role: StoredWorldRole) -> PathBuf {
    match role {
        StoredWorldRole::Local => {
            let world_root = path_worlds(base_dir);
            let result = world_root.join(format!("{}/", world_id.uuid ) );
            if !result.exists() {
                std::fs::create_dir(&result).unwrap();
            }
            result
        },
        StoredWorldRole::RemoteCached => todo!(),
        StoredWorldRole::RemoteBackup => todo!(),
    }
}

/// Gets the path to the directory where voxel chunks are stored
/// Helper function that just calls path_for_world() internally.
pub fn path_for_terrain(base_dir: &PathBuf, world_id: &WorldId, role: StoredWorldRole) -> PathBuf { 
    let path = path_for_world(base_dir, world_id, role).join("terrain/");
    if !path.exists() {
        std::fs::create_dir_all(&path).unwrap();
    }
    path
}

#[inline]
fn chunk_coord_to_str(coord: &ChunkCoord) -> String { 
    if *coord >= 0 {
        format!("+{}", coord)
    }
    else {
        format!("-{}", coord.abs())
    }
}

#[inline]
pub fn filename_for_chunk(pos: &ChunkPos) -> String { 
    format!("{}x_{}y_{}z.chunk",
        chunk_coord_to_str(&pos.x),
        chunk_coord_to_str(&pos.y),
        chunk_coord_to_str(&pos.z))
}

#[inline]
pub fn path_for_chunk(base_dir: &PathBuf, world_id: &WorldId, role: StoredWorldRole, pos: &ChunkPos) -> PathBuf {
    path_for_terrain(base_dir, world_id, role).join(filename_for_chunk(pos))
}

/*
pub fn load_chunk(world_id: &WorldId, role: StoredWorldRole, pos: &ChunkPos) -> std::result::Result<Chunk<TileId>, ChunkIoError> {
    let path = path_for_chunk(world_id, role, pos);
    let file = OpenOptions::new()
        .read(true)
        .write(false)
        .create(false)
        .open(path)?;
    
    let mut reader = BufReader::new(file);
    deserialize_chunk(&mut reader)
}

pub fn save_chunk(world_id: &WorldId, role: StoredWorldRole, pos: &ChunkPos, chunk: &Chunk<TileId>) -> std::result::Result<(), ChunkIoError> {
    let target_path = path_for_chunk(world_id, role, pos);
    // Write the file to a temporary path so that, if it crashes in the process of serializing,
    // it does not corrupt previously-existing world state.
    let in_progress_path = target_path.with_extension("chunk.lock");

    trace!("Saving chunk to: {}", in_progress_path.to_str().unwrap());
    
    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&in_progress_path)?;
    
    let mut writer = BufWriter::new(file);
    chunk.write_chunk(&mut writer)?;
    //This should be an atomic operation, so world state won't get corrupted here.
    std::fs::rename(&in_progress_path, target_path)?;
    Ok(())
}*/

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldDefaults { 
    /// Default local world to automatically log into.
    /// None on first launch. Should auto-fill at first launch
    pub lobby_world_id: Option<Uuid>,
}