use std::fmt;
use std::hash::Hash;
use hashbrown::HashMap;

pub trait ValidTile : 'static + Sized + Copy + Clone + PartialEq + PartialOrd + Hash + fmt::Debug + Default {}
impl<T> ValidTile for T where T : 'static + Sized + Copy + Clone + PartialEq + PartialOrd + Hash + fmt::Debug + Default {}

/// Tiles as they are stored in the world - as in, what a Space will return when you call chunk.get(x, y, z)
pub type TileId = u16;

/// Unlocalized name of a tile.
pub type TileName = String;

/// One coorinate (worldspace) of a tile in a 3D 3-coordinate system (i.e. x: TileCoord, y: TileCoord, z: TileCoord)
pub type TileCoord = i32;

/// Can remap a ChunkTile to a WorldTile and vice-versa. Future-proofing in case we want palettes later.
pub struct TileIdRegistry { 
    pub names : HashMap<TileId, TileName>,
    pub reverse_names : HashMap<TileName, TileId>,
}