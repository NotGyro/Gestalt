//! Types related to world data.
//! The "Voxel" module contains generalized structures for voxel data,
//! whereas this module will pertain specifically to world terrain in
//! Gestalt (the game).
pub mod tile;

pub use self::tile::{TileID, TileName};

/// Scale 0 is equal to a one meter cubed (one meter along each axis) cube. 
/// So, the length along each axis of our chunk is then 2^(CHUNK_SCALE) meters.
/// This also corresponds to the height of the root node on an octree: 
/// CHUNK_SCALE steps down from the root node is your 1x1x1 meter cube.
pub const CHUNK_SCALE : usize = 6;
//Good candidates are 5 (32 meters to a side), 6 (64 meters to a side),
//or 8 (256 meters to a side).