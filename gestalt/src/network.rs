use std::boxed::Box;
use std::error::Error;
use std::fmt;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::result::Result;
use std::sync::{Mutex, Arc};

use base16;
use sodiumoxide::crypto::sign;
use sodiumoxide::crypto::sign::{PublicKey, SecretKey, Signature};
use sodiumoxide::crypto::sign::ed25519::*;

use futures::{StreamExt, TryFutureExt};
use tokio::runtime::current_thread::Runtime;
use quinn::{Certificate, ClientConfig, ClientConfigBuilder, Endpoint, EndpointDriver, Incoming, ServerConfig, ServerConfigBuilder};
use crossbeam::crossbeam_channel::{unbounded, Sender, Receiver}; 
use hashbrown::HashSet;

use crate::entity::EntityID;
use crate::voxel::subdivmath::OctPos;
use crate::voxel::subdivmath::Scale;
use crate::voxel::voxelmath::VoxelCoord;
use crate::voxel::voxelstorage::Voxel;
use crate::world::CHUNK_SCALE;

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
#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum NetworkRole {
    Server = 0,
    Client = 1,
    Offline = 2
}

pub static PROTOCOL_VERSION: &str = "0.0.1";

// When accepting an incoming connection, we send a game-handshake packet.
// This packet contains:
// 1. Our public key.
// 2. Our game protocol version.
// 3. A buffer of random data of a fixed arbitrary size. For now, 32 bytes.
// 4. A signature make using our public key, on a buffer of Game protocol version bytes + random data buffer bytes.
// So, first we verify that the major and minor version of our protocol match.
// Non-matching patch numbers are accepted.
// (This may get more permissive later, to accept minor-version differences, but naturally that won't be backwards-compatible.)
// Identity verification is performed, and if our public key verifies the signature of version_bytes.append(random_bytes),
// this connection and this public key are authenticated.

#[derive(Copy, Clone, Debug)]
struct HandshakeMessage {
    role: NetworkRole,
    public_key: PublicKey,
    // Major, minor, and patch.
    version: (u64, u64, u64),
    nonce: [u8; 32],
    /// Signature to verify authorship on a buffer of version.0, version.1, version.2, and nonce.
    sig: Signature,
}

impl HandshakeMessage {
    pub fn new(ident: IdentitySelf, role: NetworkRole) -> Result<Self, Box<dyn Error>> { 
        let nonce: [u8; 32] = rand::random();
        let mut buf: Vec<u8> = Vec::new();
        let version = semver::Version::parse(PROTOCOL_VERSION)?;
        // Write version to a temporary buffer to generate signature.
        for byte in &version.major.to_le_bytes() {
            buf.push(*byte);
        }
        for byte in &version.minor.to_le_bytes() {
            buf.push(*byte);
        }
        for byte in &version.patch.to_le_bytes() {
            buf.push(*byte);
        }
        // Write the nonce to a temporary buffer to generate our signature.
        for byte in &nonce {
            buf.push(*byte);
        }
        // Now, produce a signature matching all of this.
        let sig = sign_detached(buf.as_slice(), &ident.secret_key);
        Ok(HandshakeMessage {
            role: role,
            public_key: ident.public_key.clone(),
            version: (version.major, version.minor, version.patch),
            nonce: nonce,
            sig: sig,
        })
    }
    pub fn verify(&self) -> bool {
        let mut buf: Vec<u8> = Vec::new();
        // Write version to a temporary buffer to generate signature.
        for byte in &self.version.0.to_le_bytes() {
            buf.push(*byte);
        }
        for byte in &self.version.1.to_le_bytes() {
            buf.push(*byte);
        }
        for byte in &self.version.2.to_le_bytes() {
            buf.push(*byte);
        }
        // Write the nonce to a temporary buffer to generate our signature.
        for byte in &self.nonce {
            buf.push(*byte);
        }
        // Now, produce a signature matching all of this.
        verify_detached(&self.sig, buf.as_slice(), &self.public_key)
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum HandshakeResponse {
    Accepted = 0,
    /// This response is for whenever HandshakeMessage.verify() fails.
    DeniedCannotVerify = 1,
    DeniedVersionMismatch = 2,
    /// Response to IP banned user trying to connect. 
    /// We shoot this off on incoming connection, no need to wait for a HandshakeMessage which is probably coming.
    DeniedBanned = 3,
    /// If the network subsystem loads before anything else is done loading, we will respond with this.
    DeniedNotReady = 4,
}

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

pub struct IdentitySelf {
    pub public_key: PublicKey,
    secret_key: SecretKey,
}

impl IdentitySelf {
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
                info!("Loaded identity. Your public key is: \n {}", pk_string);

                Ok(IdentitySelf
                 {
                    public_key: PublicKey::from_slice(&pk_bytes).ok_or(LoadKeyError::Public)?,
                    secret_key: SecretKey::from_slice(&sk_bytes).ok_or(LoadKeyError::Secret)?,
                })
            }
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

                info!("Generated identity. Your public key is: \n {}", pk_string);

                Ok(IdentitySelf
                {
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

#[derive(Clone, PartialEq, Eq)]
pub enum ConnectionState { 
    Handshake,
    /// This state should only be reachable for incoming connections with role=Client.
    PlayerSetup,
    //This state should only be reachable for incoming connections with role=Server.
    //PeerSetup,
    /// Connection is ready to process gamestate.
    Active,
}

impl Default for ConnectionState { 
    fn default() -> Self { ConnectionState::Handshake }
}

impl ConnectionState {
    fn new() -> Self { ConnectionState::Handshake }
}

pub struct ClientInfo {
    pub identity: PublicKey,
    pub player_entity: EntityID,
    pub ip: SocketAddr,
    // TODO: Move name and attention radius to the Entity Component System when it exists.
    pub name: String,
    /// This describes a (PREFERRED) radius (at scale 0 - 1.0 = 1 meter) around the player, 
    /// usually corresponding to draw distance or clientside loaded chunk distance,
    /// so that they can be notified when voxel events or entity updates occur within.
    pub attention_radius: f64,
    state: ConnectionState,
}

impl ClientInfo {
    pub fn is_ready(&self) -> bool {
        self.state == ConnectionState::Active
    }
}

/// A message from a client to the server to set up player properties - i.e. name, attention radius, etc.
#[derive(Clone, PartialEq)]
pub enum ClientSetupMessage {
    SetName(String),
    SetAttentionRadius(f64),
    ///TODO: Remove this.
    HelloWorld,
}

/// Dummy verifier for Quinn networking - we will be verifying identity on our side rather than through 
/// QUIC's TLS integration. This is a federated game, servers should be able to live on bare IP
/// addresses (no domain name), and they should not need a certificate authority.
struct DummyCertVerifier;

impl rustls::ServerCertVerifier for DummyCertVerifier {
    fn verify_server_cert(
        &self,
        _roots: &rustls::RootCertStore,
        _presented_certs: &[rustls::Certificate],
        _dns_name: webpki::DNSNameRef,
        _ocsp_response: &[u8],
    ) -> Result<rustls::ServerCertVerified, rustls::TLSError> {
        Ok(rustls::ServerCertVerified::assertion())
    }
}

pub fn make_client_endpoint<A: ToSocketAddrs>(bind_addr: A,) -> Result<(Endpoint, EndpointDriver), Box<dyn Error>> {
    // Configure client to ignore certificate / certificate authority system.
    let mut cfg = ClientConfigBuilder::default().build();
    let tls_cfg: &mut rustls::ClientConfig = Arc::get_mut(&mut cfg.crypto).unwrap();
    tls_cfg.dangerous().set_certificate_verifier(Arc::new(DummyCertVerifier{}));

    // Okay, now do the real work of building an endpoint.
    let mut endpoint_builder = Endpoint::builder();
    endpoint_builder.default_client_config(cfg);
    let (driver, endpoint, _incoming) =
        endpoint_builder.bind(&bind_addr.to_socket_addrs().unwrap().next().unwrap())?;
    Ok((endpoint, driver))
}

pub fn make_server_endpoint<A: ToSocketAddrs>(bind_addr: A,) 
        -> Result<(EndpointDriver, Incoming, Certificate), Box<dyn Error>> {
    // Continue dummying out certificate functionality rabidly, 
    // by generating a self-signed certificate we don't store anywhere.
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = cert.serialize_der().unwrap();
    let priv_key = cert.serialize_private_key_der();
    let priv_key = quinn::PrivateKey::from_der(&priv_key)?;

    // Simple default settings for transport.
    let server_config = ServerConfig {
        transport: Arc::new(quinn::TransportConfig {
            stream_window_uni: 0,
            ..Default::default()
        }),
        ..Default::default()
    };

    let mut cfg_builder = ServerConfigBuilder::new(server_config.clone());
    let cert = Certificate::from_der(&cert_der)?;
    cfg_builder.certificate(quinn::CertificateChain::from_certs(vec![cert.clone()]), priv_key)?;

    // Endpoint
    let mut endpoint_builder = Endpoint::builder();
    endpoint_builder.listen(server_config);
    let (driver, _endpoint, incoming) =
        endpoint_builder.bind(&bind_addr.to_socket_addrs().unwrap().next().unwrap())?;

    Ok((driver, incoming, cert))
}


