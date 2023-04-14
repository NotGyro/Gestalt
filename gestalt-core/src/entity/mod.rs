pub use hecs::World as EcsWorld;

use crate::world::TickLength;
pub type EntityCoord = f32;
pub type EntityVec3 = glam::f32::Vec3A;

#[derive(Copy, Clone, Default, Debug)]
pub struct EntityPos {
    pos: EntityVec3,
}
impl EntityPos { 
    pub fn new(pos: EntityVec3) -> Self { 
        Self { 
            pos,
        }
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

#[derive(Copy, Clone, Default, Debug)]
pub struct EntityVelocity {
    change_per_tick: EntityVec3,
}
impl EntityVelocity {
    pub fn new(motion_per_second: EntityVec3, seconds_per_tick: TickLength) -> Self {
        Self {
            change_per_tick: motion_per_second * seconds_per_tick.get()
        }
    }
    pub fn apply_tick(&self, to_move: &mut EntityPos) {
        to_move.move_by(self.change_per_tick)
    }
    pub fn apply_multi_tick(&self, to_move: &mut EntityPos, num_ticks: u32) {
        to_move.move_by(self.change_per_tick * (num_ticks as f32))
    }
}

pub fn tick_movement_system(world: &mut EcsWorld) {
    for (_entity, (position, velocity, last_pos_maybe)) in world.query_mut::<(
            &mut EntityPos, 
            &EntityVelocity, 
            Option<&mut LastPos>)>() { 
        let position = position;
        if let Some(previous) = last_pos_maybe { 
            previous.pos = position.get();
        }
        velocity.apply_tick(position);
    }
}