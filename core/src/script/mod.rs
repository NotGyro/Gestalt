use serde::{Deserialize, Serialize};

use crate::common::{
    resource::{ResourceDescriptor, ResourceId},
    Version,
};

pub mod lua;

pub const SCRIPT_PACKAGE_MANIFEST_VERSION: Version = version!(0,0,1);

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

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Manifest for a script package. package.toml
pub struct PackageManifest {
    /// Version of the package manifest format we're using.
    #[serde(with = "crate::common::version_string")]
    pub manifest_format: Version,
    pub package: PackageDescriptor,
    pub dependencies: Vec<ResourceId>,
}

pub struct ScriptResource { 
    /// A resource descriptor representing this script module.
    pub resource: ResourceDescriptor,

}