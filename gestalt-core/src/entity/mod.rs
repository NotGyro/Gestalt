use glam::{Quat, EulerRot};
pub use hecs::World as EcsWorld;

use crate::{world::TickLength, common::{Angle, RadianAngle}};
pub type EntityCoord = f32;
pub type EntityVec3 = glam::f32::Vec3;

#[derive(Copy, Clone, Default, Debug)]
pub struct EntityPos {
	pos: EntityVec3,
}
impl EntityPos {
	pub fn new(pos: EntityVec3) -> Self {
		Self { pos }
	}
	#[inline(always)]
	pub fn set(&mut self, new_pos: EntityVec3) {
		self.pos = new_pos
	}
	#[inline(always)]
	pub fn get(&self) -> EntityVec3 {
		self.pos
	}
	pub fn move_by(&mut self, motion: EntityVec3) {
		self.pos += motion
	}
}

/// Represents the position as of the previous server tick.
#[derive(Copy, Clone, Default, Debug)]
pub struct LastPos {
	pub pos: EntityVec3,
}
impl LastPos {
	pub fn new(pos: EntityVec3) -> Self { 
		Self { 
			pos
		}
	}
}

#[derive(Copy, Clone, Default, Debug)]
pub struct EntityRot {
	rot: Quat,
}
impl EntityRot {
	pub fn new(rot: Quat) -> Self {
		Self { rot }
	}
	pub fn new_from_euler<A: Angle>(yaw: A, pitch: A, roll: A) -> Self {
		Self { 
			rot: Quat::from_euler(
				EulerRot::YXZ,
				yaw.get_radians(),
				pitch.get_radians(),
				roll.get_radians())
		}
	}
	#[inline(always)]
	pub fn set(&mut self, new_rot: Quat) {
		self.rot = new_rot
	}
	#[inline(always)]
	pub fn get(&self) -> Quat {
		self.rot
	}
	pub fn set_euler<A: Angle>(&mut self, yaw: A, pitch: A, roll: A) { 
		self.rot = Quat::from_euler(
				EulerRot::YXZ,
				yaw.get_radians(),
				pitch.get_radians(),
				roll.get_radians()
			);
	}

	/// Returns (yaw, pitch, roll)
	pub fn get_euler(&self) -> (RadianAngle, RadianAngle, RadianAngle) { 
		let euler = self.rot.to_euler(EulerRot::YXZ);
		(RadianAngle::from_radians(euler.0),
		RadianAngle::from_radians(euler.1),
		RadianAngle::from_radians(euler.2))
	}

	pub fn turn<A: Angle>(&mut self, yaw: A, pitch: A, roll: A) { 
		self.rot.mul_quat(
			Quat::from_euler(
				EulerRot::YXZ,
				yaw.get_radians(),
				pitch.get_radians(),
				roll.get_radians()
			)
		);
	}
}

#[derive(Copy, Clone, Debug)]
pub struct EntityScale {
	scale: EntityVec3,
}
impl EntityScale {
	pub fn new(scale: EntityVec3) -> Self {
		Self { scale }
	}
	#[inline(always)]
	pub fn set(&mut self, new_scale: EntityVec3) {
		self.scale = new_scale
	}
	#[inline(always)]
	pub fn get(&self) -> EntityVec3 {
		self.scale
	}
	pub fn grow_by(&mut self, change: EntityVec3) {
		self.scale += change
	}
}
impl Default for EntityScale {
    fn default() -> Self {
        Self { scale: EntityVec3::new(1.0, 1.0, 1.0) }
    }
}

#[derive(Copy, Clone, Default, Debug)]
pub struct EntityVelocity {
	motion_per_second: EntityVec3
}
impl EntityVelocity {
	pub fn new(motion_per_second: EntityVec3) -> Self {
		Self {
			motion_per_second
		}
	}
	pub fn get_motion_per_second(&self) -> EntityVec3 { 
		self.motion_per_second
	}
	pub fn apply_tick(&self, to_move: &mut EntityPos, seconds_per_tick: TickLength) {
		to_move.move_by(self.get_motion_per_second() * seconds_per_tick.get())
	}
	pub fn apply_multi_tick(&self, to_move: &mut EntityPos, seconds_per_tick: TickLength, num_ticks: u32) {
		to_move.move_by(self.get_motion_per_second() * seconds_per_tick.get() * (num_ticks as f32))
	}
}

pub fn tick_movement_system(world: &mut EcsWorld, seconds_per_tick: TickLength) {
	for (_entity, (position, velocity, last_pos_maybe)) in
		world.query_mut::<(&mut EntityPos, &EntityVelocity, Option<&mut LastPos>)>()
	{
		let position = position;
		if let Some(previous) = last_pos_maybe {
			previous.pos = position.get();
		}
		velocity.apply_tick(position, seconds_per_tick);
	}
}
