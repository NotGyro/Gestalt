pub mod growable_buffer;
pub mod identity;
#[macro_use]
pub mod message;
#[macro_use]
pub mod voxelmath;
pub mod directories; 

use std::{
	collections::{HashMap, HashSet},
	fmt::Display,
	future::Future,
	marker::PhantomData,
	pin::Pin,
};

use serde::{Deserialize, Serialize};
use xxhash_rust::xxh3::Xxh3Builder;

pub type DynFuture<T> = Pin<Box<dyn Future<Output = T>>>;

pub trait Angle {
	fn get_degrees(&self) -> f32;
	fn get_radians(&self) -> f32;
	fn from_degrees(value: f32) -> Self;
	fn from_radians(value: f32) -> Self;
}

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct RadianAngle(pub f32);

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct DegreeAngle(pub f32);

impl Angle for RadianAngle {
	#[inline(always)]
    fn get_degrees(&self) -> f32 {
        self.0.to_degrees()
    }

	#[inline(always)]
    fn get_radians(&self) -> f32 {
        self.0
    }

	#[inline(always)]
	fn from_degrees(value: f32) -> Self {
        RadianAngle(value.to_radians())
    }

	#[inline(always)]
	fn from_radians(value: f32) -> Self {
        RadianAngle(value)
    }
}

impl Angle for DegreeAngle {
	#[inline(always)]
    fn get_degrees(&self) -> f32 {
        self.0
    }

	#[inline(always)]
    fn get_radians(&self) -> f32 {
        self.0.to_radians()
    }

	#[inline(always)]
	fn from_degrees(value: f32) -> Self {
        DegreeAngle(value)
    }

	#[inline(always)]
	fn from_radians(value: f32) -> Self {
        DegreeAngle(value.to_degrees())
    }
}

pub struct Color { 
	/// Red
	pub r: u8,
	/// Green
	pub g: u8,
	/// Blue
	pub b: u8,
}
impl Color { 
	pub fn to_normalized_float(&self) -> (f32, f32, f32) {
		(self.r as f32 / 255.0,
		self.g as f32 / 255.0,
		self.b as f32 / 255.0)
	}
}

pub struct ColorAlpha { 
	pub color: Color, 
	/// Transparency
	pub alpha: u8,
}
impl ColorAlpha { 
	pub fn to_normalized_float(&self) -> (f32, f32, f32, f32) {
		let color = self.color.to_normalized_float();
		(color.0,
		color.1,
		color.2,
		self.alpha as f32 / 255.0)
	}
}


/// Non-cryptographic hashmap for internally-generated structures.
pub type FastHashMap<K, V> = std::collections::HashMap<K, V, Xxh3Builder>;
/// Non-cryptographic hashset for internally-generated structures.
pub type FastHashSet<T> = std::collections::HashSet<T, Xxh3Builder>;

pub fn new_fast_hash_map<K, V>() -> FastHashMap<K, V> {
	HashMap::with_hasher(Xxh3Builder::new())
}
pub fn new_fast_hash_set<T>() -> FastHashSet<T> {
	HashSet::with_hasher(Xxh3Builder::new())
}

// Any HashMap which does not need to be resistant against HashDos / collision attacks.
// pub type FastHash =

macro_rules! version {
	($major:expr,$minor:expr,$patch:expr,$build:expr) => {
		crate::common::Version::new($major as u32, $minor as u32, $patch as u32, $build as u32)
	};
	($major:expr,$minor:expr,$patch:expr) => {
		crate::common::Version::new($major as u32, $minor as u32, $patch as u32, 0u32)
	};
	($major:expr,$minor:expr) => {
		crate::common::Version::new($major as u32, $minor as u32, 0u32, 0u32)
	};
}

///Array of 4 u32s stored in little-endian byte order (least significant to most): build, patch, minor, major.
#[derive(Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
pub struct Version {
	inner: u128,
}

impl Version {
	pub const fn new(major: u32, minor: u32, patch: u32, build: u32) -> Self {
		let major_bytes = major.to_le_bytes();
		let minor_bytes = minor.to_le_bytes();
		let patch_bytes = patch.to_le_bytes();
		let build_bytes = build.to_le_bytes();

		//Little endian implies least-significant first.
		//Every helper function that makes this easier is incompatible with const fn, so we have to do this the silly way.
		let result_bytes: [u8; 16] = [
			build_bytes[0],
			build_bytes[1],
			build_bytes[2],
			build_bytes[3],
			patch_bytes[0],
			patch_bytes[1],
			patch_bytes[2],
			patch_bytes[3],
			minor_bytes[0],
			minor_bytes[1],
			minor_bytes[2],
			minor_bytes[3],
			major_bytes[0],
			major_bytes[1],
			major_bytes[2],
			major_bytes[3],
		];

		Version {
			inner: u128::from_le_bytes(result_bytes),
		}
	}
	pub fn from_bytes(bytes: &[u8; 16]) -> Self {
		Version {
			inner: u128::from_le_bytes(*bytes),
		}
	}
	pub const fn as_bytes(&self) -> [u8; 16] {
		self.inner.to_le_bytes()
	}
	pub const fn major(&self) -> u32 {
		let bytes = &self.inner.to_le_bytes();
		let result_bytes = [bytes[12], bytes[13], bytes[14], bytes[15]];
		u32::from_le_bytes(result_bytes)
	}
	pub const fn minor(&self) -> u32 {
		let bytes = &self.inner.to_le_bytes();
		let result_bytes = [bytes[8], bytes[9], bytes[10], bytes[11]];
		u32::from_le_bytes(result_bytes)
	}
	pub const fn patch(&self) -> u32 {
		let bytes = &self.inner.to_le_bytes();
		let result_bytes = [bytes[4], bytes[5], bytes[6], bytes[7]];
		u32::from_le_bytes(result_bytes)
	}
	pub const fn build(&self) -> u32 {
		let bytes = &self.inner.to_le_bytes();
		let result_bytes = [bytes[0], bytes[1], bytes[2], bytes[3]];
		u32::from_le_bytes(result_bytes)
	}
	pub fn parse(value: &str) -> std::result::Result<Self, ParseVersionError> {
		let mut in_progress = value.to_ascii_lowercase();
		//Elide all of the unnecessary stuff.
		in_progress.remove_matches("build");
		in_progress.remove_matches("v");
		in_progress.remove_matches("version");

		//Make sure it contains at least one separator character.
		if !(in_progress.contains('.') || in_progress.contains(':') || in_progress.contains('-')) {
			return Err(ParseVersionError::NoSeparators(value.to_string()));
		}
		let split = in_progress.split(|c| {
			let pattern = ['.', ':', '-', '(', ')', '[', ']'];
			pattern.contains(&c) || c.is_whitespace()
		});
		//Make sure none of these are just the space between two separators for some reason.
		let mut fields: Vec<&str> = split.filter(|val| !(*val).is_empty()).collect();
		if fields.len() < 3 {
			return Err(ParseVersionError::TooShort(value.to_string()));
		} else if fields.len() > 4 {
			//Gestalt engine does not use version information more detailed than build number.
			fields.truncate(4);
		}

		//Internal method to convert a field of the string to a version field, to avoid repetition.
		fn number_from_field(
			field: &str,
			original_string: String,
		) -> Result<u32, ParseVersionError> {
			let big_number = field.parse::<u128>().map_err(|_e| {
				ParseVersionError::NotNumber(field.to_string(), original_string.clone())
			})?;
			if big_number > (u32::MAX as u128) {
				return Err(ParseVersionError::TooBig(original_string, big_number));
			}
			//Truncate
			Ok(big_number as u32)
		}

		//We have just ensured there are at least three fields. We can unwrap here.
		let major: u32 = number_from_field(*(fields.get(0).unwrap()), value.to_string())?;
		let minor: u32 = number_from_field(*(fields.get(1).unwrap()), value.to_string())?;
		let patch: u32 = number_from_field(*(fields.get(2).unwrap()), value.to_string())?;
		//Build is optional.
		if fields.len() < 4 {
			Ok(version!(major, minor, patch))
		} else {
			let build: u32 = number_from_field(*(fields.get(3).unwrap()), value.to_string())?;
			Ok(version!(major, minor, patch, build))
		}
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum ParseVersionError {
	#[error("tried to parse {0} into a version but it contained no valid separators")]
	NoSeparators(String),
	#[error("tried to parse {0} into a version but there were 3 or fewer fields")]
	TooShort(String),
	#[error("could not parse `{0}` in version string {1} as a version field: Not a number")]
	NotNumber(String, String),
	#[error("version `{0}` contained {1} as a version string which is larger than the u32 maximum and not permitted")]
	TooBig(String, u128),
}

impl Display for Version {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let build = self.build();
		if build == 0 {
			write!(f, "{}.{}.{}", self.major(), self.minor(), self.patch())
		} else {
			write!(f, "{}.{}.{}-build{}", self.major(), self.minor(), self.patch(), build)
		}
	}
}
impl std::fmt::Debug for Version {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Version({})", self)
	}
}

// Used for serializing and deserializing for Serde
// for example in a `#[serde(with = "crate::common::version_string")]` attribute
pub mod version_string {
	use std::fmt;

	use serde::{
		de::{self, Visitor},
		Deserializer, Serializer,
	};

	use super::*;

	struct VersionVisitor;

	impl<'de> Visitor<'de> for VersionVisitor {
		type Value = Version;

		fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
			formatter.write_str("a version string with 3 or 4 delimited fields for major.minor.patch or major.minor.patch.build i.e. \"1.12.2-build33\"")
		}

		fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
		where
			E: de::Error,
		{
			Version::parse(v).map_err(E::custom)
		}
	}

	pub fn serialize<S>(val: &Version, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: Serializer,
	{
		serializer.serialize_str(val.to_string().as_str())
	}

	pub fn deserialize<'de, D>(deserializer: D) -> Result<Version, D::Error>
	where
		D: Deserializer<'de>,
	{
		deserializer.deserialize_string(VersionVisitor {})
	}
}

/// Option-like semantics entirely within the type system.
/// The compiler MAY optimize to this anyway, but this is a way to be sure if you'd
/// prefer to have, for example, two different methods emitted by codegen for the Some
/// case and for the None case, and give the optimization absolute knowledge of if the
/// input is a Some or a None ahead of time. Useful in certain tight loops, for example
/// in systems for the ECS.
pub trait CompileTimeOption<T> {
	const IS_SOME: bool;
	fn unwrap(self) -> T;
	fn to_option(self) -> Option<T>;
}

pub struct CompileTimeNone<T> {
	_phantom: PhantomData<T>,
}
impl<T> CompileTimeOption<T> for CompileTimeNone<T> {
	const IS_SOME: bool = false;

	#[inline(always)]
	fn unwrap(self) -> T {
		panic!("Cannot unwrap a CompileTimeNone!");
	}

	#[inline(always)]
	fn to_option(self) -> Option<T> {
		None
	}
}

#[repr(transparent)]
pub struct CompileTimeSome<T>(T);

impl<T> CompileTimeOption<T> for CompileTimeSome<T> {
	const IS_SOME: bool = true;

	#[inline(always)]
	fn unwrap(self) -> T {
		self.0
	}

	#[inline(always)]
	fn to_option(self) -> Option<T> {
		Some(self.0)
	}
}

const HEX_TABLE: [&'static str; 256] = [
	"00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "0a", "0b", "0c", "0d", "0e", "0f",
	"10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "1a", "1b", "1c", "1d", "1e", "1f",
	"20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "2a", "2b", "2c", "2d", "2e", "2f",
	"30", "31", "32", "33", "34", "35", "36", "37", "38", "39", "3a", "3b", "3c", "3d", "3e", "3f",
	"40", "41", "42", "43", "44", "45", "46", "47", "48", "49", "4a", "4b", "4c", "4d", "4e", "4f",
	"50", "51", "52", "53", "54", "55", "56", "57", "58", "59", "5a", "5b", "5c", "5d", "5e", "5f",
	"60", "61", "62", "63", "64", "65", "66", "67", "68", "69", "6a", "6b", "6c", "6d", "6e", "6f",
	"70", "71", "72", "73", "74", "75", "76", "77", "78", "79", "7a", "7b", "7c", "7d", "7e", "7f",
	"80", "81", "82", "83", "84", "85", "86", "87", "88", "89", "8a", "8b", "8c", "8d", "8e", "8f",
	"90", "91", "92", "93", "94", "95", "96", "97", "98", "99", "9a", "9b", "9c", "9d", "9e", "9f",
	"a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7", "a8", "a9", "aa", "ab", "ac", "ad", "ae", "af",
	"b0", "b1", "b2", "b3", "b4", "b5", "b6", "b7", "b8", "b9", "ba", "bb", "bc", "bd", "be", "bf",
	"c0", "c1", "c2", "c3", "c4", "c5", "c6", "c7", "c8", "c9", "ca", "cb", "cc", "cd", "ce", "cf",
	"d0", "d1", "d2", "d3", "d4", "d5", "d6", "d7", "d8", "d9", "da", "db", "dc", "dd", "de", "df",
	"e0", "e1", "e2", "e3", "e4", "e5", "e6", "e7", "e8", "e9", "ea", "eb", "ec", "ed", "ee", "ef",
	"f0", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9", "fa", "fb", "fc", "fd", "fe", "ff",
];

pub const fn byte_to_hex(value: u8) -> &'static str { 
	HEX_TABLE[value as usize]
}

#[test]
fn version_order_correct() {
	let ver_new = Version::new(20, 2, 3, 64);
	let ver_old = Version::new(1, 1, 1, 20000);

	assert!(ver_new > ver_old);
}

#[test]
fn version_macro() {
	let a = version!(2, 10, 1);
	assert_eq!(a.major(), 2);
	assert_eq!(a.minor(), 10);
	assert_eq!(a.patch(), 1);
}

#[test]
fn version_to_string() {
	let ver = version!(20, 1, 2);
	let stringified = ver.to_string();
	assert_eq!(stringified.as_str(), "20.1.2");

	let ver2 = version!(13, 13, 2, 242);
	let stringified2 = ver2.to_string();
	assert_eq!(stringified2.as_str(), "13.13.2-build242");
}

#[test]
fn test_parse_version() {
	let stringy = "v0.1.12";
	let ver = Version::parse(stringy).unwrap();
	assert_eq!(ver.major(), 0);
	assert_eq!(ver.minor(), 1);
	assert_eq!(ver.patch(), 12);

	let stringy_with_build = "v7.20.1-build18";
	let ver_with_build = Version::parse(stringy_with_build).unwrap();

	assert_eq!(ver_with_build.major(), 7);
	assert_eq!(ver_with_build.minor(), 20);
	assert_eq!(ver_with_build.patch(), 1);
	assert_eq!(ver_with_build.build(), 18);

	//Gracefully interpret weird version numbers
	let terrible_version_string = "v20 - 19 - 19 :: BUILD(01)";
	let ver_cleaned = Version::parse(terrible_version_string).unwrap();
	assert_eq!(ver_cleaned.major(), 20);
	assert_eq!(ver_cleaned.minor(), 19);
	assert_eq!(ver_cleaned.patch(), 19);
	assert_eq!(ver_cleaned.build(), 1);
}
