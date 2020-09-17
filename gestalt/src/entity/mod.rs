/// Type to store each of an entity's coordinates - x, y and z for an entity will all be EntityCoord
pub type EntityCoord = f32;

///Position of an entity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntPos {
    pub x: EntityCoord,
    pub y: EntityCoord,
    pub z: EntityCoord,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EntVel {
    pub dx: EntityCoord,
    pub dy: EntityCoord,
    pub dz: EntityCoord,
}