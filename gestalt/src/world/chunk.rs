use crate::world::tile::TileId;
use crate::common::voxelmath::*;
//use ustr::{ustr, Ustr, UstrMap};
use hashbrown::HashMap;

use std::error::Error;
use std::io::{Read, Write, Seek, SeekFrom};
use semver::Version;

use ustr::*;

pub const SERIALIZED_CHUNK_VERSION_MAJOR: u64 = 0;
pub const SERIALIZED_CHUNK_VERSION_MINOR: u64 = 1;
pub const SERIALIZED_CHUNK_VERSION_PATCH: u64 = 0;

custom_error!{ pub ChunkSerializeError
    VersionMismatch{attempted_load_ver: Version, our_ver: Version}
     = "Attempted to load a chunk of version {attempted_load_ver} into our chunk with version {our_ver}",
    InvalidType{ty_id: u8} = "Attempted to load chunk type {ty_id}, which is not supported.",
}

pub const CHUNK_EXP : usize = 5;
pub const CHUNK_SZ : usize = 2usize.pow(CHUNK_EXP as u32);
pub const CHUNK_SQUARED : usize = CHUNK_SZ*CHUNK_SZ;
pub const CHUNK_VOLUME : usize = CHUNK_SZ*CHUNK_SZ*CHUNK_SZ;

pub const CHUNK_RANGE : VoxelRange<i32> = VoxelRange{lower:vpos!(0,0,0), upper:vpos!(CHUNK_SZ as i32,CHUNK_SZ as i32,CHUNK_SZ as i32)};
pub const CHUNK_RANGE_USIZE : VoxelRange<usize> = VoxelRange{lower:vpos!(0,0,0), upper:vpos!(CHUNK_SZ,CHUNK_SZ,CHUNK_SZ)};

#[inline(always)] 
pub fn chunk_x_to_i_component(x : usize) -> usize {
    x
}
#[inline(always)] 
pub fn chunk_y_to_i_component(y : usize) -> usize {
    y * CHUNK_SZ
}
#[inline(always)] 
pub fn chunk_z_to_i_component(z : usize) -> usize {
    z * CHUNK_SQUARED
}

#[inline(always)] 
pub fn chunk_xyz_to_i(x : usize, y : usize, z : usize) -> usize {
    chunk_z_to_i_component(z) + chunk_y_to_i_component(y) + chunk_x_to_i_component(x)
}

#[inline(always)]
pub fn chunk_i_to_xyz(i : usize) -> (usize, usize, usize) {
    let z = i/CHUNK_SQUARED;
    let y = (i-z*CHUNK_SQUARED)/CHUNK_SZ;
    let x = i - ((z*CHUNK_SQUARED) + (y*CHUNK_SZ));
    (x, y, z)
}


#[inline(always)]
pub fn get_pos_x_offset(i : usize) -> Option<usize> {
    if (i + chunk_x_to_i_component(1) < CHUNK_VOLUME) && (chunk_i_to_xyz(i).0 + 1 < CHUNK_SZ) {
        Some(i + chunk_x_to_i_component(1))
    }
    else {
        None 
    }
}
#[inline(always)]
pub fn get_neg_x_offset(i : usize) -> Option<usize> {
    if chunk_i_to_xyz(i).0.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_x_to_i_component(1))
}
#[inline(always)]
pub fn get_pos_y_offset(i : usize) -> Option<usize> {
    if (i + chunk_y_to_i_component(1) < CHUNK_VOLUME) && (chunk_i_to_xyz(i).1 + 1 < CHUNK_SZ)  {
        Some(i + chunk_y_to_i_component(1))
    }
    else {
        None 
    }
}
#[inline(always)]
pub fn get_neg_y_offset(i : usize) -> Option<usize> {
    if chunk_i_to_xyz(i).1.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_y_to_i_component(1))
}
#[inline(always)]
pub fn get_pos_z_offset(i : usize) -> Option<usize> {
    if (i + chunk_z_to_i_component(1) < CHUNK_VOLUME) && (chunk_i_to_xyz(i).2 + 1 < CHUNK_SZ) {
        Some(i + chunk_z_to_i_component(1))
    }
    else {
        None 
    }
}
#[inline(always)]
pub fn get_neg_z_offset(i : usize) -> Option<usize> {
    if chunk_i_to_xyz(i).2.checked_sub(1).is_none() {
        return None;
    }
    i.checked_sub(chunk_z_to_i_component(1))
}

/// A smaller chunk structure for chunks which only need 255 unique values.
pub struct ChunkSmall {
    pub data: Vec<u8>,
    pub palette: Vec<TileId>,
    pub reverse_palette: UstrMap<u8>,
    // Used by the serializer to tell if the palette has changed.
    pub palette_dirty: bool,
}

impl ChunkSmall {
    #[inline(always)]
    pub fn get_raw_i(&self, i: usize) -> u8 {
        self.data[i]
    }
    #[inline(always)]
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> u8 {
        self.get_raw_i(chunk_xyz_to_i(x, y, z))
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
    pub fn set_raw_i(&mut self, i: usize, value: u8) {
        self.data[i] = value;
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: TileId) -> Option<u16> {
        self.reverse_palette.get(&tile).map( #[inline(always)] |i| *i as u16)
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<TileId> {
        if idx > 255 { return None };
        if idx > self.palette.len() as u16 { return None };
        Some(self.palette[idx as usize])
    }
    ///Use this chunk to construct a chunk with u16 tiles rather than u8 ones. 
    #[inline]
    pub fn expand(&self) -> ChunkLarge {
        let mut new_palette : HashMap<u16, TileId> = HashMap::new();
        for (i, entry) in self.palette.iter().enumerate() {
            new_palette.insert(i as u16, *entry);
        }
        let mut new_data : Vec<u16> = vec![0; CHUNK_VOLUME];
        for (i, tile) in self.data.iter().enumerate() {
            new_data[i] = *tile as u16;
        }
        let mut new_reverse_palette : UstrMap<u16> = UstrMap::default();
        for (key, value) in self.reverse_palette.iter() {
            new_reverse_palette.insert(*key, *value as u16);
        }
        ChunkLarge { data: new_data,
            palette: new_palette,
            reverse_palette: new_reverse_palette,
            palette_dirty: true,
        }
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
                if self.palette.len() >= 255 { 
                    return None;
                }
                else { 
                    let idx = self.palette.len();
                    self.palette.push(tile);
                    self.reverse_palette.insert(tile, idx as u8);
                    Some(idx as u16)
                }
            }
        }
    }
    #[inline(always)]
    pub fn palette_length(&self) -> usize {
        self.palette.len()
    }
}

/// Medium chunk structure. 
pub struct ChunkLarge {
    pub data: Vec<u16>,
    pub palette: HashMap<u16, TileId>,
    pub reverse_palette: UstrMap<u16>,
    pub palette_dirty: bool,
}

impl ChunkLarge {
    #[inline(always)]
    pub fn get_raw_i(&self, i: usize) -> u16 {
        self.data[i]
    }
    #[inline(always)]
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> u16 {
        self.get_raw_i(chunk_xyz_to_i(x, y, z))
    }
    #[inline(always)]
    pub fn get(&self, x: usize, y : usize, z: usize) -> TileId {
        //Get our int data and use it as an index for our palette. Yay constant-time!  
        self.palette[&self.data[chunk_xyz_to_i(x, y, z)]]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, x: usize, y : usize, z: usize, value: u16) {
        self.data[chunk_xyz_to_i(x, y, z)] = value;
    }
    #[inline(always)]
    pub fn set_raw_i(&mut self, i: usize, value: u16) {
        self.data[i] = value;
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: TileId) -> Option<u16> {
        self.reverse_palette.get(&tile).map( #[inline(always)] |i| *i)
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<TileId> {
        self.palette.get(&idx).map( #[inline(always)] |i| *i)
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index. 
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: TileId) -> u16 {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette. 
                *idx as u16
            },
            None => {
                if self.palette.len() >= (u16::MAX as usize) { 
                    self.garbage_collect_palette();
                }
                self.palette_dirty = true;
                let next_idx : u16 = self.palette.len() as u16;
                self.palette.insert(next_idx, tile);
                self.reverse_palette.insert(tile, next_idx);
                next_idx
            }
        }
    }
    //Gets rid of unused palette entries 
    pub fn garbage_collect_palette(&mut self) {
        let mut keep_list: ustr::UstrSet = ustr::UstrSet::default();
        for i in 0..CHUNK_VOLUME {
            let idx = self.data[i];
            let entry = *self.palette.get(&idx).unwrap();
            if ! keep_list.contains(&entry) {
                keep_list.insert(entry);
            }
        }
        self.palette.retain(|&_, &mut v| {
            keep_list.contains(&v)
        });
        self.reverse_palette.retain(|&k, &mut _| {
            keep_list.contains(&k)
        });
        self.palette_dirty = true;
    }
    #[inline(always)]
    pub fn palette_length(&self) -> usize {
        self.palette.len()
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
    pub fn get_raw(&self, x: usize, y : usize, z: usize) -> u16 {
        match &self.inner {
            ChunkInner::Uniform(_) => 0,
            ChunkInner::Small(inner) => inner.get_raw(x,y,z) as u16,
            ChunkInner::Large(inner) => inner.get_raw(x,y,z) as u16,
        }
    }
    #[inline(always)]
    pub fn get_raw_i(&self, i: usize) -> u16 {
        match &self.inner {
            ChunkInner::Uniform(_) => 0,
            ChunkInner::Small(inner) => inner.get_raw_i(i) as u16,
            ChunkInner::Large(inner) => inner.get_raw_i(i) as u16,
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

    #[inline(always)]
    pub fn set_raw_i(&mut self, i: usize, value: u16) {
        match &mut self.inner {
            //TODO: Smarter way of handling this case. Currently, just don't. 
            //I don't want to return a result type HERE for performance reasons.
            ChunkInner::Uniform(_) => if value != 0 { panic!("Attempted to set_raw() on a Uniform chunk!")}, 
            ChunkInner::Small(ref mut inner) => inner.set_raw_i(i, value as u8),
            ChunkInner::Large(ref mut inner) => inner.set_raw_i(i, value),
        };
    }

    #[inline(always)]
    pub fn index_from_palette(&self, tile: TileId) -> Option<u16> {
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
    pub fn tile_from_index(&self, idx: u16) -> Option<TileId> {
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
    #[inline(always)]
    pub fn is_palette_dirty(&self) -> bool {
        match &self.inner {
            ChunkInner::Uniform(_) => false,
            ChunkInner::Small(inner) => inner.palette_dirty,
            ChunkInner::Large(inner) => inner.palette_dirty,
        }
    }
    #[inline(always)]
    pub fn mark_palette_dirty_status(&mut self, set_to: bool) {
        match &mut self.inner {
            ChunkInner::Uniform(_) => {},
            ChunkInner::Small(ref mut inner) => inner.palette_dirty = set_to,
            ChunkInner::Large(ref mut inner) => inner.palette_dirty = set_to,
        }
    }
    #[inline]
    pub fn add_to_palette(&mut self, tile: TileId) -> u16 {
        match &mut self.inner {
            ChunkInner::Uniform(val) => {
                if tile == *val {
                    0
                }
                else {
                    // Convert to a ChunkSmall.
                    let data : Vec<u8> = vec![0; CHUNK_VOLUME];

                    let mut palette : Vec<TileId> = Vec::with_capacity(256);
                    palette.push(*val);
                    palette.push(tile);
                    assert_eq!(palette.len(), 2);


                    let mut reverse_palette: UstrMap<u8> = UstrMap::default();
                    reverse_palette.insert(*val, 0);
                    reverse_palette.insert(tile, 1);

                    //info!(Mesher, "Upgrading a chunk from uniform to small.");
                    //info!(Mesher, "Palette is {:?}", palette);

                    self.inner = ChunkInner::Small(Box::new(ChunkSmall {
                        data: data,
                        palette: palette,
                        reverse_palette: reverse_palette,
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

    #[inline(always)]
    pub fn palette_length(&self) -> usize {
        match &self.inner {
            ChunkInner::Uniform(_) => { 1 },
            ChunkInner::Small(inner) => inner.palette_length(),
            ChunkInner::Large(inner) => inner.palette_length(),
        }
    }

    #[inline(always)]
    pub fn garbage_collect_palette(&mut self) {
        match &mut self.inner {
            ChunkInner::Uniform(_) => { /* Not applicable */ },
            ChunkInner::Small(_) => { /* TODO */ },
            ChunkInner::Large(inner) => inner.garbage_collect_palette(),
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
    
    pub fn serialize_data<W: Write + Seek>(&self, writer: &mut W) -> Result<usize, Box<dyn Error>> {
        match &self.inner {
            // A "Uniform" chunk does not have any chunk data apart from palette idx 0 and its associated tile ID.
            ChunkInner::Uniform(_) => {return Ok(0)},
            ChunkInner::Small(inner) => {
                for value in inner.data.iter() {
                    writer.write(&[*value])?;
                }
                return Ok(CHUNK_VOLUME);
            },
            ChunkInner::Large(inner) => {
                for value in inner.data.iter() {
                    writer.write(&value.to_le_bytes())?;
                }
                return Ok(CHUNK_VOLUME * 2);
            },
        }
    }

    pub fn serialize_palette<W: Write + Seek>(&self, writer: &mut W) -> Result<usize, Box<dyn Error>> {
        //--- Palette ---
        let mut full_palette_data : Vec<u8> = Vec::new();
        let mut count : u16 = 0;
        match &self.inner {
            ChunkInner::Uniform(val) => {
                count=1;
                let mut idx_bytes : Vec<u8> = Vec::from((0 as u16).to_le_bytes());
                let mut name_bytes : Vec<u8> = Vec::from(val.as_str());
                let mut name_len_bytes : Vec<u8> = Vec::from((name_bytes.len() as u16).to_le_bytes());
                //Write the index.
                full_palette_data.append(&mut idx_bytes);
                //Write the length of our coming string.
                full_palette_data.append(&mut name_len_bytes);
                //Then, write the string.
                full_palette_data.append(&mut name_bytes);
            },
            ChunkInner::Small(inner) => {
                count = inner.palette.len() as u16;
                for (idx, _val) in inner.palette.iter().enumerate() {
                    let mut idx_bytes : Vec<u8> = Vec::from((idx as u16).to_le_bytes());
                    let mut name_bytes : Vec<u8> = Vec::from(inner.palette[idx].as_str());
                    let mut name_len_bytes : Vec<u8> = Vec::from((name_bytes.len() as u16).to_le_bytes());
                    //Write the index.
                    full_palette_data.append(&mut idx_bytes);
                    //Write the length of our coming string.
                    full_palette_data.append(&mut name_len_bytes);
                    //Then, write the string.
                    full_palette_data.append(&mut name_bytes);
                }
            },
            ChunkInner::Large(inner) => {
                for (key, value) in &inner.palette { 
                    count += 1;
                    let mut idx_bytes : Vec<u8> = Vec::from((*key as u16).to_le_bytes());
                    let mut name_bytes : Vec<u8> = Vec::from(value.as_str());
                    let mut name_len_bytes : Vec<u8> = Vec::from((name_bytes.len() as u16).to_le_bytes());
                    //Write the index.
                    full_palette_data.append(&mut idx_bytes);
                    //Write the length of our coming string.
                    full_palette_data.append(&mut name_len_bytes);
                    //Then, write the string.
                    full_palette_data.append(&mut name_bytes);
                }
            },
        }
        let mut total_written : usize = 0;
        let count_bytes = count.to_le_bytes();
        total_written += writer.write(&count_bytes)?;
        //Write total length of palette structure.
        total_written += writer.write(&(full_palette_data.len() as u64).to_le_bytes())?;
        //Write the palette data.
        total_written += writer.write(&full_palette_data)?;

        Ok(total_written)
    }

    pub fn serialize_full<W: Write + Seek>(&self, writer: &mut W) -> Result<usize, Box<dyn Error>> {
        let mut total : usize = 0;
        total += self.serialize_header(writer)?;
        writer.seek(SeekFrom::Start(total as u64))?;
        total += self.serialize_data(writer)?;
        writer.seek(SeekFrom::Start(total as u64))?;
        total += self.serialize_palette(writer)?;
        Ok(total)
    }

    // ======= Deserialization code below. =======
    
    pub fn deserialize<R: Read + Seek>(reader: &mut R) -> Result<Self, Box<dyn Error>> {
        let mut u64_buf : [u8; 8] = [0;8];
        
        reader.read_exact(&mut u64_buf)?;
        let major : u64 = u64::from_le_bytes(u64_buf);
        reader.read_exact(&mut u64_buf)?;
        let minor : u64 = u64::from_le_bytes(u64_buf);
        reader.read_exact(&mut u64_buf)?;
        let patch : u64 = u64::from_le_bytes(u64_buf);

        let version = semver::Version::from((major, minor, patch));
        let engine_version = semver::Version::from((SERIALIZED_CHUNK_VERSION_MAJOR, SERIALIZED_CHUNK_VERSION_MINOR, SERIALIZED_CHUNK_VERSION_PATCH));

        if version != engine_version {
            return Err(Box::new(ChunkSerializeError::VersionMismatch{attempted_load_ver: version, our_ver: engine_version}));
        }

        let mut u64_buf : [u8; 8] = [0;8];
        reader.read_exact(&mut u64_buf)?;
        let flags_and_type : u64 = u64::from_le_bytes(u64_buf);
        //TODO: When there are flags, separate this out.
        let ty = flags_and_type as u8;

        //ChunkInner::Uniform(_) => 0, 
        //ChunkInner::Small(_) => 1,
        //ChunkInner::Large(_) => 2,

        reader.read_exact(&mut u64_buf)?;
        let revision : u64 = u64::from_le_bytes(u64_buf);

        //End of header. Now we get to data and palette.

        fn read_palette_entry<RR: Read>(r: &mut RR) -> Result<(usize, u16, Ustr), Box<dyn Error>> {
            let mut total_read : usize = 0;
            //Read our index
            let mut u16_buf : [u8; 2] = [0;2];
            r.read_exact(&mut u16_buf)?;
            let index : u16 = u16::from_le_bytes(u16_buf);
            total_read += 2;

            //Read the size of the name coming up.
            r.read_exact(&mut u16_buf)?;
            let name_size = u16::from_le_bytes(u16_buf);
            total_read += 2;

            //Get ready to read the name / tile ID
            let mut name_buf = vec![0u8; name_size as usize];
            //Read the name and converty it.
            r.read_exact(&mut name_buf)?;
            let name = Ustr::from(std::str::from_utf8(name_buf.as_slice())?);

            total_read += name_size as usize;

            Ok((total_read, index, name))
        }

        Ok(match ty { 
            //Uniform
            0 => {
                let mut u16_buf : [u8; 2] = [0;2];
                reader.read_exact(&mut u16_buf)?;
                let palette_count : u16 = u16::from_le_bytes(u16_buf);
                assert_eq!(palette_count, 1);
                
                reader.read_exact(&mut u64_buf)?;
                let _palette_size : u64 = u64::from_le_bytes(u64_buf);

                let palette_entry = read_palette_entry::<R>(reader)?;
                //Index should be 0
                assert_eq!(palette_entry.1, 0);
                //Uniform chunk - skips data, palette is there right away (and only has one entry).
                Chunk{revision: revision, inner: ChunkInner::Uniform(palette_entry.2)}
            },
            //Small
            1 => {
                //----- Data -----
                let mut data: Vec<u8> = vec![0u8; CHUNK_VOLUME];
                reader.read_exact(&mut data)?;

                //----- Palette -----
                
                let mut u16_buf : [u8; 2] = [0;2];
                reader.read_exact(&mut u16_buf)?;
                let palette_count : u16 = u16::from_le_bytes(u16_buf);
                
                reader.read_exact(&mut u64_buf)?;
                let _palette_size : usize = u64::from_le_bytes(u64_buf) as usize;

                let mut palette: Vec<TileId> = vec![ustr("nil"); palette_count as usize];
                let mut reverse_palette: UstrMap<u8> = UstrMap::default();
                //The palette is at the end of the file, so read the rest of it.
                //4 bytes is the absolute minimum size of a palette entry. u16 idx, u16 name_length (which would be 0 if there's no string).
                for _ in 0..palette_count {
                    let (_, idx, value) = read_palette_entry(reader)?;
                    palette[idx as usize] = value;
                    reverse_palette.insert(value, idx as u8);
                }
                //Here, build a chunk to return.
                Chunk{revision: revision,
                    inner: ChunkInner::Small( Box::new(
                        ChunkSmall {
                            data: data,
                            palette: palette,
                            reverse_palette: reverse_palette,
                            palette_dirty: false,
                        }
                    ))
                }
            },
            //Large
            2 => {
                //----- Data -----
                let mut data: Vec<u16> = vec![0u16; CHUNK_VOLUME];
                for i in 0..CHUNK_VOLUME {
                    let mut u16_buf : [u8; 2] = [0;2];
                    reader.read_exact(&mut u16_buf)?;
                    data[i] = u16::from_le_bytes(u16_buf);
                }

                //----- Palette -----
                
                let mut u16_buf : [u8; 2] = [0;2];
                reader.read_exact(&mut u16_buf)?;
                let palette_count : u16 = u16::from_le_bytes(u16_buf);

                reader.read_exact(&mut u64_buf)?;
                let _palette_size_remaining : i32 = u64::from_le_bytes(u64_buf) as i32;

                let mut palette : HashMap<u16, TileId> = HashMap::new();

                let mut reverse_palette: UstrMap<u16> = UstrMap::default();
                //The palette is at the end of the file, so read the rest of it.
                //4 bytes is the absolute minimum size of a palette entry. u16 idx, u16 name_length (which would be 0 if there's no string).
                for _ in 0..palette_count {
                    let (_, idx, value) = read_palette_entry(reader)?;
                    palette.insert(idx, value);
                    reverse_palette.insert(value, idx);
                }
                //Here, build a chunk to return. 
                Chunk{revision: revision,
                    inner: ChunkInner::Large( Box::new(
                        ChunkLarge {
                            data: data,
                            palette: palette,
                            reverse_palette: reverse_palette,
                            palette_dirty: false,
                        }
                    ))
                }
            },
            _ => return Err(Box::new(ChunkSerializeError::InvalidType{ty_id: ty})),
        })
    }
    
    // ======= End of serialization stuff. =======
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;
    use std::io::Cursor;

    #[test]
    fn chunk_index_reverse() {
        let mut rng = rand::thread_rng();
        for _ in 0..4096 {

            let x = rng.gen_range(0, CHUNK_SZ);
            let y = rng.gen_range(0, CHUNK_SZ);
            let z = rng.gen_range(0, CHUNK_SZ); 

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
        let u1 = Ustr::from("air");
        let u2 = Ustr::from("stone");
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



    #[test]
    fn chunk_serialize_deserialize() {
        let u1 = Ustr::from("stone");
        let u2 = Ustr::from("steel");
        let test_chunk = Chunk{revision: 0, inner: ChunkInner::Uniform(u1)};

        //Serialize it as a uniform. 

        let mut buf : Cursor<Vec<u8>> = Cursor::new(Vec::new());
        test_chunk.serialize_full(&mut buf).unwrap();
        buf.seek(SeekFrom::Start(0)).unwrap();
        let mut chunk_copy = Chunk::deserialize(&mut buf).unwrap();

        if let ChunkInner::Uniform(val) = chunk_copy.inner {
            assert_eq!(val, u1);
        } 
        else {
            assert!(false);
        }

        //Implicitly expand it to a Small chunk rather than a Uniform chunk. 
        chunk_copy.set(2, 2, 2, u2);
        assert_eq!(chunk_copy.get(2,2,2), u2);
        
        if let ChunkInner::Small(_) = chunk_copy.inner {} 
        else {
            assert!(false);
        }

        let mut buf : Cursor<Vec<u8>> = Cursor::new(Vec::new());
        chunk_copy.serialize_full(&mut buf).unwrap();
        buf.seek(SeekFrom::Start(0)).unwrap();
        let mut chunk_copy = Chunk::deserialize(&mut buf).unwrap();

        if let ChunkInner::Small(_) = chunk_copy.inner {
            assert_eq!(chunk_copy.get(0,0,0), u1);
            assert_eq!(chunk_copy.get(2,2,2), u2);
        }
        else {
            assert!(false);
        }

        //Add a large number of new tile types to expand it implicitly to a Large chunk 
        let mut rng = rand::thread_rng();
        
        {
            for i in 0..1024 {
                
                let x = rng.gen_range(0, CHUNK_SZ);
                let y = rng.gen_range(0, CHUNK_SZ);
                let z = rng.gen_range(0, CHUNK_SZ); 

                let name = format!("{}.test",i);
                let tile = Ustr::from(name.as_str());

                chunk_copy.set(x, y, z, tile);

                assert_eq!(chunk_copy.get(x,y,z), tile);
            }
        }

        if let ChunkInner::Large(_) = chunk_copy.inner {} 
        else {
            assert!(false);
        }

        let mut buf : Cursor<Vec<u8>> = Cursor::new(Vec::new());
        chunk_copy.serialize_full(&mut buf).unwrap();
        buf.seek(SeekFrom::Start(0)).unwrap();
        let chunk_copy = Chunk::deserialize(&mut buf).unwrap();

        //If it's still Large, we're golden. 
        if let ChunkInner::Large(_) = chunk_copy.inner {} 
        else {
            assert!(false);
        }
    }
}

#[test]
fn test_garbage_collect_palette() {
    use crate::rand::Rng;

    let u1 = Ustr::from("air");
    let mut test_chunk = Chunk{revision: 0, inner: ChunkInner::Uniform(u1)};

    let mut rng = rand::thread_rng();

    {
        for i in 0..((u16::MAX as usize)+20) {
            
            let x = rng.gen_range(0, CHUNK_SZ);
            let y = rng.gen_range(0, CHUNK_SZ);
            let z = rng.gen_range(0, CHUNK_SZ); 

            let name = format!("{}.test",i);
            let tile = Ustr::from(name.as_str());

            test_chunk.set(x, y, z, tile);

            assert_eq!(test_chunk.get(x,y,z), tile);
        }
    }
    assert!(test_chunk.palette_length() < (u16::MAX as usize));

    for x in 0..CHUNK_SZ {
        for y in 0..CHUNK_SZ {
            for z in 0..CHUNK_SZ {
                test_chunk.set(x, y, z, u1)
            }
        }
    }
    test_chunk.garbage_collect_palette();
    assert_eq!(test_chunk.palette_length(), 1);
    
    test_chunk.set(7, 7, 7, Ustr::from("steel"));
    assert_eq!(test_chunk.palette_length(), 2);
}