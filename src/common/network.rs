use std::boxed::Box;
use std::error::Error;
use std::fmt;
use std::fs;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::net::{IpAddr};
use std::path::Path;
use std::result::Result;

use hashbrown::{HashSet};
use parking_lot::Mutex;

use crate::base16;
use sodiumoxide::crypto::sign;
use sodiumoxide::crypto::sign::{PublicKey, SecretKey, Signature};
use sodiumoxide::crypto::sign::ed25519::*;

use serde::{Serialize, Deserialize};

// A chunk has to be requested by a client (or peer server) before it is sent. So, a typical flow would go like this:
// 1. Client: My revision number on chunk (15, -8, 24) is 732. Can you give me the new stuff if there is any?
// 2. Server: Mine is 738, here is a buffer of 6 new voxel event logs to get you up to date.

lazy_static! {
    pub static ref IP_BANS: Mutex<HashSet<IpAddr>> = {
        Mutex::new(HashSet::new())
    };
}

// A client can initiate a connection to a server,
// and a server can initiate a connection to a server (federation, public shared resources, etc),
// but a server should never initiate a connection to a client.
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
#[repr(u8)]
pub enum NetworkRole {
    Server = 0,
    Client = 1,
    Offline = 2
}

pub static PROTOCOL_VERSION: &str = "0.0.1";

/// An error produced when a key you're trying to decode from a serialization is the wrong number of bytes.
#[derive(Debug)]
enum LoadKeyError {
    Public,
    Secret,
}

impl fmt::Display for LoadKeyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LoadKeyError::Public => write!(
                f,
                "Public key is wrong number of bytes (should be {}).",
                PUBLICKEYBYTES
            ),
            LoadKeyError::Secret => write!(
                f,
                "Secret key is wrong number of bytes (should be {}).",
                SECRETKEYBYTES
            ),
        }
    }
}
impl Error for LoadKeyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

pub type Identity = PublicKey;

#[derive(Clone)]
pub struct SelfIdentity {
    pub public_key: PublicKey,
    secret_key: SecretKey,
}

impl SelfIdentity {
    /// Loads or generates a new identity.
    pub fn init() -> Result<Self, Box<dyn Error>> {
        let dir = Path::new("./keys/");
        if dir.is_file() {
            warn!(Network, "\"keys\" should be a directory, and is a file! Moving to file_named_keys.");
            fs::rename(dir, Path::new("file_named_keys"))?;
        }
        if !dir.is_dir() {
            warn!(Network, "\"keys\" directory does not exist! Creating.");
            fs::create_dir(dir)?;
        }

        let pk_path = dir.join("public_key");
        let sk_path = dir.join("secret_key");

        match pk_path.is_file() && sk_path.is_file() {
            true => {
                let mut pk_file = OpenOptions::new()
                    .read(true)
                    .write(false)
                    .open(pk_path.clone())?;
                let mut pk_string = String::new();
                pk_file.read_to_string(&mut pk_string)?;
                let pk_bytes = base16::decode(&pk_string)?;
                let mut sk_file = OpenOptions::new()
                    .read(true)
                    .write(false)
                    .open(sk_path.clone())?;
                let mut sk_string = String::new();
                sk_file.read_to_string(&mut sk_string)?;
                let sk_bytes = base16::decode(&sk_string)?;
                info!(Network, "Loaded identity. Your public key is: \n {}", pk_string);

                Ok(SelfIdentity {
                    public_key: PublicKey::from_slice(&pk_bytes).ok_or(LoadKeyError::Public)?,
                    secret_key: SecretKey::from_slice(&sk_bytes).ok_or(LoadKeyError::Secret)?,
                })
            }
            false => {
                warn!(Network, "Identity has not been generated! Generating it now.");

                let (pk, sk) = sign::gen_keypair();

                let mut pk_file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(pk_path.clone())?;
                let pk_string = base16::encode_upper(&pk);
                pk_file.write_all(pk_string.as_bytes())?;

                let mut sk_file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(sk_path.clone())?;
                let sk_string = base16::encode_upper(&sk);
                sk_file.write_all(sk_string.as_bytes())?;

                info!(Network, "Generated identity. Your public key is: \n {}", pk_string);

                Ok(SelfIdentity {
                    public_key: pk,
                    secret_key: sk,
                })
            }
        }
    }

    pub fn sign(&self, m: &[u8]) -> Signature {
        sign_detached(m, &self.secret_key)
    }
}

pub struct NetSystem {
    pub our_identity: SelfIdentity,
    pub role: NetworkRole,
}

impl NetSystem {

}