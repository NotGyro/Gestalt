extern crate serde;
extern crate toml;
extern crate serde_json;
use serde::{Deserialize, Serialize};
use std::result::Result;
use std::error;
use std::fmt;
use serde_json::Error as JSONError;
use toml::de::Error as TOMLError;

pub mod logger;
pub mod event;

#[derive(Debug)]
pub enum ConfigStringDeError{
    JSON(JSONError),
    TOML(TOMLError),
}

impl fmt::Display for ConfigStringDeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigStringDeError::JSON(err) => err.fmt(f),
            ConfigStringDeError::TOML(err) => err.fmt(f),
        }
    }
}

impl error::Error for ConfigStringDeError {
    fn description(&self) -> &str {
        match self {
            ConfigStringDeError::JSON(err) => err.description(),
            ConfigStringDeError::TOML(err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        match self {
            ConfigStringDeError::JSON(err) => Some(err),
            ConfigStringDeError::TOML(err) => Some(err),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigString {
    JSON(String),
    TOML(String),
}

impl Into<String> for ConfigString {
    fn into(self) -> String {
        match self { 
            ConfigString::JSON(json_string) => json_string,
            ConfigString::TOML(toml_string) => toml_string,
        } 
    }
}

impl ConfigString { 
    pub fn deserialize<'a, 'de, T> (&'a self) -> Result<T, ConfigStringDeError> where T: Deserialize<'de>, 'a : 'de {
        match self { 
            ConfigString::JSON(json_string)
                            => serde_json::from_str(json_string.as_str()).map_err(|err| { ConfigStringDeError::JSON(err) }),
            ConfigString::TOML(toml_string) 
                            => toml::from_str(toml_string.as_str()).map_err(|err| { ConfigStringDeError::TOML(err) }),
        }
    }
}