use std::hash::Hash;

use hashbrown::HashSet;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentTypeId(u32);

/// The size of a component. The largest component permitted is ComponentSize::MAX
pub type ComponentSize = u16;
pub type EntitySize = u32;

pub struct ComponentTypeDescriptor {
    /// Length of component object in bytes (not counting component header length). 
    /// Set as u32 since no component should ever, ever be larger than 2GB
    pub length: ComponentSize,
    pub id: ComponentTypeId, 
}

impl Hash for ComponentTypeDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        //Hash on ID only. 
        //self.length.hash(state);
        self.id.hash(state);
    }
}
impl PartialOrd for ComponentTypeDescriptor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}
impl PartialEq for ComponentTypeDescriptor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for ComponentTypeDescriptor {}

pub type EntityId = u64;

/// A component type descriptor entry as it lives inside an Archetype, complete with cached offset. 
pub(crate) struct ComponentInArchetype {
    /// The actual component type this entry is wrapping
    pub(crate) ty: ComponentTypeDescriptor,
    /// Offset of this component from the start of an entity.
    pub(crate) offset_cache: EntitySize,
}
impl Hash for ComponentInArchetype {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        //Hash on Type only. 
        //self.offset_cache.hash(state);
        self.ty.hash(state);
    }
}
impl PartialOrd for ComponentInArchetype {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.ty.partial_cmp(&other.ty)
    }
}
impl PartialEq for ComponentInArchetype {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
    }
}
impl Eq for ComponentInArchetype {}

/// Prepended to the start of each component.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ComponentHeader {
    pub entity_id: EntityId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archetype {
    /// Entire length of one entity fitting this archetype per entity.
    /// Cached by adding lengths of each component together. 
    pub(crate) length_cache: EntitySize,
    pub components: HashSet<ComponentInArchetype>,
    /// The underlying buffer containing data.len()/length_cache entities. 
    pub data: Vec<u8>,
    pub id_to_index: HashMap<EntityId, usize>, 
}

pub impl Archetype {
    pub fn get_component(entity: EntityId, component: ComponentId) {}
}

pub struct EntityComponentSystem {
    pub(crate) archetypes: Vec<Archetype>,
    pub(crate) to_clean_up: Vec<EntityId>,
    pub(crate) entity_to_archetype: HashMap<EntityId, usize>,
}