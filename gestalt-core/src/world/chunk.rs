use std::{mem::MaybeUninit, io::{Seek, SeekFrom, Write}};

use hashbrown::HashMap;
use log::warn;
use serde::{Serialize, Deserialize};

use crate::common::{voxelmath::*, Version};

use super::{
    voxelarray::{VoxelArrayStatic, VoxelArrayError}, voxelstorage::Voxel, VoxelStorage,
    VoxelStorageBounded, TileId,
};

pub const CHUNK_FILE_VERSION: Version = version!(0,0,1);

pub const CHUNK_EXP : usize = 5;
pub const CHUNK_SIZE: usize = 2_usize.pow(CHUNK_EXP as u32);
pub const CHUNK_SIZE_CUBED: usize = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;

pub const CHUNK_RANGE_USIZE: VoxelRange<usize> = VoxelRange {
    lower: vpos!(0, 0, 0),
    upper: vpos!(CHUNK_SIZE, CHUNK_SIZE, CHUNK_SIZE),
};

pub const CHUNK_RANGE_U16: VoxelRange<u16> = VoxelRange {
    lower: vpos!(0, 0, 0),
    upper: vpos!(CHUNK_SIZE as u16, CHUNK_SIZE as u16, CHUNK_SIZE as u16),
};

pub use super::voxelarray::{
    chunk_i_to_xyz, chunk_x_to_i_component, chunk_xyz_to_i, chunk_y_to_i_component,
    chunk_z_to_i_component, get_neg_x_offset, get_neg_y_offset, get_neg_z_offset, get_pos_x_offset,
    get_pos_y_offset, get_pos_z_offset,
};

#[derive(thiserror::Error, Debug, Clone)]
pub enum ChunkError {
    #[error("Error in underlying voxel array: {0:?}")]
    UnderlyingArrayError(#[from] VoxelArrayError),
}

pub struct ChunkSmall<T: Voxel> {
    //Attempting to use the constant causes Rust to freak out for some reason
    //so I simply type 16
    pub inner: VoxelArrayStatic<u8, 32>,
    pub palette: [T; 256],
    pub reverse_palette: HashMap<T, u8>,
    pub highest_idx: u8,
    // Used by the serializer to tell if the palette has changed.
    pub palette_dirty: bool,
}

impl<T: Voxel> ChunkSmall<T> {
    #[inline(always)]
    pub fn get_raw(&self, coord: VoxelPos<u16>) -> &u8 {
        //The intent here is so that bounds checking is only done ONCE for this structure.
        self.inner.get_raw(coord)
    }
    #[inline(always)]
    pub fn get_raw_i(&self, i: usize) -> &u8 {
        //The intent here is so that bounds checking is only done ONCE for this structure.
        self.inner.get_raw_i(i)
    }
    #[inline(always)]
    pub fn get(&self, coord: VoxelPos<u16>) -> &T {
        &self.palette[*self.get_raw(coord) as usize]
    }
    #[inline(always)]
    pub fn set_raw(&mut self, coord: VoxelPos<u16>, value: u8) {
        self.inner.set_raw(coord, value);
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: T) -> Option<u8> {
        self.reverse_palette.get(&tile).copied()
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<&T> {
        if idx > 255 {
            return None;
        };
        if idx > self.highest_idx as u16 {
            return None;
        };
        Some(&self.palette[idx as usize])
    }
    ///Use this chunk to construct a chunk with u16 tiles rather than u8 ones.
    #[inline]
    pub fn expand(&self) -> ChunkLarge<T> {
        let mut new_palette: HashMap<u16, T> = HashMap::new();
        for (i, entry) in self.palette.iter().enumerate() {
            new_palette.insert(i as u16, entry.clone());
        }
        let mut new_inner = VoxelArrayStatic::new(0);

        for i in 0..CHUNK_SIZE_CUBED {
            let tile = self.inner.get_raw_i(i);
            new_inner.set_raw_i(i, *tile as u16);
        }

        let mut new_reverse_palette: HashMap<T, u16> = HashMap::default();
        for (key, value) in self.reverse_palette.iter() {
            new_reverse_palette.insert(key.clone(), *value as u16);
        }
        ChunkLarge {
            inner: new_inner,
            palette: new_palette,
            reverse_palette: new_reverse_palette,
            palette_dirty: true,
        }
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index.
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: T) -> Option<u16> {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette.
                Some(*idx as u16)
            }
            None => {
                self.palette_dirty = true;
                //We have run out of space.
                if self.highest_idx == 255 {
                    None
                } else {
                    self.highest_idx += 1;
                    let idx = self.highest_idx;
                    self.palette[idx as usize] = tile.clone();
                    self.reverse_palette.insert(tile, idx);
                    Some(idx as u16)
                }
            }
        }
    }
}

//In a 16*16*16, a u16 encodes a number larger than the total number of possible voxel positions anyway.
pub struct ChunkLarge<T: Voxel> {
    //Attempting to use the constant causes Rust to freak out for some reason
    //so I simply type 16
    pub inner: VoxelArrayStatic<u16, 32>,
    pub palette: HashMap<u16, T>,
    pub reverse_palette: HashMap<T, u16>,
    pub palette_dirty: bool,
}

impl<T: Voxel> ChunkLarge<T> {
    #[inline(always)]
    pub fn get_raw(&self, coord: VoxelPos<u16>) -> &u16 {
        self.inner.get_raw(coord)
    }
    #[inline(always)]
    pub fn get(&self, coord: VoxelPos<u16>) -> &T {
        self.palette.get(self.inner.get_raw(coord)).unwrap()
    }
    #[inline(always)]
    pub fn get_raw_i(&self, i: usize) -> &u16 {
        //The intent here is so that bounds checking is only done ONCE for this structure.
        self.inner.get_raw_i(i)
    }
    #[inline(always)]
    pub fn set_raw(&mut self, coord: VoxelPos<u16>, value: u16) {
        self.inner.set_raw(coord, value);
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: T) -> Option<u16> {
        self.reverse_palette.get(&tile).copied()
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<&T> {
        self.palette.get(&idx)
    }
    /// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index.
    /// If it already exists, return the associated index. If we're out of room, return None.
    #[inline]
    pub fn add_to_palette(&mut self, tile: T) -> u16 {
        match self.reverse_palette.get(&tile) {
            Some(idx) => {
                //Already in the palette.
                *idx as u16
            }
            None => {
                self.palette_dirty = true;
                let next_idx: u16 = self.palette.len() as u16;
                self.palette.insert(next_idx, tile.clone());
                self.reverse_palette.insert(tile, next_idx);

                next_idx
            }
        }
    }
}

pub enum ChunkInner<T: Voxel> {
    ///Chunk that is all one value (usually this is for chunks that are 100% air). Note that, after being converted, idx 0 maps to
    Uniform(T),
    ///Chunk that maps palette to 8-bit values.
    Small(Box<ChunkSmall<T>>),
    ///Chunk that maps palette to 16-bit values.
    Large(Box<ChunkLarge<T>>),
}

pub struct Chunk<T: Voxel> {
    pub revision: u64,
    pub inner: ChunkInner<T>,
}

impl<T: Voxel> Chunk<T> {
    pub fn new(default_voxel: T) -> Self {
        Chunk {
            revision: 0,
            inner: ChunkInner::Uniform(default_voxel),
        }
    }

    #[inline(always)]
    pub fn get_raw_i(&self, i: usize) -> u16 {
        match &self.inner {
            ChunkInner::Uniform(_) => 0,
            ChunkInner::Small(inner) => *inner.get_raw_i(i) as u16,
            ChunkInner::Large(inner) => *inner.get_raw_i(i),
        }
    }

    #[inline(always)]
    pub fn get_raw(&self, pos: VoxelPos<u16>) -> u16 {
        match &self.inner {
            ChunkInner::Uniform(_) => 0,
            ChunkInner::Small(inner) => *inner.get_raw(pos) as u16,
            ChunkInner::Large(inner) => *inner.get_raw(pos),
        }
    }
    #[inline(always)]
    pub fn set_raw(&mut self, pos: VoxelPos<u16>, value: u16) {
        match &mut self.inner {
            //TODO: Smarter way of handling this case. Currently, just don't.
            //I don't want to return a result type HERE for performance reasons.
            ChunkInner::Uniform(_) => {
                if value != 0 {
                    panic!("Attempted to set_raw() on a Uniform chunk!")
                }
            }
            ChunkInner::Small(ref mut inner) => inner.set_raw(pos, value as u8),
            ChunkInner::Large(ref mut inner) => inner.set_raw(pos, value),
        };
    }
    #[inline(always)]
    pub fn index_from_palette(&self, tile: T) -> Option<u16> {
        match &self.inner {
            ChunkInner::Uniform(val) => {
                if tile == *val {
                    Some(0)
                } else {
                    None
                }
            }
            ChunkInner::Small(inner) => inner.index_from_palette(tile).map(|v| v as u16),
            ChunkInner::Large(inner) => inner.index_from_palette(tile),
        }
    }
    #[inline(always)]
    pub fn tile_from_index(&self, idx: u16) -> Option<&T> {
        match &self.inner {
            ChunkInner::Uniform(val) => {
                if idx == 0 {
                    Some(val)
                } else {
                    None
                }
            }
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
            ChunkInner::Uniform(_) => {}
            ChunkInner::Small(ref mut inner) => inner.palette_dirty = set_to,
            ChunkInner::Large(ref mut inner) => inner.palette_dirty = set_to,
        }
    }
    #[inline]
    pub fn add_to_palette(&mut self, tile: T) -> u16 {
        match &mut self.inner {
            ChunkInner::Uniform(val) => {
                if tile == *val {
                    0
                } else {
                    // Convert to a ChunkSmall.
                    let structure = VoxelArrayStatic::new(0); //0 will be *val

                    let mut palette: [T; 256] = unsafe {
                        let mut array: [T; 256] = std::mem::uninitialized();
                        for element in array.iter_mut() {
                            std::ptr::write(element, val.clone());
                        }
                        array
                    };
                    palette[1] = tile.clone();
                    let mut reverse_palette: HashMap<T, u8> = HashMap::default();
                    reverse_palette.insert(val.clone(), 0);
                    reverse_palette.insert(tile, 1);
                    self.inner = ChunkInner::Small(Box::new(ChunkSmall {
                        inner: structure,
                        palette,
                        reverse_palette,
                        highest_idx: 1,
                        palette_dirty: false,
                    }));
                    1
                }
            }
            ChunkInner::Small(inner) => {
                match inner.add_to_palette(tile.clone()) {
                    Some(idx) => idx,
                    None => {
                        //We need to expand it.
                        let mut new_inner = Box::new(inner.expand());
                        let idx = new_inner.add_to_palette(tile);
                        self.inner = ChunkInner::Large(new_inner);
                        idx
                    }
                }
            }
            ChunkInner::Large(inner) => inner.add_to_palette(tile),
        }
    }
}

impl Chunk<TileId> {
    //How many bytes will be needed to encode our palette? 
    pub fn calculate_bytes_for_palette(&self) -> usize { 
        match &self.inner {
            ChunkInner::Uniform(_) => { 
                SmallPaletteEntry::serialized_length()
            },
            ChunkInner::Small(_) => {
                SmallPaletteEntry::serialized_length()*256
            },
            ChunkInner::Large(inner) => {
                inner.palette.len() * LargePaletteEntry::serialized_length()
            },
        }
    }
    //How many bytes will be needed to encode our voxel data? 
    pub fn calculate_bytes_for_data(&self) -> usize { 
        match &self.inner {
            ChunkInner::Uniform(_) => { 
                0
            },
            ChunkInner::Small(_) => CHUNK_SIZE_CUBED * std::mem::size_of::<u8>(),
            ChunkInner::Large(_) => CHUNK_SIZE_CUBED * std::mem::size_of::<u16>(),
        }
    }

    /// Returns num bytes written.
    pub fn write_palette<W: Write + Seek>(&self, writer: &mut W) -> Result<usize, ChunkIoError> { 
        Ok(match &self.inner {
            ChunkInner::Uniform(value) => { 
                let entry = SmallPaletteEntry {
                    to_tile: *value,
                };
                let entry_bytes = entry.as_le_bytes();
                writer.write_all(&entry_bytes)?;
                entry_bytes.len()
            },
            ChunkInner::Small(inner) => {
                let mut total = 0;
                for value in inner.palette { 
                    let entry = SmallPaletteEntry {
                        to_tile: value,
                    };
                    let entry_bytes = entry.as_le_bytes();
                    writer.write_all(&entry_bytes)?;
                    total += entry_bytes.len();
                }
                total
            },
            ChunkInner::Large(inner) => {
                let mut total = 0;
                for (idx, tile) in inner.palette.iter() { 
                    let entry = LargePaletteEntry {
                        from_index: *idx,
                        to_tile: *tile,
                    };
                    let entry_bytes = entry.as_le_bytes();
                    writer.write_all(&entry_bytes)?;
                    total += entry_bytes.len();
                }
                total
            },
        })
    }
    /// Returns num bytes written.
    pub fn write_voxel_data<W: Write + Seek>(&self, writer: &mut W) -> Result<usize, ChunkIoError> { 
        Ok(match &self.inner {
            ChunkInner::Uniform(_) => {
                //Uniform has no "data" and skips this step
                0
            },
            ChunkInner::Small(inner) => {
                writer.write_all( &inner.inner.data)?;
                inner.inner.data.len()
            },
            ChunkInner::Large(inner) => {
                // We must please the capricious endianness gods.
                for value in inner.inner.data.iter() {
                    let bytes = value.to_le_bytes(); 
                    writer.write_all(&bytes)?;
                }
                inner.inner.data.len() * std::mem::size_of::<u16>()
            },
        })
    }

    pub fn write_chunk<W: Write + Seek>(&self, writer: &mut W) -> Result<(), ChunkIoError> {
        let mut header = ChunkFileHeader {
            variant: match &self.inner {
                ChunkInner::Uniform(_) => ChunkFileVariant::Uniform,
                ChunkInner::Small(smallchunk) => ChunkFileVariant::Small{highest_idx: smallchunk.highest_idx},
                ChunkInner::Large(_) => ChunkFileVariant::Large,
            },
            header_end_data_start: 3,
            data_end_palette_start: 3,
            palette_end: 3,
        };
        //Figure out how big this will be.
        let temp_header_vec = rmp_serde::to_vec(&header)?;
        let header_len = temp_header_vec.len() /* Just in case. */ + 128 + ChunkFilePreHeader::serialized_length();
        header.header_end_data_start = header_len as u32;
        header.data_end_palette_start = (header_len + self.calculate_bytes_for_data()) as u32;
        header.palette_end = (header_len + self.calculate_bytes_for_data() + self.calculate_bytes_for_palette()) as u32;
        // Remake our header vec with the proper numbers.
        let new_header_vec = rmp_serde::to_vec(&header)?;
        assert!(new_header_vec.len() <= header_len);

        let pre_header = ChunkFilePreHeader {
            version: CHUNK_FILE_VERSION,
            revision: self.revision,
            header_length: new_header_vec.len() as u32,
        };
        let pre_header_bytes = pre_header.to_le_bytes();
        writer.write_all(&pre_header_bytes)?;
        writer.write_all(&new_header_vec)?;

        writer.seek( SeekFrom::Start(header.header_end_data_start as u64) )?;
        let data_len = self.write_voxel_data(writer)?;
        writer.seek( SeekFrom::Start(header.data_end_palette_start as u64) )?;
        let palette_len = self.write_palette(writer)?;

        assert!( data_len <= self.calculate_bytes_for_data() );
        assert!( palette_len <= self.calculate_bytes_for_palette() );

        writer.flush()?;

        Ok(())
    }
}

impl<T: Voxel> VoxelStorage<T, u16> for Chunk<T> {
    type Error = VoxelArrayError;
    #[inline(always)]
    fn get(&self, pos: VoxelPos<u16>) -> Result<&T, VoxelArrayError> {
        match &self.inner {
            ChunkInner::Uniform(val) => Ok(val),
            ChunkInner::Small(inner) => Ok(inner.get(pos)),
            ChunkInner::Large(inner) => Ok(inner.get(pos)),
        }
    }
    #[inline]
    fn set(&mut self, pos: VoxelPos<u16>, tile: T) -> Result<(), VoxelArrayError> {
        let idx = self.add_to_palette(tile.clone());
        //Did we just change something?
        if self.get(pos)? != &tile {
            //Increment revision.
            self.revision += 1;
        }
        self.set_raw(pos, idx);

        Ok(())
    }
}

impl<T: Voxel> VoxelStorageBounded<T, u16> for Chunk<T> {
    fn get_bounds(&self) -> VoxelRange<u16> {
        VoxelRange {
            lower: vpos!(0, 0, 0),
            upper: vpos!(CHUNK_SIZE as u16, CHUNK_SIZE as u16, CHUNK_SIZE as u16),
        }
    }
}
#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
/// In a small-variant chunk, index is implicit.
pub struct SmallPaletteEntry { 
    to_tile: TileId,
}

impl SmallPaletteEntry { 
    //How many bytes will this take up on disk?
    pub const fn serialized_length() -> usize {
        std::mem::size_of::<TileId>()
    }
    fn as_le_bytes(&self) -> [u8; 4] {
        #[cfg(debug_assertions)]
        {
            assert_eq!(Self::serialized_length(), 4)
        } 

        self.to_tile.to_le_bytes()
    }
    fn from_le_bytes(bytes: [u8; 4]) -> Self { 
        #[cfg(debug_assertions)]
        {
            assert_eq!(Self::serialized_length(), 4)
        }

        let id = TileId::from_le_bytes(bytes);
        Self { 
            to_tile: id
        }
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct LargePaletteEntry { 
    from_index: u16,
    to_tile: TileId,
}

impl LargePaletteEntry { 
    //How many bytes will this take up on disk?
    pub const fn serialized_length() -> usize {
        std::mem::size_of::<u16>()
        + std::mem::size_of::<TileId>()
    }
    fn as_le_bytes(&self) -> [u8; 6] { 
        #[cfg(debug_assertions)]
        {
            assert_eq!(Self::serialized_length(), 6)
        }

        let index_bytes = self.from_index.to_le_bytes();
        let tile_bytes = self.to_tile.to_le_bytes();
        // Bytes for the index
        [ index_bytes[0], index_bytes[1],
            //Bytes for the actual tile it maps to
            tile_bytes[0],
            tile_bytes[1],
            tile_bytes[2], 
            tile_bytes[3]
        ]
    }
    fn from_le_bytes(bytes: [u8; 6]) -> Self { 
        #[cfg(debug_assertions)]
        {
            assert_eq!(Self::serialized_length(), 6)
        }
        let idx = u16::from_le_bytes([ 
            bytes[0],
            bytes[1],
        ]);
        let id = TileId::from_le_bytes([ 
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5]
        ]);
        Self { 
            from_index: idx,
            to_tile: id
        }
    }
}

pub fn deserialize_small_chunk_palette(buf: &[u8], highest_idx: u8) -> Result<[TileId; 256], ChunkIoError> {
    let mut output_buffer: [TileId; 256] = [TileId::default(); 256];

    // Make sure it divides evenly into palette entries. 
    let entry_size = SmallPaletteEntry::serialized_length();
    if buf.len() % entry_size != 0 { 
        return Err( ChunkIoError::PaletteSizeMismatch(buf.len(), entry_size) );
    }

    // How many entries do we have?
    let num_entries = buf.len()/entry_size;

    // Make sure we do not have more than 256 entries. 
    if num_entries > 256 { 
        return Err( ChunkIoError::InvalidPaletteLength(ChunkFileVariant::Small{ highest_idx }, buf.len()) );
    }

    for i in 0..num_entries { 
        let mut entry_bytes: [u8; SmallPaletteEntry::serialized_length()] = [0;SmallPaletteEntry::serialized_length()];
        entry_bytes.copy_from_slice(&buf[i*entry_size..(i+1)*entry_size] );
        let entry = SmallPaletteEntry::from_le_bytes(entry_bytes);
        output_buffer[i] = entry.to_tile;
    }

    Ok(output_buffer)
}

pub fn deserialize_large_chunk_palette(buf: &[u8]) -> Result<HashMap<u16, TileId>, ChunkIoError> {
    // Make sure it divides evenly into palette entries. 
    let entry_size = LargePaletteEntry::serialized_length();
    if buf.len() % entry_size != 0 { 
        return Err( ChunkIoError::PaletteSizeMismatch(buf.len(), entry_size) );
    }

    // How many entries do we have?
    let num_entries = buf.len()/entry_size;

    // Wait until this point to initialize our output buffer,
    // so that we can make an educated guess at how big of a HashMap we'll need.
    let mut output: HashMap<u16, TileId> = HashMap::with_capacity(num_entries);

    for i in 0..num_entries { 
        let mut entry_bytes: [u8; LargePaletteEntry::serialized_length()] = [0;LargePaletteEntry::serialized_length()];
        entry_bytes.copy_from_slice(&buf[i*entry_size..(i+1)*entry_size] );
        let LargePaletteEntry{ from_index, to_tile } = LargePaletteEntry::from_le_bytes(entry_bytes);
        output.insert(from_index, to_tile);
    }

    Ok(output)
}

pub fn deserialize_small_chunk_voxel_data<R: std::io::BufRead>(reader: &mut R) -> Result<[u8; CHUNK_SIZE_CUBED], ChunkIoError> {
    let output = unsafe {
        // Avoid writing CHUNK_SIZE_CUBED zeroes - we will need to tell Rust to do things it considers evil. 
        let mut buffer =  MaybeUninit::<[u8; CHUNK_SIZE_CUBED]>::uninit();
        let ptr = { &mut *buffer.as_mut_ptr() };
        reader.read_exact(ptr)?;
        buffer.assume_init()
    };

    Ok(output)
}

#[allow(clippy::needless_range_loop)]
pub fn deserialize_large_chunk_voxel_data<R: std::io::BufRead>(reader: &mut R) -> Result<[u16; CHUNK_SIZE_CUBED], ChunkIoError> {
    let output = unsafe {
        // Avoid writing CHUNK_SIZE_CUBED zeroes - we will need to tell Rust to do things it considers evil. 
        let mut buffer =  MaybeUninit::<[u16; CHUNK_SIZE_CUBED]>::uninit();
        let ptr = { &mut *buffer.as_mut_ptr() };
        // Read these things individually to ensure endianness isn't mangled. TODO: find a way to optimize this that doesn't break with endianness stuff.
        for i in 0..CHUNK_SIZE_CUBED { 
            let mut voxel_bytes: [u8; std::mem::size_of::<u16>()] = [0; std::mem::size_of::<u16>()];
            reader.read_exact(&mut voxel_bytes)?;
            let v = u16::from_le_bytes(voxel_bytes);
            //Write to our buffer. (...indirectly.)
            ptr[i] = v; 
        }
        buffer.assume_init()
    };

    Ok(output)
}

#[derive(thiserror::Error, Debug)]
pub enum ChunkIoError {
    #[error("Chunk IO error: Header end (data start) minus data end (palette start) was a negative value. Cannot have a negative-sized buffer. Start was {0} and end was {1}")]
    NegativeDataSize(u32, u32),
    #[error("Chunk IO error: Palette start (data end) minus Palette end was a negative value. Cannot have a negative-sized buffer. Start was {0} and end was {1}")]
    NegativePaletteSize(u32, u32),
    #[error("Data length was invalid for this type of chunk. Chunk is of type {0:?} and the data was {1} bytes in size")]
    InvalidDataLength(ChunkFileVariant, usize),
    #[error("Palette length was invalid for this type of chunk. Chunk is of type {0:?} and the buffer was {1} bytes in size")]
    InvalidPaletteLength(ChunkFileVariant, usize),
    #[error("Palette does not divide cleanly into palette entries! Buffer is {0} and entry size is {1}")]
    PaletteSizeMismatch(usize, usize),
    #[error("Voxel data does not divide cleanly into large chunk voxel indicies! Buffer is {0} and large chunks use u16 (2-byte) elements.")]
    LargeChunkVoxelSizeMismatch(usize),
    #[error("Palette buffer was declared as {0} bytes but the end of the file is {1} bytes away from the start of the palette.")]
    PaletteTooBigForFile(usize, usize),
    #[error("VoxelData buffer was declared as {0} bytes but the end of the file is {1} bytes away from the start of the voxel data.")]
    DataTooBigForFile(usize, usize),
    #[error("Zero-sized data for non-Uniform chunk variant! Variant was actually {0:?}")]
    ZeroSizedData(ChunkFileVariant),
    #[error("Rust Standard Library I/O caught an error: {0:?}.")]
    IoError(#[from] std::io::Error),
    #[error("Error decoding header via MessagePack: {0:?}.")]
    DecodeError(#[from] rmp_serde::decode::Error),
    #[error("Error encoding header via MessagePack: {0:?}.")]
    EncodeError(#[from] rmp_serde::encode::Error),
}
///"Plain old bytes" descriptive information before the messagepack header.
pub struct ChunkFilePreHeader { 
    version: Version,
    /// How many changes have been made to this chunk? 0 implies the chunk is exactly as it was spit out of the world generator. 
    revision: u64,
    /// Length of the "ChunkFileHeader" that comes after this PreHeader. 
    header_length: u32,
}

impl ChunkFilePreHeader { 
    pub fn new(header_length: usize, revision: u64) -> Self { 
        Self {
            version: CHUNK_FILE_VERSION,
            header_length: header_length as u32,
            revision
        }
    }
    pub const fn serialized_length() -> usize { 
        std::mem::size_of::<u128>() + std::mem::size_of::<u64>() + std::mem::size_of::<u32>()
    }
    pub fn to_le_bytes(&self) -> [u8; Self::serialized_length()] { 
        let mut out_buffer = [0; Self::serialized_length()];
        let version_bytes = self.version.as_bytes();
        out_buffer[0..std::mem::size_of::<u128>()].copy_from_slice(&version_bytes);

        let revision_bytes = self.revision.to_le_bytes();
        out_buffer[std::mem::size_of::<u128>()..std::mem::size_of::<u128>()+std::mem::size_of::<u64>()].copy_from_slice(&revision_bytes);
        
        let header_len_bytes = self.header_length.to_le_bytes();
        out_buffer[std::mem::size_of::<u128>()+std::mem::size_of::<u64>()..std::mem::size_of::<u128>()+std::mem::size_of::<u64>()+std::mem::size_of::<u32>()].copy_from_slice(&header_len_bytes);
        out_buffer
    }
    pub fn from_le_bytes(bytes: &[u8; Self::serialized_length()]) -> Self { 
        let mut version_buffer = [0;std::mem::size_of::<u128>()];
        version_buffer.copy_from_slice(&bytes[0..std::mem::size_of::<u128>()]);
        let mut revision_buffer = [0;std::mem::size_of::<u64>()];
        revision_buffer.copy_from_slice(&bytes[std::mem::size_of::<u128>() .. std::mem::size_of::<u128>() + std::mem::size_of::<u64>()]);
        let mut header_len_buffer = [0;std::mem::size_of::<u32>()];
        header_len_buffer.copy_from_slice(&bytes[std::mem::size_of::<u128>() + std::mem::size_of::<u64>() .. std::mem::size_of::<u128>() + std::mem::size_of::<u64>() + std::mem::size_of::<u32>()]);

        let version = Version::from_bytes(&version_buffer);
        let revision = u64::from_le_bytes(revision_buffer);
        let header_length = u32::from_le_bytes(header_len_buffer);
        Self { 
            version, 
            revision,
            header_length
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct ChunkFileHeader {
    variant: ChunkFileVariant,
    header_end_data_start: u32,
    data_end_palette_start: u32,
    palette_end: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[repr(u8)]
pub enum ChunkFileVariant{ 
    Uniform,
    Small{ highest_idx: u8 },
    Large,
}

impl ChunkFileVariant {
    pub fn valid_palette_len(&self, length: usize) -> bool { 
        match self {
            ChunkFileVariant::Uniform => length == SmallPaletteEntry::serialized_length(),
            ChunkFileVariant::Small{highest_idx: _} => length <= (SmallPaletteEntry::serialized_length() * 256),
            //You can have as many LargePaletteEntry as you want, but the bytes you're trying to load must divide cleanly into LargePaletteEntries.
            ChunkFileVariant::Large => (length % LargePaletteEntry::serialized_length() ) == 0,
        }
    }
    pub fn valid_data_len(&self, length: usize) -> bool { 
        match self {
            // Uniform has no "voxel data" to speak of. 
            ChunkFileVariant::Uniform => length == 0,
            ChunkFileVariant::Small{highest_idx: _} => length == CHUNK_SIZE_CUBED*std::mem::size_of::<u8>(),
            ChunkFileVariant::Large => length == CHUNK_SIZE_CUBED*std::mem::size_of::<u16>(),
        }
    }
}

impl ChunkFileHeader { 
    fn validate_data_size(&self) -> Result<usize, ChunkIoError> { 
        let data_len_signed: i64 = (self.data_end_palette_start as i64) - (self.header_end_data_start as i64);
        if data_len_signed < 0 { 
            return Err(ChunkIoError::NegativeDataSize(self.header_end_data_start, self.data_end_palette_start))
        }
        let data_len = data_len_signed as usize;
        if !self.variant.valid_data_len(data_len) { 
            return Err(ChunkIoError::InvalidDataLength(self.variant.clone(), data_len))
        }
        Ok(data_len)
    }
    fn validate_palette_size(&self) -> Result<usize, ChunkIoError> { 
        let palette_len_signed: i64 = (self.palette_end as i64) - (self.data_end_palette_start as i64);
        if palette_len_signed < 0 { 
            return Err(ChunkIoError::NegativePaletteSize(self.data_end_palette_start, self.palette_end))
        }
        let palette_len = palette_len_signed as usize;
        if !self.variant.valid_palette_len(palette_len) { 
            return Err(ChunkIoError::InvalidPaletteLength(self.variant.clone(), palette_len))
        }
        Ok(palette_len)
    }
    pub fn validate(&self) -> Result<(), ChunkIoError> {
        let _data_len = self.validate_data_size()?;
        let _palette_len = self.validate_palette_size()?;

        Ok(())
    } 
    pub fn get_data_size(&self) -> usize {
        self.validate_data_size().unwrap()
    }
    pub fn get_palette_size(&self) -> usize {
        self.validate_palette_size().unwrap()
    }
}

pub fn deserialize_chunk<R: std::io::BufRead + Seek>(reader: &mut R) -> Result<Chunk<TileId>, ChunkIoError> {
    let mut pre_header_bytes = [0u8; ChunkFilePreHeader::serialized_length()];
    reader.read_exact(&mut pre_header_bytes)?;

    let pre_header = ChunkFilePreHeader::from_le_bytes(&pre_header_bytes);

    if pre_header.version != CHUNK_FILE_VERSION { 
        //Do something?
        warn!("Our chunk file version is {:?} and we attempted to load a chunk which is version {:?}. Ignoring for now. (This behavior will probably change in later versions of the Gestalt engine.)", &pre_header.version, &CHUNK_FILE_VERSION);
    }

    // Read our header as a MessagePack message.
    let mut header_bytes = vec![0; pre_header.header_length as usize];
    reader.read_exact(&mut header_bytes)?;

    let header: ChunkFileHeader = rmp_serde::decode::from_slice(&header_bytes)?;
    drop(header_bytes);

    header.validate()?;
    let palette_size = header.get_palette_size();
    let _data_size = header.get_data_size();

    let variant = header.variant.clone();

    match variant {
        ChunkFileVariant::Uniform => {
            //Data should be zero sized. Skip reading data.
            reader.seek(SeekFrom::Start(header.data_end_palette_start as u64) )?;
            let mut palette_buf = vec![0;palette_size];
            reader.read_exact(&mut palette_buf[0..palette_size])?;
            let palette = deserialize_small_chunk_palette(palette_buf.as_slice(), 0)?;
            
            let value = palette[0];
            Ok(Chunk{ 
                inner: ChunkInner::Uniform(value),
                revision: pre_header.revision,
            })
        },
        ChunkFileVariant::Small { highest_idx } => {
            reader.seek(SeekFrom::Start(header.header_end_data_start as u64) )?;
            let data = deserialize_small_chunk_voxel_data(reader)?;

            reader.seek(SeekFrom::Start(header.data_end_palette_start as u64) )?;
            let mut palette_buf = vec![0;palette_size];
            reader.read_exact(&mut palette_buf[0..palette_size])?;
            let palette = deserialize_small_chunk_palette(palette_buf.as_slice(), highest_idx)?;

            let mut reverse_palette = HashMap::new();
            for (i, elem) in palette.iter().enumerate() { 
                reverse_palette.insert(*elem, i as u8);
            }

            Ok(Chunk{ 
                inner: ChunkInner::Small(
                    Box::new(
                        ChunkSmall {
                            inner: VoxelArrayStatic::load_new(data),
                            palette,
                            reverse_palette,
                            highest_idx,
                            palette_dirty: true,
                        }
                    )
                ),
                revision: pre_header.revision,
            })
        },
        ChunkFileVariant::Large => {
            reader.seek(SeekFrom::Start(header.header_end_data_start as u64) )?;
            let data = deserialize_large_chunk_voxel_data(reader)?;

            reader.seek(SeekFrom::Start(header.data_end_palette_start as u64) )?;
            let mut palette_buf = vec![0;palette_size];
            reader.read_exact(&mut palette_buf[0..palette_size])?;
            let palette = deserialize_large_chunk_palette(palette_buf.as_slice())?;

            let mut reverse_palette = HashMap::new();
            for (i, elem) in palette.iter() { 
                reverse_palette.insert(*elem, *i);
            }

            Ok(Chunk{ 
                inner: ChunkInner::Large(
                    Box::new(
                        ChunkLarge {
                            inner: VoxelArrayStatic::load_new(data),
                            palette,
                            reverse_palette,
                            palette_dirty: true,
                        }
                    )
                ),
                revision: pre_header.revision,
            })
        },
    }
}


#[test]
fn chunk_index_reverse() {
    use rand::Rng;

    let mut rng = rand::thread_rng();
    for _ in 0..4096 {
        let x = rng.gen_range(0, CHUNK_SIZE);
        let y = rng.gen_range(0, CHUNK_SIZE);
        let z = rng.gen_range(0, CHUNK_SIZE);

        let i_value = crate::world::voxelarray::chunk_xyz_to_i(x, y, z, CHUNK_SIZE);
        let (x1, y1, z1) = crate::world::voxelarray::chunk_i_to_xyz(i_value, CHUNK_SIZE);

        assert_eq!(x, x1);
        assert_eq!(y, y1);
        assert_eq!(z, z1);
    }
}

#[test]
fn chunk_index_bounds() {
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                assert!(
                    crate::world::voxelarray::chunk_xyz_to_i(x, y, z, CHUNK_SIZE)
                        < CHUNK_SIZE_CUBED
                );
            }
        }
    }
}

#[test]
fn assignemnts_to_chunk() {
    use rand::Rng;

    let u1 = String::from("air");
    let u2 = String::from("stone");
    let mut test_chunk = Chunk {
        revision: 0,
        inner: ChunkInner::Uniform(u1.clone()),
    };

    {
        test_chunk.set(vpos!(1, 1, 1), u1.clone()).unwrap();

        assert_eq!(test_chunk.get(vpos!(1, 1, 1)).unwrap(), &u1);
    }

    let mut valid_result = false;
    if let ChunkInner::Uniform(_) = test_chunk.inner {
        valid_result = true;
    }
    assert!(valid_result);

    //Make sure Uniform chunks work the way they're supposed to.

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let pos = vpos!(x as u16, y as u16, z as u16);
                assert_eq!(test_chunk.get(pos).unwrap(), &u1);
                //We should also be able to set every tile of the uniform to the uniform's value, and it'll do nothing.
                test_chunk.set(pos, u1.clone()).unwrap();
            }
        }
    }

    //Implicitly expand it to a Small chunk rather than a Uniform chunk.
    {
        test_chunk.set(vpos!(2, 2, 2), u2.clone()).unwrap();

        assert_eq!(test_chunk.get(vpos!(2, 2, 2)).unwrap(), &u2);
    }

    let mut valid_result = false;
    if let ChunkInner::Small(_) = test_chunk.inner {
        valid_result = true;
    }
    assert!(valid_result);

    //Make sure that our new ChunkSmall is still the Uniform's tile everywhere except the position where we assigned something else.
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let pos = vpos!(x as u16, y as u16, z as u16);
                if x == 2 && y == 2 && z == 2 {
                    assert_eq!(test_chunk.get(pos).unwrap(), &u2);
                } else {
                    assert_eq!(test_chunk.get(pos).unwrap(), &u1);
                }
            }
        }
    }

    let mut rng = rand::thread_rng();

    {
        for i in 0..253 {
            let x = rng.gen_range(0, CHUNK_SIZE);
            let y = rng.gen_range(0, CHUNK_SIZE);
            let z = rng.gen_range(0, CHUNK_SIZE);
            let pos = vpos!(x as u16, y as u16, z as u16);

            let tile = format!("{}.test", i);

            test_chunk.set(pos, tile.clone()).unwrap();

            assert_eq!(test_chunk.get(pos).unwrap(), &tile);
        }
    }

    let mut valid_result = false;
    if let ChunkInner::Small(_) = test_chunk.inner {
        valid_result = true;
    }
    assert!(valid_result);

    //Make sure we can assign to everywhere in our chunk bounds.
    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let pos = vpos!(x as u16, y as u16, z as u16);
                test_chunk.set(pos, u1.clone()).unwrap();
                assert_eq!(test_chunk.get(pos).unwrap(), &u1);
            }
        }
    }

    {
        for i in 253..1024 {
            let x = rng.gen_range(0, CHUNK_SIZE);
            let y = rng.gen_range(0, CHUNK_SIZE);
            let z = rng.gen_range(0, CHUNK_SIZE);
            let pos = vpos!(x as u16, y as u16, z as u16);

            let tile = format!("{}.test", i);

            test_chunk.set(pos, tile.clone()).unwrap();

            assert_eq!(test_chunk.get(pos).unwrap(), &tile);
        }
    }
    let mut valid_result = false;
    if let ChunkInner::Large(_) = test_chunk.inner {
        valid_result = true;
    }
    assert!(valid_result);
}

#[test]
fn chunk_serialize_deserialize() {
    use std::io::{BufWriter, Cursor};

    let air_block = 0;
    let stone_block = 37;

    let starting_chunk: Chunk<TileId> = Chunk {
        revision: 1337,
        inner: ChunkInner::Uniform(stone_block),
    };

    let mut buffer = Vec::default();
    
    // Serialize
    {
        let mut buf_writer = BufWriter::new(Cursor::new(&mut buffer));
        starting_chunk.write_chunk(&mut buf_writer).unwrap();
    }

    let len_uniform_buffer = buffer.len();

    drop(starting_chunk);

    let mut chunk = deserialize_chunk(&mut Cursor::new(&mut buffer)).unwrap();
    
    drop(buffer);

    // Make sure nothing got corrupted.
    for pos in chunk.get_bounds() { 
        assert_eq!(*chunk.get(pos).unwrap(), stone_block)
    }
    assert_eq!(chunk.revision, 1337);

    // Let's modify the chunk a bit, getting a Small variant chunk
    for pos in chunk.get_bounds() { 
        if pos.x % 2 == 0 { 
            chunk.set(pos, air_block).unwrap();
        }
    }

    let mut buffer = Vec::default();
    // Serialize
    {
        let mut buf_writer = BufWriter::new(Cursor::new(&mut buffer));
        chunk.write_chunk(&mut buf_writer).unwrap();
    }
    drop(chunk);

    let len_small_buffer = buffer.len();
    assert!(len_small_buffer > len_uniform_buffer);

    // Deserialize it again. 
    let mut chunk = deserialize_chunk(&mut Cursor::new(&mut buffer)).unwrap();
    drop(buffer);

    // We didn't corrupt it or anything, did we? 
    for pos in chunk.get_bounds() { 
        if pos.x % 2 == 0 { 
            assert_eq!(*chunk.get(pos).unwrap(), air_block);
        }
    }
    
    // Let's modify the chunk a bit, getting a Large variant chunk
    for pos in chunk.get_bounds() {
        let value = (((((pos.x % 16) as usize) * (pos.y % 16) as usize) * (pos.z % 16) as usize) % 4096) as TileId;
        chunk.set(pos, value).unwrap();
    }

    let mut buffer = Vec::default();
    // Serialize
    {
        let mut buf_writer = BufWriter::new(Cursor::new(&mut buffer));
        chunk.write_chunk(&mut buf_writer).unwrap();
    }
    drop(chunk);

    let len_large_buffer = buffer.len();
    assert!(len_large_buffer > len_small_buffer);

    // Deserialize it again. 
    let chunk = deserialize_chunk(&mut Cursor::new(&mut buffer)).unwrap();
    drop(buffer);

    for pos in chunk.get_bounds() { 
        let value = ((((pos.x % 16) as usize) * ((pos.y % 16) as usize) * ((pos.z % 16) as usize)) % 4096) as TileId;
        assert_eq!(*chunk.get(pos).unwrap(), value);
    }
}