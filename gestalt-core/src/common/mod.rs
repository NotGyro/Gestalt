pub mod growable_buffer;
pub mod identity;
#[macro_use]
pub mod message;
#[macro_use]
pub mod voxelmath;
pub mod directories;

use core::str;
use std::{
	collections::{HashMap, HashSet},
	future::Future,
	marker::PhantomData,
	pin::Pin, ptr,
};

use log::warn;
use semver::Version;
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
		(self.r as f32 / 255.0, self.g as f32 / 255.0, self.b as f32 / 255.0)
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
		(color.0, color.1, color.2, self.alpha as f32 / 255.0)
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

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FixedString([u8; 8]);

impl FixedString {
	pub fn as_str<'a>(&'a self) -> &'a str { 
		unsafe { str::from_raw_parts(ptr::from_ref(&self.0[0]), 8) }
	}
	pub fn from_str(value: &str) -> Self { 
		if value.len() > 8 {
			warn!("Constructing a FixedString out of {value} which is {0} bytes long, truncating to first 8", value.len());
		}
		let mut out = [0; 8];
		let end_copy = value.len().min(8);
		let slice = &value.as_bytes()[0..end_copy];
		out.copy_from_slice(slice);

		Self(out)
	}
	pub const fn from_const(value: &'static str) -> Self {
		let bytes = value.as_bytes();
		assert!(bytes.len() == 8);
		
		Self([bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]])
	}
}
