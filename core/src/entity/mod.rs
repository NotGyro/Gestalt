use std::{ops::{Add, Sub, AddAssign}, fmt::{Debug, Display}};

use serde::{Serialize, Deserialize};
use shipyard::{IntoIter, EntityId};

pub type EntityCoord = f32;

//TODO: Replace with some external implementation of a vector3 (for example one with a SIMD implementation)
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
#[repr(C)]
pub struct Vec3 {
    pub x: EntityCoord,
    pub y: EntityCoord,
    pub z: EntityCoord,
}

impl Vec3 {
    /// Returns the dot product between this vector and the vector passed in as an argument
    pub fn dot_product(&self, other: &Vec3) -> EntityCoord {
        (self.x * other.x) + (self.y * other.y) + (self.z * other.z)
    }
    /// Get magnitude (length of line from 0 to (x, y, z))
    pub fn mag(&self) -> EntityCoord { 
        EntityCoord::sqrt( self.x*self.x + self.y*self.y + self.z*self.z )
    }
    /// Normalize this vector, returning the normalized version
    pub fn normalize(&self) -> Vec3 { 
        //Prevent division by zero.
        if (self.x == 0.0) && (self.y == 0.0) && (self.z == 0.0) {
            return Vec3 { 
                x: 0.0,
                y: 0.0,
                z: 0.0,
            };
        }

        //Get length
        let mag = self.mag();

        Vec3 { 
            x: self.x / mag,
            y: self.y / mag,
            z: self.z / mag,
        }
    }
    /// Normalize this vector in place, mutably
    pub fn normalize_in_place(&mut self) { 
        //Prevent division by zero.
        if (self.x == 0.0) && (self.y == 0.0) && (self.z == 0.0) {
            // No operation
            return;
        }
        //Get length
        let mag = self.mag();
        
        self.x = self.x / mag;
        self.y = self.y / mag;
        self.z = self.z / mag;
    }
}

impl Display for Vec3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}

impl Add<Vec3> for Vec3 {
    type Output = Vec3;
    fn add(self, other: Vec3) -> Vec3 { 
        Vec3{
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}
impl AddAssign<Vec3> for Vec3 {
    fn add_assign(&mut self, other: Vec3) { 
        self.x += other.x;
        self.y += other.y;
        self.z += other.z;
    }
}

impl Sub<Vec3> for Vec3 {
    type Output = Vec3;
    fn sub(self, other: Vec3) -> Vec3 { 
        Vec3{
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}

impl<T> From<(T,T,T)> for Vec3 where T:Into<EntityCoord> {
    fn from(val: (T,T,T)) -> Self {
        Vec3 { 
            x: val.0.into(),
            y: val.1.into(), 
            z: val.2.into(),
        }
    }
}

impl<T> From<Vec3> for (T,T,T) where T:From<EntityCoord> {
    fn from(val: Vec3) -> Self {
        (val.x.into(), val.y.into(), val.z.into())
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct EntityVel(Vec3);

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct EntityPos(Vec3);

pub struct Health {
    pub current: u32,
    pub maximum: u32,
}

impl Health { 
    pub fn is_dead(&self) -> bool { 
        self.current == 0
    } 
    pub fn is_alive(&self) -> bool { 
        !self.is_dead()
    } 
    /// Reduce current health. Not named "damage" to avoid confusion - 
    /// this does no game logic, it just simply subtracts from current health.
    pub fn reduce(&mut self, damage: u32) {
        if damage >= self.current { 
            self.current = 0;
        }
        else { 
            self.current = self.current - damage;
        }
    }
    /// Increase current health. Not named "heal" to avoid confusion - 
    /// this does no game logic, it just simply adds to current health, checking max to ensure we don't go over.
    pub fn increase(&mut self, healing: u32) {
        self.current = self.current + healing; 
        if self.current > self.maximum { 
            self.current = self.maximum;
        }
    }
}

pub fn update_moving(mut positions: shipyard::ViewMut<EntityPos>, velocities: shipyard::View<EntityVel> ) { 
    for (mut pos, vel) in (&mut positions, &velocities).iter() {
        pos.0 = pos.0 + vel.0;
    }
}

pub mod message {
    use std::convert::Infallible;
    use std::result::Result;
    use shipyard::{ViewMut, Get};

    use super::*;

    /// The ModifyEntity message represents any mutable change made to Component on the provided target EntityID
    pub trait ModifyEntity: Debug + Clone + Send + Sync {
        type Error: Debug + std::error::Error + Clone; 
        type Component: 'static;

        /// Which should this be applied to? 
        fn get_target(&self) -> Result<&EntityId, Self::Error>;
        /// Enact the changes in this event
        fn apply(&self, component: &mut Self::Component) -> Result<(), Self::Error>;
    }

    #[derive(thiserror::Error, Debug, Clone)]
    pub enum ModifyEntityMessageError {
        #[error("error getting target ID from entity change event: {0:?}")]
        TargetError(String),
        #[error("error while trying to apply entity change event {0:?}")]
        ApplyError(String),
        #[error("no entity ID found matching `{0:?}`, which was requested by a message.")]
        NoEntity(EntityId),
        #[error("a message tried to change a component of type `{0:?}` on entity ID `{1:?}`, but that entity does not have that component. Error was: `{2:?}`")]
        NoComponent(String, EntityId, shipyard::error::MissingComponent),
    }

    pub fn apply_entity_changes<T: ModifyEntity, I: Iterator<Item=T> >(world_view: &mut ViewMut<T::Component>, mut messages: I) -> Result<(), ModifyEntityMessageError> { 
        //Iterate through list of modify entity messages
        while let Some(message) = messages.next() { 
            //Look up the entity we need and the component on that entity
            let target = message.get_target()
                .map_err(|e| ModifyEntityMessageError::TargetError(format!("{:?}", e)) )?;
            let component = world_view.fast_get(*target)
                .map_err(|e| 
                    ModifyEntityMessageError::NoComponent(
                        std::any::type_name::<T::Component>().to_string(),
                        *target, 
                        e
                    )
                )?;
            //Actually apply the change.
            message.apply(component)
                .map_err(|e| {
                    ModifyEntityMessageError::ApplyError(format!("{:?}", e)) 
                })?;
        }
        Ok(())
    }

    #[derive(Clone, Copy, Debug)]
    pub enum ChangePosition {
        /// Target, new position 
        Set(EntityId, Vec3),
        /// Target, amount to move by
        Move(EntityId, Vec3),
    }
    
    impl ChangePosition {
        pub fn get_target(&self) -> &EntityId {
            match self {
                ChangePosition::Set(id, _) => id,
                ChangePosition::Move(id, _) => id,
            } 
        }
        pub fn apply(&self, val: &mut EntityPos) { 
            match self {
                ChangePosition::Set(_, v) => {
                    val.0 = *v;
                },
                ChangePosition::Move(_, v) => {
                    val.0 += *v;
                },
            }
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub enum ChangeVelocity {
        /// Target, new velocity 
        Set(EntityId, Vec3),
        /// Target, acceleration or deceleration to apply
        Accelerate(EntityId, Vec3),
    }

    impl ChangeVelocity {
        pub fn get_target(&self) -> &EntityId {
            match self {
                ChangeVelocity::Set(id, _) => id,
                ChangeVelocity::Accelerate(id, _) => id,
            } 
        }
        pub fn apply(&self, val: &mut EntityVel) { 
            match self {
                ChangeVelocity::Set(_, v) => {
                    val.0 = *v;
                },
                ChangeVelocity::Accelerate(_, v) => {
                    val.0 += *v;
                },
            }
        }
    }

    impl ModifyEntity for ChangePosition {
        type Error = Infallible;
        type Component = EntityPos;

        fn get_target(&self) -> Result<&EntityId, Self::Error> {
            Ok(self.get_target())
        }

        fn apply(&self, component: &mut EntityPos) -> Result<(), Self::Error> {
            self.apply(component);
            Ok(())
        }
    }

    impl ModifyEntity for ChangeVelocity {
        type Error = Infallible;
        type Component = EntityVel;

        fn get_target(&self) -> Result<&EntityId, Self::Error> {
            Ok(self.get_target())
        }

        fn apply(&self, component: &mut EntityVel) -> Result<(), Self::Error> {
            self.apply(component);
            Ok(())
        }
    }
}


#[test] 
fn test_vec_magnitude() {
    // Simple 3 4 5 triangle
    let pos = Vec3 { 
        x: 3.0,
        y: 4.0, 
        z: 0.0,
    }; 
    assert!( pos.mag() == 5.0 );
    
    //3 dimensions. 
    let pos = Vec3 { 
        x: 2.0,
        y: 3.0, 
        z: 6.0,
    };
    assert!( pos.mag() == 7.0 );
    
    //Negative numbers shouldn't matter. 
    let pos = Vec3 { 
        x: -2.0,
        y: -3.0, 
        z: -6.0,
    };
    assert!( pos.mag() == 7.0 );
}

#[test] 
fn test_vec_dot_product() {
    let pos_a = Vec3 { 
        x: 1.0,
        y: 2.0, 
        z: 3.0,
    }; 
    let pos_b = Vec3 { 
        x: 4.0,
        y: 5.0, 
        z: 6.0,
    }; 
    assert!( pos_a.dot_product(&pos_b) == 32.0 );
}

#[test]
fn test_move_event() {
    use message::*;
    use shipyard::{ViewMut, Get};

    let entity_position = EntityPos(Vec3 { 
        x: 4.0,
        y: 5.0, 
        z: 6.0,
    });
    
    let entity_2_position = EntityPos(Vec3 { 
        x: 4.0,
        y: 5.0, 
        z: 6.0,
    });
    
    let mut world = shipyard::World::new();
    world.add_entity( (entity_2_position,) );
    let entity_id = world.add_entity( (entity_position,) );
    
    let new_position = Vec3 { 
        x: 1.0,
        y: 1.0, 
        z: 1.0,
    };

    {
        let change_position_message = ChangePosition::Set(entity_id, new_position); 

        let message_list = vec![change_position_message];
        let mut entities = world.borrow::<ViewMut<EntityPos>>().unwrap();

        apply_entity_changes(&mut entities, message_list.into_iter() ).unwrap();
    }

    let entities = world.borrow::<shipyard::View<EntityPos>>().unwrap();

    //Does the entity reflect our new position as passed in through a message? If so, apply_entity_changes() worked.
    let entity_pos = entities.fast_get(entity_id).unwrap();
    assert!(entity_pos.0 == new_position);
}