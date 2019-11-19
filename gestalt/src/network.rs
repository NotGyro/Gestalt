use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::boxed::Box;
use std::error::Error;
use std::path::Path;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::fmt;
use std::result::Result;
use sodiumoxide::crypto::sign;
use sodiumoxide::crypto::sign::ed25519::*;
use base16;

use crate::entity::EntityID;
use crate::voxel::voxelstorage::Voxel;
use crate::voxel::voxelmath::VoxelCoord;
use crate::voxel::subdivmath::Scale;
use crate::voxel::subdivmath::OctPos;
use crate::world::CHUNK_SCALE;

pub static PROTOCOL_VERSION: &str = "0.0.1";

#[derive(Debug)]
pub enum OpenKeyError {
    Public,
    Secret,
}

impl fmt::Display for OpenKeyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OpenKeyError::Public => write!(f, "Public key is wrong number of bytes (should be {}).", PUBLICKEYBYTES),
            OpenKeyError::Secret => write!(f, "Secret key is wrong number of bytes (should be {}).", SECRETKEYBYTES),
        }
    }
}
impl Error for OpenKeyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}


pub enum DetailFalloff {
    /// Provide as much detail as the server can provide for the chunks loaded by the player.
    NoFalloff,
    /// Sends detail at decreasing levels the further away you are from the player, split up by
    /// chunks and rounded up. 
    /// This behavior should match Linear(f); 
    /// finest scale provided = fine_grained + distance (chunk_pos, player_pos) * f
    /// (remember that larger scale numbers = bigger boxes = less detail)
    Linear(f32),
}
impl Default for DetailFalloff {
    fn default() -> Self { DetailFalloff::NoFalloff }
}

pub struct VoxelSubscription {
    /// The larger cubes / lower-detail end of the data requested
    pub coarse_grained: Scale,
    /// The smaller cubes / lower-detail end of the data requested 
    pub fine_grained: Scale,
    /// Distance of chunks around the player to fetch from the server.
    /// The server is free to ignore this setting if it's higher than what the server
    /// is configured to provide. It's just the "preferred" chunk range.
    pub chunk_radius: f32,
    /// Determines if and how the server should send less detail for chunks further away
    /// from the player.
    pub falloff: DetailFalloff,
}
impl Default for VoxelSubscription {
    fn default() -> Self {
        VoxelSubscription { 
            coarse_grained: CHUNK_SCALE,
            fine_grained: -4,
            chunk_radius: 4.0,
            falloff: DetailFalloff::default(),
        }
    }
}

trait ClientNetWrapper : std::marker::Sized {
    fn new() -> Result<Self, Box<dyn Error>>;
}

pub type Identity = PublicKey;

pub struct SelfIdentity { 
    pub public_key: PublicKey,
    secret_key: SecretKey,
}

impl SelfIdentity {
    /// Loads or generates a new identity. 
    pub fn init() -> Result<Self, Box<dyn Error>> { 
        let dir = Path::new("./keys/");
        if dir.is_file() {
            warn!("\"keys\" should be a directory, and is a file! Moving to file_named_keys.");
            fs::rename(dir, Path::new("file_named_keys"))?;
        }
        if !dir.is_dir() {
            warn!("\"keys\" directory does not exist! Creating.");
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
                
                info!("Loaded identity. Your public key is: {}", pk_string);
                
                Ok(SelfIdentity{
                    public_key: PublicKey::from_slice(&pk_bytes).ok_or(OpenKeyError::Public)?,
                    secret_key: SecretKey::from_slice(&sk_bytes).ok_or(OpenKeyError::Secret)?,
                })
            },
            false => {
                warn!("Identity has not been generated! Generating it now.");

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

                info!("Generated identity. Your public key is: {}", pk_string);

                Ok(SelfIdentity{
                    public_key: pk,
                    secret_key: sk,
                })
            },
        }
    }

    pub fn sign(&self, m: &[u8]) -> Signature {
        sign_detached(m, &self.secret_key)
    }
}