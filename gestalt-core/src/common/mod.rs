pub mod identity;
pub mod message;
#[macro_use]
pub mod voxelmath;

use std::{fmt::Display, future::Future, pin::Pin};

use serde::{Deserialize, Serialize};

pub type DynFuture<T> = Pin<Box<dyn Future<Output = T>>>;

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
            write!(
                f,
                "{}.{}.{}-build{}",
                self.major(),
                self.minor(),
                self.patch(),
                build
            )
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