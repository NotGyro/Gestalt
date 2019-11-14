//! Types related to world data.
//! The "Voxel" module contains generalized structures for voxel data,
//! whereas this module will pertain specifically to world terrain in
//! Gestalt (the game).

pub mod tile;
pub use self::tile::{TileID, TileName};

pub mod generators;

pub mod chunk;
pub use self::chunk::Chunk;

pub mod dimension;
pub use self::dimension::Dimension;

pub mod gen_cache;


/// A voxel at scale 0 is one cubic meter. Each step up in scale multiplies by 2.
/// So, the length along each axis of our chunk is then 2^(CHUNK_SCALE) meters.
/// This also corresponds to the height of the root node on an octree: 
/// CHUNK_SCALE steps down from the root node is your 1x1x1 meter cube.
#[allow(dead_code)]
pub const CHUNK_SCALE : i8 = 5; // 32x32x32 meters
/// Helper constant for the size of a chunk in 1m³ voxels, as a `usize`.
///
/// ```
/// assert!(gestalt::world::CHUNK_SIZE as u32 == 2u32.pow(gestalt::world::CHUNK_SCALE as u32));
/// ```
pub const CHUNK_SIZE : usize = 32; // 2 ^ 5
/// Helper constant for the size of a chunk in 1m³ voxels, as a `f32`.
///
/// ```
/// assert!(gestalt::world::CHUNK_SIZE_F32 == 2f32.powf(gestalt::world::CHUNK_SCALE as f32));
/// ```
pub const CHUNK_SIZE_F32 : f32 = 32.0; // 2 ^ 5
