//pub trait ValidTile: 'static + Sized + Copy + Clone + PartialEq + PartialOrd + Hash + fmt::Debug + Default {}
//impl<T> ValidTile for T where T: 'static + Sized + Copy + Clone + PartialEq + PartialOrd + Hash + fmt::Debug + Default {}

/// Tiles as they are interacted with in the world (not as stored in a chunk, necessarily) - as in, what a Space will return when you call chunk.get(x, y, z)
pub type TileId = u32;

/// Unlocalized name of a tile.
pub type TileName = String;

/// One coorinate (worldspace) of a tile in a 3D 3-coordinate system (i.e. x: TileCoord, y: TileCoord, z: TileCoord)
pub type TileCoord = i32;
