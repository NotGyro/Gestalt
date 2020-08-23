use crate::world::tile::TileID;
//use ustr::{ustr, Ustr, UstrMap};
use hashbrown::HashMap;
use hashbrown::hash_map::*;

// Dependencies for testing
use rand::Rng;
use rand::thread_rng;
use ustr::*;

pub const CHUNK_SZ : usize = 32;
pub const CHUNK_SQUARED : usize = 1024;
pub const CHUNK_VOLUME : usize = 32768;
//The length of each chunk side is 2^5.
pub const CHUNK_EXP : usize = 5;

#[inline(always)] 
pub fn chunk_xyz_to_i(x : usize, y : usize, z : usize) -> usize {
    (z * CHUNK_SQUARED) + (y * CHUNK_SZ) + x
}

#[inline(always)] 
pub fn chunk_i_to_xyz(i : usize) -> (usize, usize, usize) {
    let x = i % CHUNK_SZ;
    let z = (i-x)/CHUNK_SQUARED; //The remainder on this (the y value) just gets thrown away, which is good here.
    let y = (i - (z * CHUNK_SQUARED))/CHUNK_SZ;
    (x, y, z)
}

/// A smaller chunk structure for chunks which only need 255 unique values.
pub struct ChunkSmall {
    data: [u8; CHUNK_VOLUME],
    pub palette: [TileID; 256],
    reverse_palette: HashMap<TileID, u8>,
    highest_idx: u8,
}

impl ChunkSmall {
    #[inline(always)]
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> u8 {
        self.data[chunk_xyz_to_i(x, y, z)]
    }
    #[inline(always)]
    pub fn get(&self, x: usize, y : usize, z: usize) -> TileID {
        //Get our int data and use it as an index for our palette. Yay constant-time!  
        self.palette[self.data[chunk_xyz_to_i(x, y, z)] as usize]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, x: usize, y : usize, z: usize, value: u8) {
        self.data[chunk_xyz_to_i(x, y, z)] = value;
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: TileID) -> Option<u16> {
        self.reverse_palette.get(&tile).map( #[inline(always)] |i| *i as u16)
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<TileID> {
        if idx > 255 { return None };
        if idx > self.highest_idx as u16 { return None };
        Some(self.palette[idx as usize])
    }
    ///Use this chunk to construct a chunk with u16 tiles rather than u8 ones. 
    #[inline]
    pub fn expand(&self) -> ChunkLarge {
        let mut new_palette : HashMap<u16, TileID> = HashMap::new();
        for (i, entry) in self.palette.iter().enumerate() {
            new_palette.insert(i as u16, *entry);
        }
        let mut new_data : [u16; CHUNK_VOLUME] = [0; CHUNK_VOLUME];
        for (i, tile) in self.data.iter().enumerate() {
            new_data[i] = *tile as u16;
        }
        let mut new_reverse_palette : HashMap<TileID, u16> = HashMap::new();
        for (key, value) in self.reverse_palette.iter() {
            new_reverse_palette.insert(*key, *value as u16);
        }
        ChunkLarge { data: new_data,
            palette: new_palette,
            reverse_palette: new_reverse_palette,
        }
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index. 
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: TileID) -> Option<u16> {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette. 
                Some(*idx as u16)
            },
            None => {
                //We have run out of space.
                if self.highest_idx >= 255 { 
                    return None;
                }
                else { 
                    self.highest_idx += 1;
                    let idx = self.highest_idx;
                    self.palette[idx as usize] = tile;
                    self.reverse_palette.insert(tile, idx);
                    Some(idx as u16)
                }
            }
        }
    }
}

/// Medium chunk structure. 
pub struct ChunkLarge {
    pub data: [u16; CHUNK_VOLUME],
    pub palette: HashMap<u16, TileID>,
    pub reverse_palette: HashMap<TileID, u16>,
}

impl ChunkLarge {
    #[inline(always)]
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> u16 {
        self.data[chunk_xyz_to_i(x, y, z)]
    }
    #[inline(always)]
    pub fn get(&self, x: usize, y : usize, z: usize) -> TileID {
        //Get our int data and use it as an index for our palette. Yay constant-time!  
        self.palette[&self.data[chunk_xyz_to_i(x, y, z)]]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, x: usize, y : usize, z: usize, value: u16) {
        self.data[chunk_xyz_to_i(x, y, z)] = value;
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: TileID) -> Option<u16> {
        self.reverse_palette.get(&tile).map( #[inline(always)] |i| *i)
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<TileID> {
        self.palette.get(&idx).map( #[inline(always)] |i| *i)
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index. 
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: TileID) -> u16 {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette. 
                *idx as u16
            },
            None => {
                //We have run out of space.
                let next_idx : u16 = self.palette.len() as u16;
                self.palette.insert(next_idx, tile);
                self.reverse_palette.insert(tile, next_idx);
                next_idx
            }
        }
    }
}

pub enum ChunkInner {
    ///Chunk that is all one value (usually this is for chunks that are 100% air). Note that, after being converted, idx 0 maps to 
    Uniform(TileID),
    ///Chunk that maps palette to 8-bit values.
    Small(Box<ChunkSmall>),
    ///Chunk that maps palette to 16-bit values.
    Large(Box<ChunkLarge>),
}

pub struct Chunk {
    revision_number: u64,
    inner: ChunkInner,
}

impl Chunk {
    #[inline(always)]
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> usize {
        match &self.inner {
            ChunkInner::Uniform(_) => 0,
            ChunkInner::Small(inner) => inner.get_raw(x,y,z) as usize,
            ChunkInner::Large(inner) => inner.get_raw(x,y,z) as usize,
        }
    }
    #[inline(always)]
    pub fn get(&self, x: usize, y : usize, z: usize) -> TileID {
        match &self.inner{
            ChunkInner::Uniform(val) => *val, 
            ChunkInner::Small(inner) => inner.get(x,y,z),
            ChunkInner::Large(inner) => inner.get(x,y,z),
        }
    }
    #[inline(always)]
    pub fn set_raw(&mut self, x: usize, y : usize, z: usize, value: u16) {
        match &mut self.inner {
            //TODO: Smarter way of handling this case. Currently, just don't. 
            //I don't want to return a result type HERE for performance reasons.
            ChunkInner::Uniform(_) => if value != 0 { panic!("Attempted to set_raw() on a Uniform chunk!")}, 
            ChunkInner::Small(inner) => inner.set_raw(x,y,z, value as u8),
            ChunkInner::Large(inner) => inner.set_raw(x,y,z, value),
        };
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: TileID) -> Option<u16> {
        match &self.inner {
            ChunkInner::Uniform(val) => { 
                if tile == *val { 
                    Some(0)
                }
                else { 
                    None
                }
            }, 
            ChunkInner::Small(inner) => inner.index_from_palette(tile),
            ChunkInner::Large(inner) => inner.index_from_palette(tile),
        }
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<TileID> {
        match &self.inner {
            ChunkInner::Uniform(val) => {
                if idx == 0 { 
                    Some(*val)
                }
                else { 
                    None
                }
            }, 
            ChunkInner::Small(inner) => inner.tile_from_index(idx),
            ChunkInner::Large(inner) => inner.tile_from_index(idx),
        }
    }
    #[inline]
    pub fn add_to_palette(&mut self, tile: TileID) -> u16 {
        match &mut self.inner {
            ChunkInner::Uniform(val) => {
                if tile == *val {
                    0
                }
                else {
                    // Convert to a ChunkSmall.
                    let mut data : [u8; CHUNK_VOLUME] = [0; CHUNK_VOLUME];
                    let mut palette : [TileID; 256] = [*val; 256];
                    palette[1] = tile;
                    let mut reverse_palette: HashMap<TileID, u8> = HashMap::new();
                    reverse_palette.insert(*val, 0);
                    reverse_palette.insert(tile, 1);
                    self.inner = ChunkInner::Small(Box::new(ChunkSmall {
                        data: data,
                        palette: palette,
                        reverse_palette: reverse_palette,
                        highest_idx: 1,
                    }));
                    1
                }
            }, 
            ChunkInner::Small(inner) => {
                match inner.add_to_palette(tile) {
                    Some(idx) => { 
                        idx
                    },
                    None => {
                        //We need to expand it. 
                        let mut new_inner = Box::new(inner.expand());
                        let idx = new_inner.add_to_palette(tile); //We just went from u8s to u16s, the ID space has quite certainly 
                        self.inner = ChunkInner::Large(new_inner);
                        idx
                    },
                }
            },
            ChunkInner::Large(inner) => inner.add_to_palette(tile),
        }
    }
    #[inline]
    pub fn set(&mut self, x: usize, y : usize, z: usize, tile: TileID) {
        let idx = self.add_to_palette(tile);
        //Did we just change something?
        if self.get(x, y, z) != tile {
            //Increment revision.
            self.revision_number += 1;
        }
        self.set_raw(x,y,z, idx)
    }
}

#[test]
fn chunk_index_reverse() {
    let mut rng = rand::thread_rng();
    for _ in 0..4096 {
        //println!("Iteration: {}", iter);

        let x = rng.gen_range(0, CHUNK_SZ);
        let y = rng.gen_range(0, CHUNK_SZ);
        let z = rng.gen_range(0, CHUNK_SZ); 

        let i_value = chunk_xyz_to_i(x, y, z);
        let (x1, y1, z1) = chunk_i_to_xyz(i_value);
        
        //println!("x: {}, y: {}, z: {}", x, y, z);
        //println!("x1: {}, y1: {}, z1: {}", x1, y1, z1);

        assert_eq!( x, x1 );
        assert_eq!( y, y1 );
        assert_eq!( z, z1 );
    }
}

#[test]
fn chunk_index_bounds() {
    for x in 0..CHUNK_SZ {
        for y in 0..CHUNK_SZ {
            for z in 0..CHUNK_SZ {
                assert!(chunk_xyz_to_i(x, y, z) < CHUNK_VOLUME);
            }
        }
    }
}

#[test]
fn assignemnts_to_chunk() {
    let u1 = Ustr::from("air");
    let u2 = Ustr::from("stone");
    let mut test_chunk = Chunk{revision_number: 0, inner: ChunkInner::Uniform(u1)};

    {
        test_chunk.set(1, 1, 1, u1);
        
        assert_eq!(test_chunk.get(1,1,1), u1);
    }

    if let ChunkInner::Uniform(_) = test_chunk.inner {} 
    else {
        assert!(false);
    }

    //Make sure Uniform chunks work the way they're supposed to. 
    
    for x in 0..CHUNK_SZ {
        for y in 0..CHUNK_SZ {
            for z in 0..CHUNK_SZ {
                assert_eq!(test_chunk.get(x,y,z), u1);
                //We should also be able to set every tile of the uniform to the uniform's value, and it'll do nothing.
                test_chunk.set(x,y,z, u1);
            }
        }
    }

    //Implicitly expand it to a Small chunk rather than a Uniform chunk. 
    {
        test_chunk.set(2, 2, 2, u2);

        assert_eq!(test_chunk.get(2,2,2), u2);
    }

    if let ChunkInner::Small(_) = test_chunk.inner {} 
    else {
        assert!(false);
    }

    //Make sure that our new ChunkSmall is still the Uniform's tile everywhere except the position where we assigned something else.
    for x in 0..CHUNK_SZ {
        for y in 0..CHUNK_SZ {
            for z in 0..CHUNK_SZ {
                if x == 2 && y == 2 && z == 2 {
                    assert_eq!(test_chunk.get(x,y,z), u2);
                }
                else { 
                    assert_eq!(test_chunk.get(x,y,z), u1);
                }
            }
        }
    }

    let mut rng = rand::thread_rng();

    {
        for i in 0..253 {
            
            let x = rng.gen_range(0, CHUNK_SZ);
            let y = rng.gen_range(0, CHUNK_SZ);
            let z = rng.gen_range(0, CHUNK_SZ); 

            let name = format!("{}.test",i);
            let tile = Ustr::from(name.as_str());

            test_chunk.set(x, y, z, tile);

            assert_eq!(test_chunk.get(x,y,z), tile);
        }
    }

    if let ChunkInner::Small(_) = test_chunk.inner {} 
    else {
        assert!(false);
    }

    //Make sure we can assign to everywhere in our chunk bounds.
    for x in 0..CHUNK_SZ {
        for y in 0..CHUNK_SZ {
            for z in 0..CHUNK_SZ {
                test_chunk.set(x,y,z, u1);
                assert_eq!(test_chunk.get(x,y,z), u1);
            }
        }
    }

    {
        for i in 253..1024 {
            
            let x = rng.gen_range(0, CHUNK_SZ);
            let y = rng.gen_range(0, CHUNK_SZ);
            let z = rng.gen_range(0, CHUNK_SZ); 

            let name = format!("{}.test",i);
            let tile = Ustr::from(name.as_str());
            
            test_chunk.set(x, y, z, tile);

            assert_eq!(test_chunk.get(x,y,z), tile);
        }
    }
    if let ChunkInner::Large(_) = test_chunk.inner {} 
    else {
        assert!(false);
    }
}