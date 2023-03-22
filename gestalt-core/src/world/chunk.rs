use std::ops::Range;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::common::{voxelmath::*, Version};

use super::{
	voxelarray::{VoxelArrayError, VoxelArrayStatic},
	voxelstorage::Voxel,
	TileId, VoxelStorage, VoxelStorageBounded,
};

pub const NEWEST_CHUNK_FILE_VERSION: Version = version!(0, 0, 1);

pub const CHUNK_EXP: usize = 4;
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

// Up to 256 chunk data layers (0 is terrain / voxel).
// Layers can be marked early-order or late-order to preferentially
// put them near the start of the file or the end.
// A layer is n sublayers which are not guaranteed to be contiguous with
// eachother but are guaranteed to be contiguous internally (per sublayer)
// 1-byte layer ID (to tell you which layer this is - not positional)
// 6-byte layer metadata
// 1-byte "num sublayers"
// repeating num-sublayers times:
// > 4 bytes layer start position in file
// > 4 bytes layer end position in file

// Declarative stuff for describing chunk data layers goes here.
pub type ChunkLayerId = u8;
pub type ChunkSublayerId = u8;
pub type ChunkLayerMetadata = [u8; 6];

#[derive(Debug, Clone)]
pub enum ExpectedSublayerLength {
	Exact(usize),
	Range(Range<usize>),
}

pub struct DataSublayerDescriptor {
	/// Used for debug strings on errors and such
	pub name: &'static str,
	/// Used to order in the sublayer array and also for equality checks.  
	pub id: u8,
	/// Based on metadata, how big do we expect this to be? If it's 0 it shouldn't be present.
	pub expected_size: fn(&ChunkLayerMetadata) -> Result<ExpectedSublayerLength, ChunkValidationError>,
}

pub struct DataLayerDescriptor {
	/// Used for debug strings on errors and such
	pub name: &'static str,
	pub id: ChunkLayerId,
	pub possible_sublayers: &'static [DataSublayerDescriptor],
	/// Are there other layers that must be present for this one to make sense?
	pub requires: &'static [ChunkLayerId],
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ChunkValidationError {
	#[error("Could not determine ChunkTilesVariant from metadata: {0} is not a valid variant.")]
	InvalidTilesVariant(u8),
	#[error("Chunk sublayer {0} is an invalid size - we were given {1} bytes and the expected size is {2:?}.")]
	InvalidSizeSublayer(String, usize, ExpectedSublayerLength),
	#[error("Chunk data layer {0} requires data layer {1}, but that requirement is not present in this file.")]
	LayerWithoutRequirement(String, ChunkLayerId),
}

// Length in file. Mapping a u8 to a u32 so it'll be 5 bytes.
pub const SMALL_PALETTE_ENTRY_LEN: usize = 5;
// Mapping a u16 to a u32 so it'll be 6 bytes.
pub const LARGE_PALETTE_ENTRY_LEN: usize = 6;

// Now we start getting into things that are not just for validating and parsing things,
// but which will be more generally useful later.
/// How is voxel data for this chunk stored?
/// * Uniform chunks are all one tile ID and that tile ID is stored in the metadata -
/// useful for things that are all-air or all-stone, for instance.
/// * Small chunks have a palette that can fit in the 0-256 range, and so they can be an
/// array of CHUNK_SIZE_CUBED bytes which are indexes into the palette.
/// * Large chunks have more than 256 unique tile IDs in them, and so it has to be
/// CHUNK_SIZE_CUBED u16s. To be more specific, a sort of unsigned 16-bit integer
/// that is always little-endian on disk and in memory.
#[derive(Debug, Copy, Clone)]
pub enum ChunkTilesVariant {
	Uniform,
	Small,
	Large,
}
impl ChunkTilesVariant {
	pub fn as_upper_metadata_byte(&self) -> u8 {
		match self {
			Self::Uniform => 0,
			Self::Small => 1,
			Self::Large => 2,
		}
	}
}

pub fn chunk_variant_from_metadata(upper_byte: u8) -> Result<ChunkTilesVariant, ChunkValidationError> {
	match upper_byte {
		0 => Ok(ChunkTilesVariant::Uniform),
		1 => Ok(ChunkTilesVariant::Small),
		2 => Ok(ChunkTilesVariant::Large),
		_ => Err(ChunkValidationError::InvalidTilesVariant(upper_byte)),
	}
}

pub fn voxel_data_expected_size(metadata: &ChunkLayerMetadata) -> Result<ExpectedSublayerLength, ChunkValidationError> {
	Ok(match chunk_variant_from_metadata(metadata[0])? {
		ChunkTilesVariant::Uniform => ExpectedSublayerLength::Exact(0),
		ChunkTilesVariant::Small => ExpectedSublayerLength::Exact(CHUNK_SIZE_CUBED),
		ChunkTilesVariant::Large => ExpectedSublayerLength::Exact(CHUNK_SIZE_CUBED * 2),
	})
}

pub fn voxel_palette_expected_size(
	metadata: &ChunkLayerMetadata,
) -> Result<ExpectedSublayerLength, ChunkValidationError> {
	Ok(match chunk_variant_from_metadata(metadata[0])? {
		// Not here in a Uniform chunk - the one individual TileID lives in metadata instead.
		ChunkTilesVariant::Uniform => ExpectedSublayerLength::Exact(0),
		ChunkTilesVariant::Small => {
			ExpectedSublayerLength::Range((SMALL_PALETTE_ENTRY_LEN)..(256 * SMALL_PALETTE_ENTRY_LEN))
		}
		// We could do 256*LARGE_PALETTE_ENTRY_LEN here but I want to give the garbage collection
		// of downgrading chunks some wiggle room.
		ChunkTilesVariant::Large => {
			ExpectedSublayerLength::Range((LARGE_PALETTE_ENTRY_LEN)..(CHUNK_SIZE_CUBED * LARGE_PALETTE_ENTRY_LEN))
		}
	})
}

pub const TILES_LAYER_ID: u8 = 0;
pub const VOXEL_DATA_SUBLAYER_ID: u8 = 0;
pub const VOXEL_PALETTE_SUBLAYER_ID: u8 = 1;

/// Bloxel terrain, should (almost) always be present.
pub const TILES_LAYER_DESC: DataLayerDescriptor = DataLayerDescriptor {
	name: "tiles",
	id: TILES_LAYER_ID,
	possible_sublayers: &[
		DataSublayerDescriptor {
			name: "voxel_data",
			id: VOXEL_DATA_SUBLAYER_ID,
			expected_size: voxel_data_expected_size,
		},
		DataSublayerDescriptor {
			name: "voxel_palette",
			id: VOXEL_PALETTE_SUBLAYER_ID,
			expected_size: voxel_palette_expected_size,
		},
	],
	requires: &[],
};
/* There are 24 made up of 1 identity element,
9 rotations about opposite faces,
8 rotations about opposite vertices and 6 rotations about opposite lines.
This gives 9 + 8 + 6 = 23 possible rotations of the cube,
plus the identity element (leave it where it is giving 24 possible rotations in total. */
// 5 bits for orientation (24 valid states, 8 invalid states. 0 is identity, 23 are actual rotated-states.)
// + 1 bit for mirror
// So, 6 bits per tile. The least common multiple of 6 and 8 is 24.
// 4 tiles = 3 bytes
// (CHUNK_SIZE_CUBED / 4) * 3 should give you the number of bytes required to describe rotations for every tile.

pub fn rotations_expected_size(_metadata: &ChunkLayerMetadata) -> Result<ExpectedSublayerLength, ChunkValidationError> {
	Ok(ExpectedSublayerLength::Exact((CHUNK_SIZE_CUBED / 4) * 3))
}

pub const ROTATIONS_LAYER_ID: u8 = 1;

/// Bloxel terrain, should (almost) always be present.
pub const ROTATIONS_LAYER_DESC: DataLayerDescriptor = DataLayerDescriptor {
	name: "tiles",
	id: ROTATIONS_LAYER_ID,
	possible_sublayers: &[DataSublayerDescriptor {
		name: "rotation_data",
		id: 0,
		expected_size: rotations_expected_size,
	}],
	requires: &[0],
};

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct AlwaysLeU16([u8; 2]);

impl AlwaysLeU16 {
	#[inline(always)]
	pub const fn new(value: u16) -> Self {
		Self(value.to_le_bytes())
	}
	#[inline(always)]
	pub const fn get(&self) -> u16 {
		u16::from_le_bytes(self.0)
	}
	#[inline(always)]
	pub const fn get_bytes(&self) -> &[u8; 2] {
		&self.0
	}
}

impl PartialEq<u16> for AlwaysLeU16 {
	#[inline(always)]
	fn eq(&self, other: &u16) -> bool {
		u16::from_le_bytes(self.0) == *other
	}
}

impl std::hash::Hash for AlwaysLeU16 {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		u16::from_le_bytes(self.0).hash(state);
	}
}

impl std::fmt::Display for AlwaysLeU16 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		u16::from_le_bytes(self.0).fmt(f)
	}
}

impl std::fmt::Debug for AlwaysLeU16 {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		self.get().fmt(f)
	}
}

impl USizeAble for AlwaysLeU16 {
	fn as_usize(&self) -> usize {
		self.get() as usize
	}

	fn from_usize(val: usize) -> Self {
		Self::new(val as u16)
	}
}

// Actual chunk implementation starts here:
pub struct ChunkTilesSmall<T: Voxel> {
	//Attempting to use the constant causes Rust to freak out for some reason
	//so I simply type 16
	pub inner: VoxelArrayStatic<u8, u8, 16>,
	pub palette: [T; 256],
	pub reverse_palette: HashMap<T, u8>,
	pub highest_idx: u8,
	// Used by the serializer to tell if the palette has changed.
	pub palette_dirty: bool,
}

impl<T: Voxel> ChunkTilesSmall<T> {
	#[inline(always)]
	pub fn get_raw(&self, coord: VoxelPos<u8>) -> &u8 {
		//The intent here is so that bounds checking is only done ONCE for this structure.
		self.inner.get_raw(coord)
	}
	#[inline(always)]
	pub fn get_raw_i(&self, i: usize) -> &u8 {
		//The intent here is so that bounds checking is only done ONCE for this structure.
		self.inner.get_raw_i(i)
	}
	#[inline(always)]
	pub fn get(&self, coord: VoxelPos<u8>) -> &T {
		&self.palette[*self.get_raw(coord) as usize]
	}
	#[inline(always)]
	pub fn set_raw(&mut self, coord: VoxelPos<u8>, value: u8) {
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
	pub fn expand(&self) -> ChunkTilesLarge<T> {
		let mut new_palette: Vec<T> = Vec::new();
		for entry in self.palette.iter() {
			new_palette.push(entry.clone())
		}
		let mut new_inner = VoxelArrayStatic::new(AlwaysLeU16::new(0));

		for i in 0..CHUNK_SIZE_CUBED {
			let tile = self.inner.get_raw_i(i);
			new_inner.set_raw_i(i, AlwaysLeU16::new(*tile as u16));
		}

		let mut new_reverse_palette: HashMap<T, AlwaysLeU16> = HashMap::default();
		for (key, value) in self.reverse_palette.iter() {
			new_reverse_palette.insert(key.clone(), AlwaysLeU16::new(*value as u16));
		}
		ChunkTilesLarge {
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
pub struct ChunkTilesLarge<T: Voxel> {
	//Attempting to use the constant causes Rust to freak out for some reason
	//so I simply type 16
	pub inner: VoxelArrayStatic<AlwaysLeU16, u8, 16>,
	pub palette: Vec<T>,
	pub reverse_palette: HashMap<T, AlwaysLeU16>,
	pub palette_dirty: bool,
}

impl<T: Voxel> ChunkTilesLarge<T> {
	#[inline(always)]
	pub fn get_raw(&self, coord: VoxelPos<u8>) -> &AlwaysLeU16 {
		self.inner.get_raw(coord)
	}
	#[inline(always)]
	pub fn get(&self, coord: VoxelPos<u8>) -> &T {
		self.palette.get(self.inner.get_raw(coord).as_usize()).unwrap()
	}
	#[inline(always)]
	pub fn get_raw_i(&self, i: usize) -> &AlwaysLeU16 {
		//The intent here is so that bounds checking is only done ONCE for this structure.
		self.inner.get_raw_i(i)
	}
	#[inline(always)]
	pub fn set_raw(&mut self, coord: VoxelPos<u8>, value: AlwaysLeU16) {
		self.inner.set_raw(coord, value);
	}
	#[inline(always)]
	pub fn index_from_palette(&self, tile: T) -> Option<u16> {
		self.reverse_palette.get(&tile).map(|v| v.get())
	}
	#[inline(always)]
	pub fn tile_from_index(&self, idx: AlwaysLeU16) -> Option<&T> {
		self.palette.get(idx.as_usize())
	}
	/// Adds a Tile ID to its palette. If we succeeded in adding it, return the associated index.
	/// If it already exists, return the associated index. If we're out of room, return None.
	#[inline]
	pub fn add_to_palette(&mut self, tile: T) -> AlwaysLeU16 {
		match self.reverse_palette.get(&tile) {
			Some(idx) => {
				//Already in the palette.
				*idx
			}
			None => {
				self.palette_dirty = true;
				let next_index = AlwaysLeU16::new(self.palette.len() as u16);
				self.palette.push(tile.clone());
				self.reverse_palette.insert(tile, next_index);

				next_index
			}
		}
	}
}

pub enum ChunkInner<T: Voxel> {
	///Chunk that is all one value (usually this is for chunks that are 100% air). Note that, after being converted, idx 0 maps to
	Uniform(T),
	///Chunk that maps palette to 8-bit values.
	Small(Box<ChunkTilesSmall<T>>),
	///Chunk that maps palette to 16-bit values.
	Large(Box<ChunkTilesLarge<T>>),
}

pub struct Chunk<T: Voxel> {
	pub revision: u64,
	pub tiles: ChunkInner<T>,
}

impl<T: Voxel> Chunk<T> {
	pub fn new(default_voxel: T) -> Self {
		Chunk {
			revision: 0,
			tiles: ChunkInner::Uniform(default_voxel),
		}
	}

	#[inline(always)]
	pub fn get_raw_i(&self, i: usize) -> u16 {
		match &self.tiles {
			ChunkInner::Uniform(_) => 0,
			ChunkInner::Small(inner) => *inner.get_raw_i(i) as u16,
			ChunkInner::Large(inner) => inner.get_raw_i(i).get(),
		}
	}

	#[inline(always)]
	pub fn get_raw(&self, pos: VoxelPos<u8>) -> u16 {
		match &self.tiles {
			ChunkInner::Uniform(_) => 0,
			ChunkInner::Small(inner) => *inner.get_raw(pos) as u16,
			ChunkInner::Large(inner) => inner.get_raw(pos).get(),
		}
	}
	#[inline(always)]
	pub fn set_raw(&mut self, pos: VoxelPos<u8>, value: AlwaysLeU16) {
		match &mut self.tiles {
			//TODO: Smarter way of handling this case. Currently, just don't.
			//I don't want to return a result type HERE for performance reasons.
			ChunkInner::Uniform(_) => {
				if value != 0 {
					panic!("Attempted to set_raw() on a Uniform chunk!")
				}
			}
			ChunkInner::Small(ref mut inner) => inner.set_raw(pos, value.get() as u8),
			ChunkInner::Large(ref mut inner) => inner.set_raw(pos, value),
		};
	}
	#[inline(always)]
	pub fn index_from_palette(&self, tile: T) -> Option<u16> {
		match &self.tiles {
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
		match &self.tiles {
			ChunkInner::Uniform(val) => {
				if idx == 0 {
					Some(val)
				} else {
					None
				}
			}
			ChunkInner::Small(inner) => inner.tile_from_index(idx),
			ChunkInner::Large(inner) => inner.tile_from_index(AlwaysLeU16::new(idx)),
		}
	}
	#[inline(always)]
	pub fn is_palette_dirty(&self) -> bool {
		match &self.tiles {
			ChunkInner::Uniform(_) => false,
			ChunkInner::Small(inner) => inner.palette_dirty,
			ChunkInner::Large(inner) => inner.palette_dirty,
		}
	}
	#[inline(always)]
	pub fn mark_palette_dirty_status(&mut self, set_to: bool) {
		match &mut self.tiles {
			ChunkInner::Uniform(_) => {}
			ChunkInner::Small(ref mut inner) => inner.palette_dirty = set_to,
			ChunkInner::Large(ref mut inner) => inner.palette_dirty = set_to,
		}
	}
	#[inline]
	pub fn add_to_palette(&mut self, tile: T) -> AlwaysLeU16 {
		match &mut self.tiles {
			ChunkInner::Uniform(val) => {
				if tile == *val {
					AlwaysLeU16::new(0)
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
					self.tiles = ChunkInner::Small(Box::new(ChunkTilesSmall {
						inner: structure,
						palette,
						reverse_palette,
						highest_idx: 1,
						palette_dirty: false,
					}));
					AlwaysLeU16::new(1)
				}
			}
			ChunkInner::Small(inner) => {
				match inner.add_to_palette(tile.clone()) {
					Some(idx) => AlwaysLeU16::new(idx),
					None => {
						//We need to expand it.
						let mut new_inner = Box::new(inner.expand());
						let idx = new_inner.add_to_palette(tile);
						self.tiles = ChunkInner::Large(new_inner);
						idx
					}
				}
			}
			ChunkInner::Large(inner) => inner.add_to_palette(tile),
		}
	}
}

impl<T: Voxel> VoxelStorage<T, u8> for Chunk<T> {
	type Error = VoxelArrayError<u8>;
	#[inline(always)]
	fn get(&self, pos: VoxelPos<u8>) -> Result<&T, VoxelArrayError<u8>> {
		match &self.tiles {
			ChunkInner::Uniform(val) => Ok(val),
			ChunkInner::Small(inner) => Ok(inner.get(pos)),
			ChunkInner::Large(inner) => Ok(inner.get(pos)),
		}
	}
	#[inline]
	fn set(&mut self, pos: VoxelPos<u8>, tile: T) -> Result<(), VoxelArrayError<u8>> {
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

impl<T: Voxel> VoxelStorageBounded<T, u8> for Chunk<T> {
	fn get_bounds(&self) -> VoxelRange<u8> {
		VoxelRange {
			lower: vpos!(0, 0, 0),
			upper: vpos!(CHUNK_SIZE as u8, CHUNK_SIZE as u8, CHUNK_SIZE as u8),
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
		Self { to_tile: id }
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
		std::mem::size_of::<u16>() + std::mem::size_of::<TileId>()
	}
	fn as_le_bytes(&self) -> [u8; 6] {
		#[cfg(debug_assertions)]
		{
			assert_eq!(Self::serialized_length(), 6)
		}

		let index_bytes = self.from_index.to_le_bytes();
		let tile_bytes = self.to_tile.to_le_bytes();
		// Bytes for the index
		[
			index_bytes[0],
			index_bytes[1],
			//Bytes for the actual tile it maps to
			tile_bytes[0],
			tile_bytes[1],
			tile_bytes[2],
			tile_bytes[3],
		]
	}
	fn from_le_bytes(bytes: [u8; 6]) -> Self {
		#[cfg(debug_assertions)]
		{
			assert_eq!(Self::serialized_length(), 6)
		}
		let idx = u16::from_le_bytes([bytes[0], bytes[1]]);
		let id = TileId::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
		Self {
			from_index: idx,
			to_tile: id,
		}
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
				assert!(crate::world::voxelarray::chunk_xyz_to_i(x, y, z, CHUNK_SIZE) < CHUNK_SIZE_CUBED);
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
		tiles: ChunkInner::Uniform(u1.clone()),
	};

	{
		test_chunk.set(vpos!(1, 1, 1), u1.clone()).unwrap();

		assert_eq!(test_chunk.get(vpos!(1, 1, 1)).unwrap(), &u1);
	}

	let mut valid_result = false;
	if let ChunkInner::Uniform(_) = test_chunk.tiles {
		valid_result = true;
	}
	assert!(valid_result);

	//Make sure Uniform chunks work the way they're supposed to.

	for x in 0..CHUNK_SIZE {
		for y in 0..CHUNK_SIZE {
			for z in 0..CHUNK_SIZE {
				let pos = vpos!(x as u8, y as u8, z as u8);
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
	if let ChunkInner::Small(_) = test_chunk.tiles {
		valid_result = true;
	}
	assert!(valid_result);

	//Make sure that our new ChunkSmall is still the Uniform's tile everywhere except the position where we assigned something else.
	for x in 0..CHUNK_SIZE {
		for y in 0..CHUNK_SIZE {
			for z in 0..CHUNK_SIZE {
				let pos = vpos!(x as u8, y as u8, z as u8);
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
			let pos = vpos!(x as u8, y as u8, z as u8);

			let tile = format!("{}.test", i);

			test_chunk.set(pos, tile.clone()).unwrap();

			assert_eq!(test_chunk.get(pos).unwrap(), &tile);
		}
	}

	let mut valid_result = false;
	if let ChunkInner::Small(_) = test_chunk.tiles {
		valid_result = true;
	}
	assert!(valid_result);

	//Make sure we can assign to everywhere in our chunk bounds.
	for x in 0..CHUNK_SIZE {
		for y in 0..CHUNK_SIZE {
			for z in 0..CHUNK_SIZE {
				let pos = vpos!(x as u8, y as u8, z as u8);
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
			let pos = vpos!(x as u8, y as u8, z as u8);

			let tile = format!("{}.test", i);

			test_chunk.set(pos, tile.clone()).unwrap();

			assert_eq!(test_chunk.get(pos).unwrap(), &tile);
		}
	}
	let mut valid_result = false;
	if let ChunkInner::Large(_) = test_chunk.tiles {
		valid_result = true;
	}
	assert!(valid_result);
}

// This exists to ensure #[repr(transparent)] does the thing I think it does, and that this remains the case.
// Otherwise, some of the code involved in serialization and deserialization would explode dramatically.
#[test]
fn always_le_u16_expectation() {
	assert_eq!(
		std::mem::size_of::<[u8; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]>() * 2,
		std::mem::size_of::<[AlwaysLeU16; CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE]>(),
	)
}
