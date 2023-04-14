pub mod chunk;
pub mod fsworldstorage;
pub mod voxelarray;
pub mod voxelstorage;

use std::ops::Add;
use std::ops::Div;
use std::ops::Mul;
use std::ops::Sub;
use std::time::Duration;

use uuid::Uuid;

//pub use space::Space;
pub use voxelstorage::VoxelStorage;
pub use voxelstorage::VoxelStorageBounded;

use crate::common::identity::NodeIdentity;
use crate::common::voxelmath::VoxelPos;

/// Tiles as they are interacted with in the world (not as stored in a chunk, necessarily) - as in, what a Space will return when you call world_voxel_space.get(x, y, z)
pub type TileId = u32;

/// One coorinate (worldspace) of a tile in a 3D 3-coordinate system (i.e. x: TileCoord, y: TileCoord, z: TileCoord)
pub type TileCoord = i32;

pub type TilePos = VoxelPos<TileCoord>;

//Position of a chunk cell in the space.
pub type ChunkCoord = i32;
pub type ChunkPos = VoxelPos<ChunkCoord>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorldId {
	pub uuid: Uuid,
	/// Either us or the server we're mirroring this from.
	pub host: NodeIdentity,
}
#[derive(Default, Debug, Clone)]
pub struct WorldInfo {
	pub name: String,
}

pub struct World {
	pub world_id: WorldId,
	pub world_info: WorldInfo,
}

/// Length of the fixed time step used for server ticks, and therefore game world logic.
/// This is 1/target ticks per second - but may not correspond exactly to *actual* ticks per
/// second if the server is overtaxed.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct TickLength {
	seconds_per_tick: f32,
}
impl TickLength {
	pub fn new(target_tps: f32) -> Self {
		if !target_tps.is_normal() {
			panic!(
				"Ticks per second must not be zero, infinite, or NaN! \n\
				{} is an invalid TPS.",
				target_tps
			);
		}
		Self {
			seconds_per_tick: { 1.0 / target_tps },
		}
	}

	#[inline(always)]
	pub fn get(&self) -> f32 {
		self.seconds_per_tick
	}

	#[inline(always)]
	pub fn get_duration(&self) -> Duration {
		Duration::from_secs_f32(self.seconds_per_tick)
	}
}

pub const DEFAULT_TPS: f32 = 30.0;

impl Default for TickLength {
	fn default() -> Self {
		Self::new(DEFAULT_TPS)
	}
}

impl Add<f32> for TickLength {
	type Output = f32;

	fn add(self, rhs: f32) -> Self::Output {
		self.get() + rhs
	}
}
impl Add<TickLength> for f32 {
	type Output = f32;

	fn add(self, rhs: TickLength) -> Self::Output {
		self + rhs.get()
	}
}

impl Sub<f32> for TickLength {
	type Output = f32;

	fn sub(self, rhs: f32) -> Self::Output {
		self.get() - rhs
	}
}
impl Sub<TickLength> for f32 {
	type Output = f32;

	fn sub(self, rhs: TickLength) -> Self::Output {
		self - rhs.get()
	}
}

impl Mul<f32> for TickLength {
	type Output = f32;

	fn mul(self, rhs: f32) -> Self::Output {
		self.get() * rhs
	}
}
impl Mul<TickLength> for f32 {
	type Output = f32;

	fn mul(self, rhs: TickLength) -> Self::Output {
		self * rhs.get()
	}
}

impl Div<f32> for TickLength {
	type Output = f32;

	fn div(self, rhs: f32) -> Self::Output {
		self.get() / rhs
	}
}
impl Div<TickLength> for f32 {
	type Output = f32;

	fn div(self, rhs: TickLength) -> Self::Output {
		self / rhs.get()
	}
}

#[test]
#[should_panic]
fn zero_tps_does_panic() {
	let _value = TickLength::new(0.0);
}
