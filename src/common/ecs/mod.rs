use std::{fmt, mem::size_of};
use std::hash::Hash;

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ComponentTypeId(u32);

/// The size of a component. The largest component permitted is ComponentSize::MAX
pub type ComponentSize = u16;
pub type EntitySize = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentTypeDescriptor {
    /// Length of component object in bytes (not counting component header length). 
    /// Set as u32 since no component should ever, ever be larger than 2GB
    pub(crate) size_without_header: ComponentSize,
    /// Ephemeral, in-memory but not guaranteed-consistent component type ID
    pub id: ComponentTypeId,
    /// Human-readable name for debugging purposes, as specified by the module author of the module that requested this component type.
    pub name: String,
    /// Unique identifier for the ComponentType
    pub uuid: Uuid
}

impl Hash for ComponentTypeDescriptor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        //Hash on ID only.
        self.uuid.hash(state);
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

impl fmt::Display for ComponentTypeDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Component type {} ({} bytes in size, internal ID {:?}, UUID {})", &self.name, self.get_size_headless(), &self.id, &self.uuid)
    }
}
impl ComponentTypeDescriptor { 
    /// How many bytes will a component of this type take up in memory, header included?
    #[inline(always)]
    fn get_size_with_header(&self) -> usize {
        (self.size_without_header as usize) + size_of::<ComponentHeader>()
    }
    /// How many bytes will a component of this type take up in memory, ignoring the ComponentHeader?
    #[inline(always)]
    fn get_size_headless(&self) -> usize {
        self.size_without_header as usize
    }
}

pub type EntityId = u64;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl fmt::Display for ComponentInArchetype {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.ty.fmt(f)
    }
}

/// Prepended to the start of each component.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ComponentHeader {
    pub entity_id: EntityId,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchetypeDescriptor {
    /// Entire length of one entity fitting this archetype per entity.
    /// Cached by adding lengths of each component together. 
    pub(crate) entity_length_cache: EntitySize,
    pub(crate) components: HashMap<ComponentTypeId, ComponentInArchetype>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archetype {
    pub descriptor: ArchetypeDescriptor, 
    /// The underlying buffer containing data.len()/length_cache entities. 
    pub data: Vec<u8>,
    /// Maps entity ID to the byte in self.data at which this entity's data begins. 
    pub id_to_offset: HashMap<EntityId, usize>, 
}

pub enum ComponentLookupError { 
    EntityNotPresent(EntityId, usize, ArchetypeDescriptor), 
    OutOfBounds(usize, usize, ArchetypeDescriptor), 
    OutOfEntity(usize, usize, ArchetypeDescriptor), 
    ComponentNotPresent(ComponentTypeId, ArchetypeDescriptor), 
    InvalidSized(EntityId, usize, ComponentTypeId, ArchetypeDescriptor), 
}
impl fmt::Display for ComponentLookupError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ComponentLookupError::EntityNotPresent(entity, num_entities, archetype) => write!(f, "Entity ID {} was not present on an archetype containing {} entities. That archetype is: {:?}", entity, num_entities, archetype),
            ComponentLookupError::OutOfBounds(attempted_index, buffer_size, archetype) => write!(f, "Attempted to access byte {} on a {}-byte entity data buffer for archetype {:?}", attempted_index, buffer_size, archetype),
            ComponentLookupError::OutOfEntity(attempted_index, entity_size, archetype) => write!(f, "Attempted to access byte {} on an entity {} bytes in size for archetype {:?}", attempted_index, entity_size, archetype),
            ComponentLookupError::ComponentNotPresent(component, archetype) => write!(f, "Component ID {:?} is not a part of archetype {:?}", component, archetype),
            ComponentLookupError::InvalidSized(entity, entity_size, component,  archetype) => write!(f, "Entity ID {} was expected to be {} bytes in size. However, attempting to look up the ComponentHeader on component {:?} would result in going past the end of the entity. This should not be possible. Archetype is: {:?} ", entity, entity_size, component,  archetype),
        }
    }
}

impl<'a> Archetype {
    /// Random access of a single component and its header. Please use ECS systems for iteration - this will be very slow in bulk compared to proper ECS iterators.
    pub fn get_component_with_header(&'a self, entity: EntityId, component: ComponentTypeId) -> Result<(&'a ComponentHeader, &'a [u8]),ComponentLookupError> {
        let entity_offset = match self.id_to_offset.get(&entity) {
            Some(ent_offset) => *ent_offset,
            None => return Err(ComponentLookupError::EntityNotPresent(entity, self.id_to_offset.len(), self.descriptor.clone())),
        };
        //Let's learn about this component 
        let (component_offset, component_len_with_header) = match self.descriptor.components.get(&component) {
            Some(comp) => (comp.offset_cache as usize, comp.ty.get_size_with_header()),
            None => return Err(ComponentLookupError::ComponentNotPresent(component, self.descriptor.clone())),
        };

        //NOTE, component_len already includes the size_of::<ComponentHeader>
        // specifically because we got this by calling comp.ty.get_size_with_header().
        // I made these method names verbose because it looked like it could become a footgun. 
        let offset = entity_offset + component_offset;
        let component_end = offset + component_len_with_header; 

        let entity_end = entity_offset + self.descriptor.entity_length_cache as usize;

        //Bounds check to make sure we're not outside of self.data
        //Note component_end should always be used like [start..end), not [start..end], thus the > instead of >= here. 
        if component_end > self.data.len() { 
            return Err(ComponentLookupError::OutOfBounds(component_end, self.data.len(), self.descriptor.clone()));
        }
        //Bounds check to make sure we're not peeking into the next entity
        if component_end > entity_end {
            return Err(ComponentLookupError::OutOfEntity(component_end, self.descriptor.entity_length_cache as usize, self.descriptor.clone()));
        }

        let full_slice = &self.data[offset..component_end];
        //I will feel safe removing this ~later~, when my code is better battle-tested.
        if size_of::<ComponentHeader>() > full_slice.len() {
            return Err(ComponentLookupError::InvalidSized(entity, self.id_to_offset.len(), component, self.descriptor.clone()));
        }

        let (header_bytes, component_slice) = full_slice.split_at(size_of::<ComponentHeader>());

        let header: &ComponentHeader = unsafe {
            //Wishing there was a slightly-safer transmute that checks your byte slice's size against the type's size at runtime. oh well.
            std::mem::transmute::<*const u8, &ComponentHeader>(header_bytes.as_ptr())
        };

        Ok((header, component_slice))
    }
    /// Random access of a single component. Please use ECS systems for iteration - this will be very slow in bulk compared to proper ECS iterators.
    pub fn get_component(&'a self, entity: EntityId, component: ComponentTypeId) -> Result<&'a [u8], ComponentLookupError> {
        // Ignore header, only get component.
        self.get_component_with_header(entity, component).map(|r| r.1 )
    }
}

pub struct EntityWorld {
    pub(crate) archetypes: Vec<Archetype>,
    pub(crate) to_clean_up: Vec<EntityId>,
    pub(crate) entity_to_archetype: HashMap<EntityId, usize>,
}