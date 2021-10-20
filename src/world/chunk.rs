use crate::world::tile::TileId;
use crate::common::voxelmath::*;
//use u16::{u16, u16, u16Map};
use hashbrown::HashMap;

// Dependencies for testing
use std::error::Error;
use std::io::{Write, Seek};
use semver::Version;

pub const SERIALIZED_CHUNK_VERSION_MAJOR: u64 = 0;
pub const SERIALIZED_CHUNK_VERSION_MINOR: u64 = 1;
pub const SERIALIZED_CHUNK_VERSION_PATCH: u64 = 0;

custom_error!{ pub ChunkSerializeError
    VersionMismatch{attempted_load_ver: Version, our_ver: Version}
     = "Attempted to load a chunk of version {attempted_load_ver} into our chunk with version {our_ver}",
    InvalidType{ty_id: u8} = "Attempted to load chunk type {ty_id}, which is not supported.",
}

pub const CHUNK_SZ : usize = 32;
pub const CHUNK_SQUARED : usize = 1024;
pub const CHUNK_VOLUME : usize = 32768;
//The length of each chunk side is 2^5.
pub const CHUNK_EXP : usize = 5;

pub const CHUNK_RANGE : VoxelRange<i32> = VoxelRange{lower:vpos!(0,0,0), upper:vpos!(32,32,32)};
pub const CHUNK_RANGE_USIZE : VoxelRange<usize> = VoxelRange{lower:vpos!(0,0,0), upper:vpos!(32,32,32)};

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
    pub data: Vec<u8>,
    pub palette: [TileId; 256],
    pub reverse_palette: HashMap<u16, u8>,
    pub highest_idx: u8,
    // Used by the serializer to tell if the palette has changed.
    pub palette_dirty: bool,
}

impl ChunkSmall {
    #[inline(always)]
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> u8 {
        self.data[chunk_xyz_to_i(x, y, z)]
    }
    #[inline(always)]
    pub fn get(&self, x: usize, y : usize, z: usize) -> TileId {
        //Get our int data and use it as an index for our palette. Yay constant-time!  
        self.palette[self.data[chunk_xyz_to_i(x, y, z)] as usize]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, x: usize, y : usize, z: usize, value: u8) {
        self.data[chunk_xyz_to_i(x, y, z)] = value;
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: TileId) -> Option<u16> {
        self.reverse_palette.get(&tile).map( #[inline(always)] |i| *i as u16)
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<TileId> {
        if idx > 255 { return None };
        if idx > self.highest_idx as u16 { return None };
        Some(self.palette[idx as usize])
    }
    ///Use this chunk to construct a chunk with u16 tiles rather than u8 ones. 
    #[inline]
    pub fn expand(&self) -> ChunkLarge {
        let mut new_data : Vec<u16> = vec![0; CHUNK_VOLUME];
        for (i, tile) in self.data.iter().enumerate() {
            new_data[i] = self.palette[*tile as usize];
        }
        ChunkLarge { data: new_data, }
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index. 
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: TileId) -> Option<u16> {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette. 
                Some(*idx as u16)
            },
            None => {
                self.palette_dirty = true;
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
    pub data: Vec<u16>,
}

impl ChunkLarge {
    #[inline(always)]
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> u16 {
        self.data[chunk_xyz_to_i(x, y, z)]
    }
    #[inline(always)]
    pub fn get(&self, x: usize, y : usize, z: usize) -> TileId {
        //Get our int data and use it as an index for our palette. Yay constant-time!  
        self.data[chunk_xyz_to_i(x, y, z)]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, x: usize, y : usize, z: usize, value: u16) {
        self.data[chunk_xyz_to_i(x, y, z)] = value;
    }
}

pub enum ChunkInner {
    ///Chunk that is all one value (usually this is for chunks that are 100% air). Note that, after being converted, idx 0 maps to 
    Uniform(TileId),
    ///Chunk that maps palette to 8-bit values.
    Small(Box<ChunkSmall>),
    ///Chunk that maps palette to 16-bit values.
    Large(Box<ChunkLarge>),
}

pub struct Chunk {
    pub revision: u64,
    pub inner: ChunkInner,
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
    pub fn get(&self, x: usize, y : usize, z: usize) -> TileId {
        match &self.inner{
            ChunkInner::Uniform(val) => *val, 
            ChunkInner::Small(inner) => inner.get(x,y,z),
            ChunkInner::Large(inner) => inner.get(x,y,z),
        }
    }
    #[inline(always)]
    pub fn getv(&self, pos: VoxelPos<usize>) -> TileId {
        self.get(pos.x, pos.y, pos.z)
    }
    #[inline(always)]
    pub fn set_raw(&mut self, x: usize, y : usize, z: usize, value: u16) {
        match &mut self.inner {
            //TODO: Smarter way of handling this case. Currently, just don't. 
            //I don't want to return a result type HERE for performance reasons.
            ChunkInner::Uniform(_) => if value != 0 { panic!("Attempted to set_raw() on a Uniform chunk!")}, 
            ChunkInner::Small(ref mut inner) => inner.set_raw(x,y,z, value as u8),
            ChunkInner::Large(ref mut inner) => inner.set_raw(x,y,z, value),
        };
    }
    #[inline]
    pub fn add_to_palette(&mut self, tile: TileId) -> u16 {
        match &mut self.inner {
            ChunkInner::Uniform(val) => {
                if tile == *val {
                    tile
                }
                else {
                    // Convert to a ChunkSmall.
                    let data : Vec<u8> = vec![*val as u8; CHUNK_VOLUME];

                    let mut palette : [TileId; 256] = [*val; 256];
                    palette[1] = tile;
                    let mut reverse_palette: HashMap<u16, u8> = HashMap::default();
                    reverse_palette.insert(*val, 0);
                    reverse_palette.insert(tile, 1);
                    self.inner = ChunkInner::Small(Box::new(ChunkSmall {
                        data: data,
                        palette: palette,
                        reverse_palette: reverse_palette,
                        highest_idx: 1,
                        palette_dirty: false,
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
                        let new_inner = Box::new(inner.expand());
                        let idx = tile; //We just went from u8s to u16s, the ID space has quite certainly 
                        self.inner = ChunkInner::Large(new_inner);
                        idx
                    },
                }
            },
            ChunkInner::Large(_) => tile,
        }
    }
    #[inline]
    pub fn set(&mut self, x: usize, y : usize, z: usize, tile: TileId) {
        let idx = self.add_to_palette(tile);
        //Did we just change something?
        if self.get(x, y, z) != tile {
            //Increment revision.
            self.revision += 1;
        }
        self.set_raw(x,y,z, idx)
    }
    #[inline(always)]
    pub fn setv(&mut self, pos: VoxelPos<usize>, tile: TileId) {
        self.set(pos.x, pos.y, pos.z, tile);
    }

    // ======= Serialization code below. =======
    pub fn serialize_header<W: Write + Seek>(&self, writer: &mut W) -> Result<usize, Box<dyn Error>> {
        //--- Header ---
        //The header gets to be fixed size.
        //Header:
        //    Version: 
        //        u64 major - 8 bytes
        //        u64 minor - 8 bytes
        //        u64 patch - 8 bytes
        //    u64 type/flags - 8 bytes
        //    u64 revision number. - 8 bytes
        const MAGIC_SIZE_NUMBER : usize = 40;

        //Write version - must come first.
        writer.write(&SERIALIZED_CHUNK_VERSION_MAJOR.to_le_bytes())?;
        writer.write(&SERIALIZED_CHUNK_VERSION_MINOR.to_le_bytes())?;
        writer.write(&SERIALIZED_CHUNK_VERSION_PATCH.to_le_bytes())?;
        
        //8 bits for type of chunk (more than we'll ever need but I want to keep it byte-aligned for simplicity)
        let ty = match self.inner {
            ChunkInner::Uniform(_) => 0, 
            ChunkInner::Small(_) => 1,
            ChunkInner::Large(_) => 2,
        };
        let flags : u64 = 0 + ty;
        
        writer.write(&flags.to_le_bytes())?;

        //--- Revision ---
        writer.write(&self.revision.to_le_bytes())?;
        Ok(MAGIC_SIZE_NUMBER)
    }
}


#[test]
fn chunk_index_reverse() {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    for _ in 0..4096 {

        let x = rng.gen_range(0..CHUNK_SZ);
        let y = rng.gen_range(0..CHUNK_SZ);
        let z = rng.gen_range(0..CHUNK_SZ); 

        let i_value = chunk_xyz_to_i(x, y, z);
        let (x1, y1, z1) = chunk_i_to_xyz(i_value);

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
    
    use rand::Rng;

    let u1 = 0;
    let u2 = 1;
    let mut test_chunk = Chunk{revision: 0, inner: ChunkInner::Uniform(u1)};

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
            
            let x = rng.gen_range(0..CHUNK_SZ);
            let y = rng.gen_range(0..CHUNK_SZ);
            let z = rng.gen_range(0..CHUNK_SZ); 

            let tile = i + 2;

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
            
            let x = rng.gen_range(0..CHUNK_SZ);
            let y = rng.gen_range(0..CHUNK_SZ);
            let z = rng.gen_range(0..CHUNK_SZ); 

            let tile = i + 2;
            
            test_chunk.set(x, y, z, tile);

            assert_eq!(test_chunk.get(x,y,z), tile);
        }
    }
    if let ChunkInner::Large(_) = test_chunk.inner {} 
    else {
        assert!(false);
    }
}