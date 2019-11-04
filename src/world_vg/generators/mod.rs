//! World generator types.


mod perlingenerator;

pub use self::perlingenerator::PerlinGenerator;


/// Trait for world generators.
pub trait WorldGenerator {
    /// Generates a chunk with this generator.
    fn generate(&self, pos: (i32, i32, i32), dimension_id: u32) -> super::Chunk;
}