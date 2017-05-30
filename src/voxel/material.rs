extern crate serde;
extern crate serde_yaml;
extern crate string_cache;
extern crate lazy_static;

use self::serde::ser::{Serialize, Serializer, SerializeStruct};
use self::serde::de::DeserializeOwned;
use self::serde::de::{self, Deserialize, Deserializer, Visitor, SeqAccess, MapAccess};
use self::serde_yaml::{to_writer, from_reader};

use std::fmt;

use string_cache::DefaultAtom as Atom;

use std::sync::Mutex;

lazy_static! {
    static ref INTERNAL_ID_MAPPING : Mutex<Vec<String>> = Mutex::new(vec![]);
}

fn id_for(name : &String) -> Option<u64> {
    match INTERNAL_ID_MAPPING.lock().unwrap().iter().position(|ref n| **n == *name) {
        /// An ID exists which matches this name.
        Some(idx) => return Some(idx as u64),
        None => return None,
    }
}
fn id_for_create(name : &String) -> u64 {
    match id_for(name) {
        Some(idx) => return idx,
        None => {
            let mut idmap_handle = INTERNAL_ID_MAPPING.lock().unwrap();
            //The value of len(), when used as an index, is one cell past the end of the vector.
            let temp_len = idmap_handle.len() as u64;
            idmap_handle.push(name.clone());
            //After pushing, temp_len will now point to the index of name in the mapping. Calling len() on it again would now yield an index one unit past name.
            return temp_len;
        },
    }
}
fn name_for(id : u64) -> String {
    //TODO: Better error handling here.
    INTERNAL_ID_MAPPING.lock().unwrap().get(id as usize).unwrap().clone()
}

/// An identifier for a material.
/// These should act like Atoms: Constructing the same Material ID struct with the same name in two completely different contexts should result in functionally the same value.
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub struct MaterialID {
    /// The internal, in-memory representation of the ID.
    /// Since it's a Copy type and not a reference, comparisons and assignments using it should be very fast.
    /// This should NOT be used outside of material.rs. It should not even be sent over the network.
    /// It will not stay valid between disk writes. It really is just a volatile optimization.
    internal_id : u64,
}
impl MaterialID { 
    pub fn to_name(&self) -> String {
        name_for(self.internal_id)
    }
    pub fn from_name(name : &String) -> Self {
        MaterialID { internal_id : id_for_create(name) }
    }
}
impl fmt::Display for MaterialID {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_name())
    }
}

impl Serialize for MaterialID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    { serializer.serialize_str(self.to_name().as_str()) }
}

impl<'de> Deserialize<'de> for MaterialID {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        //Many thanks to dtolnay on IRC for the much, MUCH more elegant implementation.
        String::deserialize(deserializer).map(|name| MaterialID::from_name(&name))
    }
}

/*
impl Into<&String> for MaterialID { 
    fn into(self) -> String { self.to_name() }
}*/

//Is this still totally necessary? I suppose it'll be necessary when whe associate behaviors with IDs.
#[derive(Clone)]
pub struct MaterialIndex { }

impl MaterialIndex { 
    pub fn new() -> Self {
        MaterialIndex { }
    }
    pub fn for_name(&self, n : &String) -> MaterialID {
        MaterialID::from_name(n)
    }
    pub fn name_of(&self, mat : MaterialID ) -> String { 
        mat.to_name()
    }
}

/* A Material in Gestalt represents any solid voxel that is part of the game-world. Stone walls, dirt, air, etc...
The representation in memory and on disk of a Material must be something you can boil down to a MaterialID. There can be
separate metadata, but the primary thing saying "there is a material here" must be that a cell in a VoxelStorage can
evaluate to a MaterialID which is then linked to the Material.

In the game I'm trying to make, there will be separate BlockMaterials and TerrainMaterials, with TerrainMaterials meshing
via marching cubes to a smooth mesh and BlockMaterials becoming Minecraft-like cubes. Not quite sure how to architect that yet -
separate types, or something that acts like inheritance from a common Material class would in a straight OO language?

Note I mentioned solids because fluids will be a different beast entirely - the voxel itself will be either a floating point
value or some range represented with an integer, and the world layer it is contained in will imply the type of the fluid.
*/
pub trait Material {
    fn get_id() -> MaterialID;
}
