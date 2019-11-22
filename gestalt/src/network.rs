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
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use hashbrown::{HashSet, HashMap};
use parking_lot::Mutex;

use base16;
use sodiumoxide::crypto::sign;
use sodiumoxide::crypto::sign::{PublicKey, SecretKey, Signature};
use sodiumoxide::crypto::sign::ed25519::*;

//use tokio::{net::TcpListener, net::TcpStream, stream::Stream, stream::StreamExt, io::AsyncWriteExt, io::AsyncReadExt, runtime::Runtime};

use laminar::{SocketEvent, Socket, Packet};

use crossbeam_channel::{bounded, Sender, Receiver, TryRecvError}; 

use serde::{Serialize, Deserialize};
use bincode::serialize;
use bincode::deserialize;

use crate::entity::EntityID;

//lazy_static! {
    // This is an example for using doc comment attributes
    // static ref TOKIO_RT: Mutex<Runtime> = Mutex::new(Runtime::new().unwrap());
//}

/*use crate::voxel::subdivmath::OctPos;
use crate::voxel::subdivmath::Scale;
use crate::voxel::voxelmath::VoxelCoord;
use crate::voxel::voxelstorage::Voxel;
use crate::world::CHUNK_SCALE;*/

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

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
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
    pub fn new(ident: SelfIdentity, role: NetworkRole) -> Result<Self, Box<dyn Error>> { 
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

                Ok(SelfIdentity {
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

#[derive(Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Connection has just been initiated.
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
    //pub player_entity: Option<EntityID>,
    pub ip: SocketAddr,
    // TODO: Move name and attention radius to the Entity Component System when it exists.
    // pub name: String,
    // This describes a (PREFERRED) radius (at scale 0 - 1.0 = 1 meter) around the player, 
    // usually corresponding to draw distance or clientside loaded chunk distance,
    // so that they can be notified when voxel events or entity updates occur within.
    // pub attention_radius: f64,
    state: ConnectionState,
    pub name: String,
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

pub struct ClientNet {
    keys: SelfIdentity,
    state: ConnectionState,
    //addr: SocketAddr,
}

impl ClientNet {
    pub fn new(identity: &SelfIdentity/*, addr: SocketAddr*/) -> Self { 
        ClientNet {
            keys: identity.clone(),
            state: ConnectionState::Handshake,
            //addr: addr,
        }
    }
    pub fn connect(&mut self, server_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let mut socket = Socket::bind_any()?;
        let handshake_message = HandshakeMessage::new(self.keys.clone(),NetworkRole::Client)?;
        
        let (sender, receiver) : (Sender<Packet>,Receiver<SocketEvent>) 
                                  = (socket.get_packet_sender(), socket.get_event_receiver());
        let _thread = thread::spawn(move || socket.start_polling());
        
        sender.send(Packet::reliable_unordered(server_addr, serialize(&handshake_message)?))?;
        sender.send(Packet::reliable_unordered(server_addr, b"Hello, world!".to_vec()))?;

        Ok(())
    }
}

pub struct ServerNet {
    keys: SelfIdentity,
    pub clients: HashMap<SocketAddr, ClientInfo>,
    pub client_identities: HashMap<Identity, SocketAddr>,
    unauth_clients: HashSet<SocketAddr>, 
    pub our_address: SocketAddr,
    sender: Sender<Packet>,
    incoming: Receiver<SocketEvent>,
}

impl ServerNet { 
    pub fn new(identity: &SelfIdentity, our_address: SocketAddr) -> Result<Self, Box<dyn Error>> { 

        let mut socket = Socket::bind(our_address)?;

        let (sender, receiver) : (Sender<Packet>,Receiver<SocketEvent>) 
                                  = (socket.get_packet_sender(), socket.get_event_receiver());
        let _thread = thread::spawn(move || socket.start_polling());
        
        info!("Initiating server on {:?}", our_address);
        Ok(ServerNet {
            keys: identity.clone(),
            clients: HashMap::new(),
            client_identities: HashMap::new(),
            unauth_clients: HashSet::new(),
            our_address:our_address,
            sender:sender,
            incoming:receiver,
        })
    }

    pub fn poll(&mut self) -> Result<(), Box<dyn Error>> {
        // Check for a packet. 
        loop { 
            match self.incoming.try_recv() {
                Ok(event) => match event { 
                    SocketEvent::Packet(packet) => {
                        if IP_BANS.lock().contains(&packet.addr().ip()) {
                            //Deny connection somehow. 
                            info!("A client attempted to connect from {}, but that IP is banned.", packet.addr().ip());
                        }
                        else if self.unauth_clients.contains(&packet.addr()) {
                            let handshake : HandshakeMessage = deserialize(packet.payload())?;
                            if handshake.verify() { 
                                info!("Successfully verified client! Identity is: {}", 
                                            base16::encode_upper(handshake.public_key.as_ref()) );
                                self.unauth_clients.remove(&packet.addr());
                                // Client has been verified as having a legit handshake packet,
                                // add to our list of active clients.
                                self.clients.insert(packet.addr().clone(),
                                    ClientInfo { identity: handshake.public_key.clone(), 
                                                    ip: packet.addr().clone(), 
                                                    state: ConnectionState::PlayerSetup,
                                                    name: String::from("") });
                                self.client_identities.insert(handshake.public_key.clone(), packet.addr().clone());
                            }
                        }
                        else if self.clients.contains_key(&packet.addr()) {
                            let msg = packet.payload();

                            let msg = String::from_utf8_lossy(msg);
                            let ip = packet.addr().ip();
                            info!("Received {:?} from {:?}", msg, ip);
                        }
                        else {
                            warn!("Abnormal packet from {:?}", packet.addr());
                        }
                    },
                    SocketEvent::Connect(client_address) => { 
                        info!("Incoming connection from {:?}!", client_address);
                        //Is this a NEW connection? Laminar seems to fire this event on every
                        //new message.
                        if !self.clients.contains_key(&client_address) {
                            info!("Queueing client {:?} for authorization.", client_address);
                            self.unauth_clients.insert(client_address);
                        }
                    },
                    SocketEvent::Timeout(client_address) => {
                        info!("Client timed out: {:?}", client_address);
                    },
                },
                Err(TryRecvError::Empty) => { 
                    break; 
                },
                Err(e) => {
                    error!("Other TryRecvError: {:?}", e);
                    return Err(Box::new(e));
                },
            }
        }
        Ok(())
    }
}
