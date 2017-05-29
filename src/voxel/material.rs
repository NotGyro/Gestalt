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

pub trait SerializeAs<T> {
    fn get_serialize(&self) -> T;
    fn from_serialize(value : T) -> Self;
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
/*
impl SerializeAs<String> for MaterialID { 
    fn get_serialize(&self) -> String {
        self.to_name()
    }
    fn from_serialize(value : String) -> Self {
        MaterialID::from_name(&value)
    }
}*/

impl Serialize for MaterialID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    { serializer.serialize_str(self.to_name().as_str()) }
}

impl<'de> Deserialize<'de> for MaterialID {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where D: Deserializer<'de>
    {
        enum Field { Name };

        // This part could also be generated independently by:
        //
        //    #[derive(Deserialize)]
        //    #[serde(field_identifier, rename_all = "lowercase")]
        //    enum Field { Secs, Nanos }
        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
                where D: Deserializer<'de>
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`name`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                        where E: de::Error
                    {
                        match value {
                            "name" => Ok(Field::Name),
                            _ => Err(de::Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct NameVisitor;

        impl<'de> Visitor<'de> for NameVisitor {
            type Value = MaterialID;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct MaterialID")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<MaterialID, V::Error>
                where V: SeqAccess<'de>
            {
                let name : String = seq.next_element()?
                              .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                Ok(MaterialID::from_name(&name))
            }

            fn visit_map<V>(self, mut map: V) -> Result<MaterialID, V::Error>
                where V: MapAccess<'de>
            {
                let mut name = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            if name.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name = Some(map.next_value()?);
                        }
                    }
                }
                let name : String = name.ok_or_else(|| de::Error::missing_field("name"))?;
                Ok(MaterialID::from_name(&name))
            }
        }

        const FIELDS: &'static [&'static str] = &["name"];
        deserializer.deserialize_struct("MaterialID", FIELDS, NameVisitor)
    }
}
//type MaterialID = Atom;

/*
#[derive(Clone, Debug, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaterialID {
    pub name : String,
}

impl Into<String> for MaterialID { 
    fn into(self) -> String { self.name }
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
