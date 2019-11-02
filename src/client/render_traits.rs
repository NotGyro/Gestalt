use std::error::Error;
use std::result::Result;

use voxel::subdivstorage::*;
use world::tile::TileID;
use util::ConfigString;

pub trait OctreeRenderer {
    /// Arguments: 
    /// tile: What tile are we setting the art for? 
    /// art: A string which is a TOML or JSON object describing rendering properties for this tile.
	fn reg_tile_art(&mut self, tile : TileID, art : ConfigString) -> Result<(), Box<dyn Error>>;
}