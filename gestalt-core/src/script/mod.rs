use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::common::identity::NodeIdentity;
use crate::common::Version;
use crate::resource::ResourceId;
use string_cache::DefaultAtom as Atom;

pub mod lua;

pub const SCRIPT_PACKAGE_MANIFEST_VERSION: Version = version!(0, 0, 1);

/// A "Module" as a runtime object in Gestalt Engine parlance is the glue between the
/// fast-changing likely-unique world of Worlds and Entities and Voxels and such and the
/// slow-changing, likely-to-duplicate content-addressed world of Resources.
/// It can be attached to one of three things: A world (most commonly), the client, or a
/// player (Player Model and such).
/// Module names are strongly intended for use in interoperability and resources that can
/// be updated without breaking links. Modules have namespaces.
/// Not all Modules are scripts, script packages, or directly interact with scripts - but
/// it is the most common usage of a module, as that's the time you'd need a human-memorable
/// name for a changing resource the most.
#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
pub struct ModuleId {
	// 64-bit, aka 8-byte.
	pub name: Atom,
	// 128-bit, aka 16-byte
	pub uuid: Uuid,
}

//#[derive(Clone, Debug, Hash, PartialEq, PartialOrd)]
//pub struct ModuleId {
//    // 64-bit, aka 8-byte.
//    pub name: Atom,
//    // 256-bit, aka 32-byte
//    pub author: NodeIdentity,
//}
pub struct ModuleDef {
	pub id: ModuleId,
	pub author: NodeIdentity,
	// A ModuleDef is itself a resource. This does not stop us from autoupdating it if we know
	// how to contact `author`, it just means we know where to find this ModuleDef.
	pub this_manifest_id: ResourceId,
	pub dependencies: Vec<ResourceId>,
	pub namespace: HashMap<Atom, ResourceId>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SupportedLanguages {
	Lua,
	/// Transpiles to Lua
	Moon,
}

/// The top-level information about a script package at the start of a PackageManifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageDescriptor {
	pub name: String,
	#[serde(with = "crate::common::version_string")]
	pub version: Version,
	pub language: SupportedLanguages,
}

/// Manifest for a script package. package.ron, for instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
	/// Version of the package manifest format we're using.
	#[serde(with = "crate::common::version_string")]
	pub manifest_format: Version,
	pub package: PackageDescriptor,
	pub dependencies: Vec<ResourceId>,
}
