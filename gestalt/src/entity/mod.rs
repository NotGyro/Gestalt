use cgmath::{Vector3};

/// Type to store each of an entity's coordinates - x, y and z for an entity will all be EntityCoord
pub type EntityCoord = f32;

/// Time step for entity, in seconds 
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct TimeStep(pub f32);

/// Position of an entity.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EntPos (pub Vector3<EntityCoord>);

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct EntVel (pub Vector3<EntityCoord>);

#[system(for_each)]
pub fn update_positions(pos: &mut EntPos, vel: &EntVel, #[resource] time: &TimeStep) {
    pos.0.x += vel.0.x * time.0;
    pos.0.y += vel.0.y * time.0;
    pos.0.z += vel.0.z * time.0;
}