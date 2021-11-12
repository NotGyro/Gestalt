/*! Runtime type information, bdesigned for use inside an entity-component system 
accessible to a scripting engine so that both the Rust host and any scripts it runs 
can tell things like "What kind of components are on this thing?" and "How do I 
deallocate this when I'm done with it?"
 
Notably, any GelatoType TypeLayout is a  *fixed number of byes in size.*
 
This is intended for use in-engine, not on the wire.*/

use std::hash::Hash;

/*------------------------------------------\\
||-------- LAYOUT, TYPE DESCRIPTORS --------||
\\------------------------------------------*/


pub type LayoutTypeID = u8;

// Primitive types 
pub const U8_TYPE_ID:     LayoutTypeID  = 0;
pub const U16_TYPE_ID:    LayoutTypeID  = 1; 
pub const U32_TYPE_ID:    LayoutTypeID  = 2; 
pub const U64_TYPE_ID:    LayoutTypeID  = 3; 
pub const I8_TYPE_ID:     LayoutTypeID  = 4;
pub const I16_TYPE_ID:    LayoutTypeID  = 5; 
pub const I32_TYPE_ID:    LayoutTypeID  = 6; 
pub const I64_TYPE_ID:    LayoutTypeID  = 7; 
pub const F32_TYPE_ID:    LayoutTypeID  = 8;
pub const F64_TYPE_ID:    LayoutTypeID  = 9;
pub const UUID_TYPE_ID:   LayoutTypeID  = 10;

// Strings as index into per-entity byte buffer? Like a Box<<VecU8>> which is one-per-entity.
// Or, perhaps, one per (entity, component_id) pair. 
// Or a Box<Vec<String>>? I'm not sure if this will be necessary for non-strings. 
// Give some thought to the default memory footprint limit for a single component.
//  256-byte? 
// This is the max size per-"statically-sized" component type, btw. Again, unsized elements 
// live elsewhere. 
/*
pub const STRING_128:     LayoutTypeID  = 11; //16-byte string buffer.
pub const STRING_256:     LayoutTypeID  = 12; //32-byte string buffer.
pub const STRING_512:     LayoutTypeID  = 13; //64-byte string buffer.
pub const STRING_1024:    LayoutTypeID  = 14; //128-byte string buffer.
*/

pub struct LayoutEntry { 
    name: String,
    type_id: LayoutTypeID,
}