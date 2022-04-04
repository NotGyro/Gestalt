use std::fmt::Debug;

use glam::f32::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};
use shipyard::IntoIter;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Velocity(Vec3);

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
pub struct Transform {
    pub position: Vec3,
    pub rotation: Quat,
}

pub type EntityId = shipyard::EntityId;

#[derive(Clone, Copy, PartialEq, Debug, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Scale(Vec3);

impl Transform {
    pub fn new(position: Vec3) -> Self {
        Transform {
            position,
            rotation: Quat::IDENTITY,
        }
    }
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(Vec3::ONE, self.rotation, self.position)
    }
    pub fn to_matrix_scaled(&self, scale: Vec3) -> Mat4 {
        Mat4::from_scale_rotation_translation(scale, self.rotation, self.position)
    }
}

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
        } else {
            self.current -= damage;
        }
    }
    /// Increase current health. Not named "heal" to avoid confusion -
    /// this does no game logic, it just simply adds to current health, checking max to ensure we don't go over.
    pub fn increase(&mut self, healing: u32) {
        self.current += healing;
        if self.current > self.maximum {
            self.current = self.maximum;
        }
    }
}

pub fn update_moving(
    mut transforms: shipyard::ViewMut<Transform>,
    velocities: shipyard::View<Velocity>,
) {
    for (mut transform, vel) in (&mut transforms, &velocities).iter() {
        transform.position += vel.0;
    }
}

pub mod message {
    use shipyard::{Get, ViewMut};
    use std::convert::Infallible;
    use std::result::Result;

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

    pub fn apply_entity_changes<T: ModifyEntity, I: Iterator<Item = T>>(
        world_view: &mut ViewMut<T::Component>,
        messages: I,
    ) -> Result<(), ModifyEntityMessageError> {
        //Iterate through list of modify entity messages
        for message in messages {
            //Look up the entity we need and the component on that entity
            let target = message
                .get_target()
                .map_err(|e| ModifyEntityMessageError::TargetError(format!("{:?}", e)))?;
            let component = world_view.fast_get(*target).map_err(|e| {
                ModifyEntityMessageError::NoComponent(
                    std::any::type_name::<T::Component>().to_string(),
                    *target,
                    e,
                )
            })?;
            //Actually apply the change.
            message
                .apply(component)
                .map_err(|e| ModifyEntityMessageError::ApplyError(format!("{:?}", e)))?;
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
        pub fn apply(&self, val: &mut Transform) {
            match self {
                ChangePosition::Set(_, v) => {
                    val.position = *v;
                }
                ChangePosition::Move(_, v) => {
                    val.position += *v;
                }
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
        pub fn apply(&self, val: &mut Velocity) {
            match self {
                ChangeVelocity::Set(_, v) => {
                    val.0 = *v;
                }
                ChangeVelocity::Accelerate(_, v) => {
                    val.0 += *v;
                }
            }
        }
    }

    impl ModifyEntity for ChangePosition {
        type Error = Infallible;
        type Component = Transform;

        fn get_target(&self) -> Result<&EntityId, Self::Error> {
            Ok(self.get_target())
        }

        fn apply(&self, component: &mut Transform) -> Result<(), Self::Error> {
            self.apply(component);
            Ok(())
        }
    }

    impl ModifyEntity for ChangeVelocity {
        type Error = Infallible;
        type Component = Velocity;

        fn get_target(&self) -> Result<&EntityId, Self::Error> {
            Ok(self.get_target())
        }

        fn apply(&self, component: &mut Velocity) -> Result<(), Self::Error> {
            self.apply(component);
            Ok(())
        }
    }
}

#[test]
fn test_move_event() {
    use message::*;
    use shipyard::{Get, ViewMut};

    let entity_position = Transform {
        position: Vec3::new(4.0, 5.0, 6.0),
        rotation: Quat::default(),
    };

    let entity_2_position = Transform {
        position: Vec3::new(4.0, 5.0, 6.0),
        rotation: Quat::default(),
    };

    let mut world = shipyard::World::new();
    world.add_entity((entity_2_position,));
    let entity_id = world.add_entity((entity_position,));

    let new_position = Vec3::new(1.0, 1.0, 1.0);

    {
        let change_position_message = ChangePosition::Set(entity_id, new_position);

        let message_list = vec![change_position_message];
        let mut entities = world.borrow::<ViewMut<Transform>>().unwrap();

        apply_entity_changes(&mut entities, message_list.into_iter()).unwrap();
    }

    let entities = world.borrow::<shipyard::View<Transform>>().unwrap();

    //Does the entity reflect our new position as passed in through a message? If so, apply_entity_changes() worked.
    let entity_pos = entities.fast_get(entity_id).unwrap();
    assert!(entity_pos.position == new_position);
}
