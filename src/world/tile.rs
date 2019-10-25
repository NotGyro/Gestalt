extern crate string_cache;
extern crate parking_lot;

use self::string_cache::DefaultAtom as Atom;
use std::collections::HashMap;

use self::parking_lot::Mutex;
use voxel::voxelarray::VoxelArray;

pub type TileID = u64;
pub type TileName = Atom;
pub type Chunk = VoxelArray<TileID, u8>;

pub struct TileRegistry {
    id_to_name : Vec<TileName>,
    name_to_id : HashMap<TileName,TileID>,
}

impl TileRegistry {
    pub fn id_for_name(&self, id : &TileID) -> TileName{
        self.id_to_name.get(*id as usize).unwrap().clone()
    }
    pub fn name_for_id(&self, name : &TileName) -> TileID{ self.name_to_id.get(name).unwrap().clone() }
    pub fn all_mappings(&self) -> HashMap<TileName, TileID> { self.name_to_id.clone()}
    pub fn register_tile(&mut self, name: &TileName) -> TileID { 
        {
            assert!(self.name_to_id.contains_key(name) == false);
        }
        let new_id = self.id_to_name.len() as TileID;
        self.id_to_name.push(name.clone());
        self.name_to_id.insert(name.clone(), new_id.clone());
        return new_id;
    }
}

lazy_static! {
    pub static ref TILE_REGISTRY : Mutex<TileRegistry> = {
        Mutex::new(TileRegistry { 
            id_to_name : Vec::new(),
            name_to_id : HashMap::new(),
        })
    };
}
