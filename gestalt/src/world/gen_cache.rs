//! Module for storing, querying, and loading default chunk states. These are cached locally for
//! performance, but they aren't transmitted over the network (clients always generate and cache
//! default chunk states before loading changesets from a server.)

use std::fs::{File, create_dir_all};
use std::io::{Write, Read};
use std::path::Path;

use crate::world::{Chunk, CHUNK_SIZE, CHUNK_SCALE};
use crate::voxel::subdivstorage::{SubdivSource, NaiveVoxelOctree, SubdivDrain};
use crate::voxel::subdivstorage::SubdivNode::Leaf;


pub fn write_chunk_to_disk(seed: u32, chunk: &Chunk, pos: (i32, i32, i32)) {
    let world_name = String::from("test_world");
    let path: String = format!("worlds/{}/gencache/{}/", world_name, seed);
    let filename: String = format!("{}.{}.{}.gen", pos.0, pos.1, pos.2);
    create_dir_all(path.clone()).unwrap();
    let mut file = File::create(path+&filename).unwrap();
    let mut data = [0u8; CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE];
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let idx = z + (y * CHUNK_SIZE) + (x * CHUNK_SIZE * CHUNK_SIZE);
                let node = chunk.data.get(opos!((x, y, z) @ 0)).unwrap();
                if let Leaf(l) = node {
                    data[idx] = l;
                }
                else {
                    panic!();
                }
            }
        }
    }
    file.write_all(&data).unwrap();
}

pub fn load_chunk_from_disk(seed: u32, pos: (i32, i32, i32)) -> Option<NaiveVoxelOctree<u8, ()>> {
    let world_name = String::from("test_world");
    let path: String = format!("worlds/{}/gencache/{}/", world_name, seed);
    let filename: String = format!("{}.{}.{}.gen", pos.0, pos.1, pos.2);
    if Path::new("does_not_exist.txt").exists() {
        let mut file = File::open(path+&filename).unwrap();
        let mut data = [0u8; CHUNK_SIZE*CHUNK_SIZE*CHUNK_SIZE];
        file.read(&mut data).unwrap();
        let mut tree = NaiveVoxelOctree::new(0, CHUNK_SCALE);
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let idx = z + (y * CHUNK_SIZE) + (x * CHUNK_SIZE * CHUNK_SIZE);
                    tree.set(opos!((x, y, z) @ 0), data[idx]).unwrap();
                }
            }
        }
        Some(tree)
    }
    else {
        None
    }
}