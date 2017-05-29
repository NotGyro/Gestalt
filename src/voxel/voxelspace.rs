extern crate std;
extern crate linear_map;
extern crate num;

use voxel::voxelstorage::VoxelStorage;
use voxel::voxelstorage::VoxelStorageIOAble;
use voxel::voxelarray::*;
use voxel::vspalette::*;

use voxel::material::MaterialID;
use voxel::material::MaterialIndex;
use util::voxelutil::VoxelPos;
use util::voxelutil::VoxelRange;
//use voxel::voxelstorage::ContiguousVS;
use std::io;
use std::io::prelude::*;
use std::path::Path;
use std::error::Error;
use std::fs::File;
use std::fs::OpenOptions;

use std::marker::Copy;

use self::linear_map::LinearMap;
use self::linear_map::Entry::{Occupied, Vacant};

type Chunk = VoxelPalette<MaterialID, u8, VoxelArray<u8>, u16>;

const CHUNK_X_LENGTH : usize = 16;
const CHUNK_Y_LENGTH : usize = 16;
const CHUNK_Z_LENGTH : usize = 16;
const OURSIZE : usize  = (CHUNK_X_LENGTH * CHUNK_Y_LENGTH * CHUNK_Z_LENGTH) as usize;
const EXPECTED_CHUNKS : usize = 64;

impl Chunk {
	fn new_chunk(default_val : MaterialID) -> Box<Chunk> {
        let mut array : Vec<u8> = vec![0; OURSIZE];
        let mut bva : Box<VoxelArray<u8>> = VoxelArray::load_new(CHUNK_X_LENGTH as u16, CHUNK_Y_LENGTH as u16, CHUNK_Z_LENGTH as u16, array);
        let mut result = Box::new( VoxelPalette::new(bva) );
        // VoxelPalette<String, u8, VoxelArray<u8>, u16> 
        result.init_default_value(default_val, 0);
		return result;
	}
}

fn testworldgen_surface(chunk : &mut Chunk, air_id : MaterialID, stone_id : MaterialID, dirt_id : MaterialID, grass_id : MaterialID) {
    let surface = ((CHUNK_Z_LENGTH / 8) * 3) as u16;
    let dirt_height = (surface-2);
    for x in 0 .. CHUNK_X_LENGTH as u16 {
        for y in 0 .. CHUNK_Y_LENGTH as u16 {
            for z in 0 .. dirt_height {
                chunk.set(x, y, z, stone_id.clone()); //TODO: less stupid material IDs that pass-by-copy by default
            }
            for z in dirt_height .. surface {
                chunk.set(x, y, z, dirt_id.clone()); //TODO: less stupid material IDs that pass-by-copy by default
            }
            chunk.set(x, y, surface, grass_id.clone());
        }
    }
}
fn testworldgen_underground(chunk : &mut Chunk, air_id : MaterialID, stone_id : MaterialID) {
    for x in 0 .. CHUNK_X_LENGTH as u16 {
        for y in 0 .. CHUNK_Y_LENGTH as u16 {
            for z in 0 .. CHUNK_Z_LENGTH as u16 {
                chunk.set(x, y, z, stone_id.clone()); //TODO: less stupid material IDs that pass-by-copy by default
            }
        }
    }
}


fn pos_to_local(block_pos : VoxelPos<i32>, chunk_pos : VoxelPos<i32>) -> VoxelPos<u16> {
    let mut chunk_origin : VoxelPos<i32> = VoxelPos{ x : chunk_pos.x * CHUNK_X_LENGTH as i32, y : chunk_pos.y * CHUNK_Y_LENGTH as i32, z : chunk_pos.z * CHUNK_Z_LENGTH as i32};
    let mut temp_pos = block_pos - chunk_origin;
    let new_pos : VoxelPos<u16> = VoxelPos{ x : temp_pos.x as u16, y : temp_pos.y as u16, z : temp_pos.z as u16};
    return new_pos;
}

/*fn pos_to_local(p : VoxelPos<i32>) -> VoxelPos<u16> {
    VoxelPos { x : x_to_local(p.x), y : y_to_local(p.y), z : z_to_local(p.z) }
}*/

/// Converts a unit measured in blocks (one of our i32 = 1 block) to one measured in chunks (one i32 = 1 chunk).
fn select_chunk(xp : i32, yp : i32, zp : i32) -> VoxelPos<i32> { 
    VoxelPos { 
        x : ((xp as f64) / (CHUNK_X_LENGTH as f64)).floor() as i32, //Consider a more efficient way of doing this other than casting to f64 and back.
        y : ((yp as f64) / (CHUNK_Y_LENGTH as f64)).floor() as i32,
        z : ((zp as f64) / (CHUNK_Z_LENGTH as f64)).floor() as i32
    }
}

/// A primitive big-world representation. 
/// Will get split off into a bunch of smaller structs and traits later.
pub struct VoxelSpace {
    chunk_list : LinearMap<VoxelPos<i32>, Chunk>,
    mat_idx : MaterialIndex,
    pub not_loaded_val : MaterialID,
    pub error_val : MaterialID,
}


impl VoxelSpace { 
    //Intended to take ownership of the material index, which should be a cloneable type.
    pub fn new(mat_idx : MaterialIndex) -> Self {
        VoxelSpace { 
            chunk_list : LinearMap::with_capacity(EXPECTED_CHUNKS),
            not_loaded_val : mat_idx.for_name(&String::from("reserved.not_loaded")),
            error_val : mat_idx.for_name(&String::from("reserved.error")),
            mat_idx : mat_idx,
        }
    }
    /// Loads the chunk if there is saved data for it, or creates it via worldgen if not. 
    /// Note: These are chunk positions, not voxel positions. 
    pub fn load_or_create_c(&mut self, x : i32, y : i32, z : i32) {
        //TODO: extract worldgen out into its own thing
        //If we can load this chunk from disk, we do not need to generate it.
        //All loading / adding-to-list tasks are taken care of in self.load_c(x,y,z)).
        self.load_c(x,y,z); //So if it's loaded, we'll return in the next block.
        if (self.chunk_list.contains_key(&VoxelPos{ x : x, y : y, z : z})) {
            return;
        }
        println!("Generating chunk at {}, {}, {}", x, y, z);
        let air_mat = self.mat_idx.for_name(&String::from("test.air"));
        let stone_mat = self.mat_idx.for_name(&String::from("test.stone"));
        let dirt_mat = self.mat_idx.for_name(&String::from("test.dirt"));
        let grass_mat = self.mat_idx.for_name(&String::from("test.grass"));

        let mut chunk = Chunk::new_chunk(air_mat.clone());
        /*if (z > 0) { 
            //Do nothing, leave it as exclusively air.
        }*/

        if (z == 0) {
            testworldgen_surface(&mut chunk, air_mat.clone(), stone_mat.clone(), dirt_mat.clone(), grass_mat.clone());
        }
        if (z < 0) {
            testworldgen_underground(&mut chunk, air_mat.clone(), stone_mat.clone());
        }
        self.chunk_list.insert(VoxelPos{ x : x, y : y, z : z}, *chunk);
    }
    /// Loads the chunk if there is saved data for it, or creates it via worldgen if not. 
    /// Note: These are voxel positions, not chunk positions.
    pub fn load_or_create(&mut self, x : i32, y : i32, z : i32) {
        let c = select_chunk(x,y,z);
        self.load_or_create_c(c.x, c.y, c.z);
    }
    /// Tells you if we have loaded a chunk yet or not.
    /// Note: These are chunk positions, not voxel positions.
    pub fn is_loaded_c(&self, x : i32, y : i32, z : i32) -> bool {
        self.chunk_list.contains_key(&VoxelPos{x : x, y : y, z : z})
    }
    /// Tells you if we have loaded a chunk yet or not.
    pub fn is_loaded(&self, x : i32, y : i32, z : i32) -> bool {
        let c = select_chunk(x,y,z);
        return self.is_loaded_c(c.x, c.y, c.z);
    }

    /// Loads a chunk by chunk position.
    /// Returns true if the chunk exists to load.
    fn load_c(&mut self, x : i32, y : i32, z : i32) -> bool {
        let display_path = format!("map/c{}x{}y{}z.bin", x, y, z);
        let chunk_path = Path::new(&display_path);
        match OpenOptions::new()
            .read(true)
            .open(&chunk_path) {
            // The `description` method of `io::Error` returns a string that
            // describes the error
            //let result_chunk = Option<
            Err(_) => return false, //Chunk does not exist previously.
            Ok(mut file) => {
                println!("Loading chunk at {}, {}, {}", x, y, z);
                let mut allocate = false;
                match self.chunk_list.get(&VoxelPos{x : x, y : y, z : z}) {
                    None => {
                        allocate = true;
                    }, //We don't have a chunk for this position, create a new one and load it from disk.
                    Some(chunk) => { //We have a chunk for this position, load the data over it.
                        allocate = false;
                    },
                }
                if(allocate == true) { 
                    let mut chunk = Chunk::new_chunk(MaterialID::from_name(&String::from("test.air")));
                    chunk.load(&mut file);
                    self.chunk_list.insert(VoxelPos{ x : x, y : y, z : z}, *chunk);
                }
                else {
                    let mut chunk = self.chunk_list.get_mut(&VoxelPos{x : x, y : y, z : z}).unwrap();
                    chunk.load(&mut file);
                }
                return true;
            },
        };
    }
    /// Loads a location which includes this voxel.
    /// Note: These are voxel positions, not chunk positions.
    pub fn load(&mut self, x : i32, y : i32, z : i32) {
        let c = select_chunk(x,y,z);
        self.load_c(c.x, c.y, c.z);
    }
    /// Saves a chunk.
    /// Note: These are chunk positions, not voxel positions.
    fn save_c(&self, x : i32, y : i32, z : i32) {
        let display_path = format!("map/c{}x{}y{}z.bin", x, y, z);
        let chunk_path = Path::new(&display_path);
        println!("Saving chunk at {}, {}, {}", x, y, z);
        match OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&chunk_path) {
            // The `description` method of `io::Error` returns a string that
            // describes the error
            Err(why) => println!("couldn't open {}: {}", display_path, Error::description(&why)),
            Ok(mut file) => {
                let chunk_maybe = self.chunk_list.get(&VoxelPos{x : x, y : y, z : z});
                match chunk_maybe {
                    None => return, //We don't have a chunk for this position, we cannot save it.
                    Some(chunk) => {
                        chunk.save(&mut file);
                    },
                }
            },
        };
    }

    /// Saves a location which includes this voxel.
    /// Note: These are voxel positions, not chunk positions.
    pub fn save(&self, x : i32, y : i32, z : i32) {
        let c = select_chunk(x,y,z);
        self.save_c(c.x, c.y, c.z);
    }

    /// Saves all currently loaded terrain for this voxel space.
    pub fn save_all(&self) { 
        for (pos, chunk) in &self.chunk_list {
            self.save_c(pos.x, pos.y, pos.z);
        }
    }

    /// Saves and unloads a chunk.
    /// Note: These are chunk positions, not voxel positions.
    pub fn unload_c(&mut self, x : i32, y : i32, z : i32) {
        self.save_c(x,y,z);
        self.chunk_list.remove(&VoxelPos{ x : x, y : y, z : z});
    }

    /// Saves and unloads a location corresponding to this voxel.
    /// Note: These are voxel positions, not chunk positions.
    pub fn unload(&mut self, x : i32, y : i32, z : i32) {
        let c = select_chunk(x,y,z);
        self.unload_c(c.x, c.y, c.z);
    }

    pub fn unload_all(&mut self) {
        let temp_list = self.chunk_list.clone();
        for (pos, chunk) in temp_list { 
            self.unload_c(pos.x, pos.y, pos.z);
        }
    }

    /// Gets a list of areas full of valid voxels.
    pub fn get_regions(&self) -> Vec<VoxelRange<i32>> {
        let mut ret : Vec<VoxelRange<i32>>  = Vec::new();
        for (pos, chunk) in &self.chunk_list { 
            let x = pos.x * CHUNK_X_LENGTH as i32;
            let y = pos.y * CHUNK_Y_LENGTH as i32;
            let z = pos.z * CHUNK_Z_LENGTH as i32;
            
            let current = VoxelRange { 
                lower : VoxelPos { x : x, y : y, z : z, }, 
                upper : VoxelPos { x : x + CHUNK_X_LENGTH as i32, y : y + CHUNK_Y_LENGTH as i32, z : z + CHUNK_Z_LENGTH as i32 },
            };
            ret.push(current);
        }
        return ret;
    }
}

impl VoxelStorage<MaterialID, i32> for VoxelSpace {
    fn get(&self, x: i32, y: i32, z: i32) -> Option<MaterialID> {
        let chunk_pos = select_chunk(x,y,z);
        let chunk_maybe = self.chunk_list.get(&chunk_pos);
        match chunk_maybe {
            None => return Some(self.not_loaded_val.clone()), //We don't have a chunk for this position, so it's either not loaded or not generated.
            Some(chunk) => {
                return chunk.getv(pos_to_local(VoxelPos{x:x,y:y,z:z}, chunk_pos));
            }
        }
    }

    fn set(&mut self, x: i32, y: i32, z: i32, value: MaterialID) {
        let chunk_pos = select_chunk(x,y,z);
        let mut chunk_maybe = self.chunk_list.get_mut(&chunk_pos);
        //Not sure what the best way to handle errors here is. Shouldn't panic.
        if(chunk_maybe.is_some()) {
            let mut chunk = chunk_maybe.unwrap();
            chunk.setv(pos_to_local(VoxelPos{x:x,y:y,z:z}, chunk_pos), value.clone());
        }
    }
}

fn gen_test_space(range : VoxelRange<i32>) -> VoxelSpace {
    let mat_idx : MaterialIndex = MaterialIndex::new();

    let air_id : MaterialID = mat_idx.for_name(&String::from("Air"));
    let stone_id : MaterialID = mat_idx.for_name(&String::from("Stone"));
    let dirt_id : MaterialID = mat_idx.for_name(&String::from("Dirt"));
    let grass_id : MaterialID = mat_idx.for_name(&String::from("Grass"));
    
    let mut space = VoxelSpace::new(mat_idx);
    for pos in range {
        space.load_or_create_c(pos.x, pos.y, pos.z);
    }
    return space;
}

#[test]
fn test_space_gen() { 
   
    let lower : VoxelPos<i32> = VoxelPos{x : -2, y : -2, z : -2};
    let upper : VoxelPos<i32> = VoxelPos{x : 2, y : 2, z : 2};
    let range : VoxelRange<i32> = VoxelRange{lower : lower, upper : upper};
    let mut space = gen_test_space(range);

    
    for pos in range {
        assert!(space.is_loaded_c(pos.x,pos.y,pos.z));
    }
}
#[test]
fn test_get_ranges() {
   
    let lower : VoxelPos<i32> = VoxelPos{x : -2, y : -2, z : -2};
    let upper : VoxelPos<i32> = VoxelPos{x : 2, y : 2, z : 2};
    let chunk_range : VoxelRange<i32> = VoxelRange{lower : lower, upper : upper};
    let mut space = gen_test_space(chunk_range);
    
    let mut count = 0;
    for pos in chunk_range {
        assert!(space.is_loaded_c(pos.x,pos.y,pos.z));
        count = count + 1;
    }
    
    let regions = space.get_regions();
    assert_eq!(regions.len(), count);

    for reg in regions {
        for pos in reg { 
            assert!(space.getv(pos).is_some());
            assert!( space.getv(pos).unwrap() != space.not_loaded_val );
        }
    }
}
