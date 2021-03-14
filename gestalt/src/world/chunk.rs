use crate::world::tile::*;
use crate::common::voxelmath::*;
//use ustr::{ustr, Ustr, UstrMap};
use hashbrown::HashMap;

use std::error::Error;
use std::io::{Read, Write, Seek, SeekFrom};
use semver::Version;

pub const SERIALIZED_CHUNK_VERSION_MAJOR: u64 = 0;
pub const SERIALIZED_CHUNK_VERSION_MINOR: u64 = 1;
pub const SERIALIZED_CHUNK_VERSION_PATCH: u64 = 0;

custom_error!{ pub ChunkSerializeError
    VersionMismatch{attempted_load_ver: Version, our_ver: Version}
     = "Attempted to load a chunk of version {attempted_load_ver} into our chunk with version {our_ver}",
    InvalidType{ty_id: u8} = "Attempted to load chunk type {ty_id}, which is not supported.",
}

custom_error!{ pub ChunkVoxelError
    OutOfBounds{attempted_pos: VoxelPos<usize>, oursize: VoxelPos<usize>}
     = "Attempted to access position {attempted_pos} in a chunk with size {oursize}",
}

pub const CHUNK_EXP : usize = 5;
pub const CHUNK_SZ_X : usize = 2usize.pow(CHUNK_EXP as u32);
pub const CHUNK_SZ_Y : usize = 2usize.pow(CHUNK_EXP as u32);
pub const CHUNK_SZ_Z : usize = 2usize.pow(CHUNK_EXP as u32);
pub const CHUNK_VOLUME : usize = CHUNK_SZ_X*CHUNK_SZ_Y*CHUNK_SZ_Z;

pub const CHUNK_RANGE : VoxelRange<i32> = VoxelRange{lower:vpos!(0,0,0), upper:vpos!(CHUNK_SZ_X as i32,CHUNK_SZ_Y as i32,CHUNK_SZ_Z as i32)};
pub const CHUNK_RANGE_USIZE : VoxelRange<usize> = VoxelRange{lower:vpos!(0,0,0), upper:vpos!(CHUNK_SZ_X,CHUNK_SZ_Y,CHUNK_SZ_Z)};

/// The underlying storage for a standard voxel world chunk. Genericized because I may use this to store more than one type of voxel data. 
pub struct VoxelArray<T: ValidTile, const SIZE_X: usize, const SIZE_Y: usize, const SIZE_Z: usize> 
                            where [T; SIZE_X * SIZE_Y * SIZE_Z]: Sized {
    pub data: [T; SIZE_X * SIZE_Y * SIZE_Z],
}

impl<T: ValidTile, const SIZE_X: usize, const SIZE_Y: usize, const SIZE_Z: usize> VoxelArray<T, SIZE_X, SIZE_Y, SIZE_Z>
                                where [T; SIZE_X * SIZE_Y * SIZE_Z]: Sized {
    #[inline(always)] 
    pub fn chunk_x_to_i_component(x : usize) -> usize {
        x
    }
    #[inline(always)] 
    pub fn chunk_y_to_i_component(y : usize) -> usize {
        y * SIZE_X
    }
    #[inline(always)] 
    pub fn chunk_z_to_i_component(z : usize) -> usize {
        z * SIZE_X * SIZE_Y
    }

    #[inline(always)] 
    pub fn chunk_xyz_to_i(x : usize, y : usize, z : usize) -> usize {
        Self::chunk_z_to_i_component(z) + Self::chunk_y_to_i_component(y) + Self::chunk_x_to_i_component(x)
    }

    #[inline(always)]
    pub fn chunk_i_to_xyz(i : usize) -> (usize, usize, usize) {
        let z = i/SIZE_X*SIZE_Y;
        let y = (i-z*(SIZE_X*SIZE_Y))/SIZE_Z;
        let x = i - ((z*(SIZE_X*SIZE_Y)) + (y*SIZE_X));
        (x, y, z)
    }

    // Offset stuff. 
    #[inline(always)]
    pub fn get_pos_x_offset(i : usize) -> Option<usize> {
        if (i + Self::chunk_x_to_i_component(1) < SIZE_X * SIZE_Y * SIZE_Z) && (Self::chunk_i_to_xyz(i).0 + 1 < SIZE_X) {
            Some(i + Self::chunk_x_to_i_component(1))
        }
        else {
            None 
        }
    }
    #[inline(always)]
    pub fn get_neg_x_offset(i : usize) -> Option<usize> {
        if Self::chunk_i_to_xyz(i).0.checked_sub(1).is_none() {
            return None;
        }
        i.checked_sub(Self::chunk_x_to_i_component(1))
    }
    #[inline(always)]
    pub fn get_pos_y_offset(i : usize) -> Option<usize> {
        if (i + Self::chunk_y_to_i_component(1) < SIZE_X * SIZE_Y * SIZE_Z) && (Self::chunk_i_to_xyz(i).1 + 1 < SIZE_Y)  {
            Some(i + Self::chunk_y_to_i_component(1))
        }
        else {
            None 
        }
    }
    #[inline(always)]
    pub fn get_neg_y_offset(i : usize) -> Option<usize> {
        if Self::chunk_i_to_xyz(i).1.checked_sub(1).is_none() {
            return None;
        }
        i.checked_sub(Self::chunk_y_to_i_component(1))
    }
    #[inline(always)]
    pub fn get_pos_z_offset(i : usize) -> Option<usize> {
        if (i + Self::chunk_z_to_i_component(1) < SIZE_X * SIZE_Y * SIZE_Z) && (Self::chunk_i_to_xyz(i).2 + 1 < SIZE_Z) {
            Some(i + Self::chunk_z_to_i_component(1))
        }
        else {
            None 
        }
    }
    #[inline(always)]
    pub fn get_neg_z_offset(i : usize) -> Option<usize> {
        if Self::chunk_i_to_xyz(i).2.checked_sub(1).is_none() {
            return None;
        }
        i.checked_sub(Self::chunk_z_to_i_component(1))
    }

    //UNCHECKED offset stuff, to be fast with. Here be dragons. 
    #[inline(always)]
    pub fn get_pos_x_offset_unchecked(i : usize) -> usize {
        i + Self::chunk_x_to_i_component(1)
    }
    #[inline(always)]
    pub fn get_neg_x_offset_unchecked(i : usize) -> usize {
        i - Self::chunk_x_to_i_component(1)
    }
    
    #[inline(always)]
    pub fn get_pos_y_offset_unchecked(i : usize) -> usize {
        i + Self::chunk_y_to_i_component(1)
    }
    #[inline(always)]
    pub fn get_neg_y_offset_unchecked(i : usize) -> usize {
        i - Self::chunk_y_to_i_component(1)
    }

    #[inline(always)]
    pub fn get_pos_z_offset_unchecked(i : usize) -> usize {
        i + Self::chunk_z_to_i_component(1)
    }
    #[inline(always)]
    pub fn get_neg_z_offset_unchecked(i : usize) -> usize {
        i - Self::chunk_z_to_i_component(1)
    }

    #[inline(always)]
    pub fn get_i(&self, i: usize) -> T { 
        self.data[i]
    } 
    #[inline(always)]
    pub fn get_xyz(&self, x: usize, y: usize, z: usize) -> T { 
        self.get_i(Self::chunk_xyz_to_i(x, y, z))
    }
    #[inline(always)]
    pub fn set_i(&self, tile: &T, i: usize) { 
        self.data[i] = tile.clone();
    } 
    #[inline(always)]
    pub fn set_xyz(&self, tile: &T, x: usize, y: usize, z: usize){ 
        self.set_i(tile, Self::chunk_xyz_to_i(x, y, z));
    }

    pub fn new() -> Self {
        VoxelArray { 
            data: [T::default(); SIZE_X*SIZE_Y*SIZE_Z],
        }
    }
}

// Swizzle to Z X Y.
// +2 to each dimension for NEFARIOUS REASONS. (adjacency stuff)
// each voxel in a chunk which borders another chunk is duplicated, to make it easier to 
// write code which takes a neighborhood 
pub const CHUNK_INNER_SZ_1 : usize = CHUNK_SZ_Z+2;
pub const CHUNK_INNER_SZ_2 : usize = CHUNK_SZ_X+2;
pub const CHUNK_INNER_SZ_3 : usize = CHUNK_SZ_Y+2;

type ChunkUnderlying = VoxelArray<ChunkTileId, CHUNK_INNER_SZ_1, CHUNK_INNER_SZ_2, CHUNK_INNER_SZ_3>;

pub struct Chunk {
    /// Offset of this chunk from world position. 
    pub offset: VoxelPos<TileCoord>,
    inner: ChunkUnderlying,
}

impl Chunk {
    pub fn new(offset: &VoxelPos<TileCoord>) -> Self {
        Chunk { 
            offset: offset.clone(),
            inner: ChunkUnderlying::new(),
        }
    }

    #[inline(always)]
    pub fn xyz_to_inner_i(&self, x: usize, y: usize, z: usize) -> usize {
        ChunkUnderlying::chunk_xyz_to_i(x, y, z)
    }

    #[inline(always)]
    //NOT bounds-checked! beware.
    pub fn get_raw_i(&self, i: usize) -> ChunkTileId {
        self.inner.get_i(i)
    }

    #[inline(always)]
    //NOT bounds-checked! beware.
    pub fn get_raw(&self, x: usize, y: usize, z: usize) -> ChunkTileId { 
        // Swizzle to be Z X Y.
        self.inner.get_xyz(z+1,x+1,y+1)
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> Result<ChunkTileId, ChunkVoxelError> {
        if (x >= CHUNK_SZ_X) || (z >= CHUNK_SZ_Y) || (z >= CHUNK_SZ_Z) {
            return Err(ChunkVoxelError::OutOfBounds{attempted_pos: vpos!(x, y, z), 
                                                    oursize: vpos!(CHUNK_SZ_X, CHUNK_SZ_Y, CHUNK_SZ_Z)});
        }
        // self.get_raw already swizzles it, DO NOT SWIZZLE HERE 
        Ok(self.get_raw(x,y,z))
    }

    #[inline(always)]
    //NOT bounds-checked! beware.
    pub fn set_raw(&mut self, tile: ChunkTileId, x: usize, y: usize, z: usize) { 
        // Swizzle to be Z X Y.
        self.inner.set_xyz(tile, z+1,x+1,y+1)
    }

    #[inline]
    pub fn set(&mut self, tile: ChunkTileId, x: usize, y: usize, z: usize) -> Result<_, ChunkVoxelError> {
        if (x >= CHUNK_SZ_X) || (z >= CHUNK_SZ_Y) || (z >= CHUNK_SZ_Z) {
            return Err(ChunkVoxelError::OutOfBounds{attempted_pos: vpos!(x, y, z), 
                                                    oursize: vpos!(CHUNK_SZ_X, CHUNK_SZ_Y, CHUNK_SZ_Z)});
        }
        // self.get_raw already swizzles it, DO NOT SWIZZLE HERE 
        Ok(self.set_raw(tile, x,y,z))
    }
}