//! Types related to dimensions/chunks.

pub mod generators;

#[allow(dead_code)]
pub mod chunk;
pub mod dimension;

pub use self::chunk::Chunk;
pub use self::dimension::Dimension;
