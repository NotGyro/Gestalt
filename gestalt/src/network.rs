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
struct HandshakePrelude {
    pub role: NetworkRole,
    pub public_key: PublicKey,
    pub version: (u64, u64, u64),
    // This is the nonce we're asking THEM to sign.
    pub please_sign: [u8; 32],
}

impl HandshakePrelude {
    pub fn new(public_key: PublicKey, role: NetworkRole) -> Result<Self, Box<dyn Error>> {
        let version = semver::Version::parse(PROTOCOL_VERSION)?;
        Ok(HandshakePrelude {
            role: role,
            public_key: public_key,
            version: (version.major, version.minor, version.patch),
            please_sign: rand::random(),
        })
    }
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
struct HandshakeSignature {
    sig: Signature,
}

impl HandshakeSignature {
    pub fn reply_to(ident: SelfIdentity, role: NetworkRole, their_prelude: &HandshakePrelude) -> Result<Self, Box<dyn Error>> {
        let version = semver::Version::parse(PROTOCOL_VERSION)?;
        HandshakeSignature::new(ident, role, &their_prelude.please_sign.to_vec(), (version.major, version.minor, version.patch))
    }
    pub fn new(ident: SelfIdentity, role: NetworkRole, nonce: &Vec<u8>, version: (u64, u64, u64)) -> Result<Self, Box<dyn Error>> { 

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
        for byte in nonce {
            buf.push(*byte);
        }
        // Now, produce a signature matching all of this.
        let sig = sign_detached(buf.as_slice(), &ident.secret_key);
        Ok(HandshakeSignature {
            sig: sig,
        })
    }
    pub fn verify(&self, key: PublicKey, nonce: &Vec<u8>, version: (u64, u64, u64)) -> bool {
        let mut buf: Vec<u8> = Vec::new();
        // Write version to a temporary buffer to generate signature.
        for byte in &version.0.to_le_bytes() {
            buf.push(*byte);
        }
        for byte in &version.1.to_le_bytes() {
            buf.push(*byte);
        }
        for byte in &version.2.to_le_bytes() {
            buf.push(*byte);
        }
        // Write the nonce to a temporary buffer to generate our signature.
        for byte in nonce {
            buf.push(*byte);
        }
        // Verify this signs the data.
        verify_detached(&self.sig, buf.as_slice(), &key)
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
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

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
enum HandshakeMessage {
    Prelude(HandshakePrelude), 
    Signature(HandshakeSignature),
    Response(HandshakeResponse),
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

/// A message from a client to the server to set up player properties - i.e. name, attention radius, etc.
#[derive(Clone, PartialEq)]
pub enum ClientSetupMessage {
    SetName(String),
    SetAttentionRadius(f64),
    ///TODO: Remove this.
    HelloWorld,
}

#[derive(Debug, Clone)]
pub enum ClientConnectError {
    HandshakeTimeout,
    Rejected(HandshakeResponse),
    CouldNotVerifyServer
}
impl fmt::Display for ClientConnectError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            ClientConnectError::HandshakeTimeout => write!(f, "Server did not complete handshake in expected time."),
            ClientConnectError::Rejected(response) => write!(f, "Server rejected our connection attempt with response {:?}", response),
            ClientConnectError::CouldNotVerifyServer => write!(f, "Server provided invalid or corrupt handshake and signature."),
        }
    }
}
impl Error for ClientConnectError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        // Generic error, underlying cause isn't tracked.
        None
    }
}

pub struct ConnectionToServer { 
    pub addr: SocketAddr,
    pub identity: PublicKey,
    pub sender: Sender<Packet>,
    pub receiver: Receiver<SocketEvent>,
}

pub struct ClientNet {
    keys: SelfIdentity,
    pub servers: HashMap<SocketAddr, ConnectionToServer>, 
    //addr: SocketAddr,
}

impl ClientNet {
    pub fn new(identity: &SelfIdentity/*, addr: SocketAddr*/) -> Self { 
        ClientNet {
            keys: identity.clone(),
            servers: HashMap::new(),
            //addr: addr,
        }
    }
    pub fn connect(&mut self, server_addr: SocketAddr) -> Result<(), Box<dyn Error>> {
        let mut socket = Socket::bind_any()?;
        
        // Describe our instance and come up with a random nonce so that we can announce ourselves to the server,
        // and also have something they can verify themselves with that can't be copied by an observer.
        let our_prelude = HandshakePrelude::new(self.keys.public_key.clone(),NetworkRole::Client)?;
        socket.send(Packet::reliable_unordered(server_addr, serialize(&HandshakeMessage::Prelude(our_prelude))?))?;

        // This will become Some() later if we get a prelude from them.
        let mut server_prelude : Option<HandshakePrelude> = None;
        // This will become Some() later if we get a sig from them. 
        let mut server_sig : Option<HandshakeSignature> = None;

        let mut server_is_valid = false;
        let mut server_accepted_us = false;

        // In a loop, poll the socket for handshake packets - until timeout.
        let start_connect = Instant::now();
        loop {
            socket.manual_poll(Instant::now());
            match socket.recv() {
                Some(event) => {
                    match event {
                        SocketEvent::Packet(packet) => {
                            let message : HandshakeMessage = deserialize(packet.payload())?;
                            info!("Got another handshake message from {:?}! \n Its contents are: {:?}", packet.addr(), message);
                            match message {
                                HandshakeMessage::Prelude(their_prelude) => { 
                                    info!("Received server's handshake, sending our signature.");
                                    let sig = HandshakeSignature::reply_to(self.keys.clone(), NetworkRole::Client, &their_prelude)?;
                                    socket.send(Packet::reliable_unordered(server_addr, serialize(&HandshakeMessage::Signature(sig))?))?;
                                    server_prelude = Some(their_prelude);
                                },
                                HandshakeMessage::Signature(their_sig) => { 
                                    // Store prelude and sig for later in case they come in the wrong order.
                                    server_sig = Some(their_sig);
                                },
                                HandshakeMessage::Response(response) => match response {
                                    //Handshake accepted! We're good to go.
                                    HandshakeResponse::Accepted => {
                                        server_accepted_us = true;
                                    },
                                    _ => return Err(Box::new(ClientConnectError::Rejected(response))),
                                },
                            };
                        },
                        _ => {},
                    };
                },
                _ => {},
            }
            // Don't keep trying to send "Accepted" forever.
            if !server_is_valid {
                //If they have sent us both a prelude and a sig, try to verify.
                if let Some(their_prelude) = server_prelude {
                    if let Some(their_sig) = server_sig {
                        if their_sig.verify(their_prelude.public_key, &our_prelude.please_sign.to_vec(), their_prelude.version) {
                            let response = HandshakeMessage::Response(HandshakeResponse::Accepted);
                            socket.send(Packet::reliable_unordered(server_addr, serialize(&response)?))?;
                            server_is_valid = true;
                        }
                        else {
                            //The sig they sent does not verify.
                            let response = HandshakeMessage::Response(HandshakeResponse::DeniedCannotVerify);
                            socket.send(Packet::reliable_unordered(server_addr, serialize(&response)?))?;
                            return Err(Box::new(ClientConnectError::CouldNotVerifyServer));
                        }
                    }
                }
            }

            socket.manual_poll(Instant::now());

            // Timeout
            if Instant::now() - start_connect >= Duration::from_secs(4) {
                return Err(Box::new(ClientConnectError::HandshakeTimeout));
            }
            // Is handshake successful?
            if server_is_valid && server_accepted_us {
                break;
            }
        }
        let (sender, receiver) : (Sender<Packet>,Receiver<SocketEvent>) 
                                  = (socket.get_packet_sender(), socket.get_event_receiver());

        // If we got this far, the server verifies and has good identity.
        if server_is_valid { 
            info!("Connection to server completed!");
            let their_prelude = server_prelude.unwrap();
            self.servers.insert(server_addr, ConnectionToServer { 
                addr: server_addr,
                identity: their_prelude.public_key,
                sender: sender.clone(),
                receiver: receiver,
            });
        }
        let _thread = thread::spawn(move || socket.start_polling());
        
        sender.send(Packet::reliable_unordered(server_addr, b"Hello, world!".to_vec()))?;

        Ok(())
    }
}

pub struct ConnectionToClient {
    pub identity: PublicKey,
    //pub player_entity: Option<EntityID>,
    pub addr: SocketAddr,
    // TODO: Move name and attention radius to the Entity Component System when it exists.
    // pub name: String,
    // This describes a (PREFERRED) radius (at scale 0 - 1.0 = 1 meter) around the player, 
    // usually corresponding to draw distance or clientside loaded chunk distance,
    // so that they can be notified when voxel events or entity updates occur within.
    // pub attention_radius: f64,
    pub name: String,
}

struct IncompleteClient { 
    addr: SocketAddr,
    /// Client sends prelude first so this doesn't need to be an option type.
    clients_prelude: HandshakePrelude, 
    our_prelude_to_client: HandshakePrelude,
    they_accepted_us: bool, 
    we_accepted_them: bool,
}

impl IncompleteClient { 
    fn new(addr: SocketAddr, clients_prelude: HandshakePrelude, our_prelude_to_client: HandshakePrelude) -> Self {
        IncompleteClient {
            addr:addr,
            clients_prelude: clients_prelude, 
            our_prelude_to_client: our_prelude_to_client,
            they_accepted_us: false, 
            we_accepted_them: false,
        }
    }
    /// Returns Ok(true) if this client is ready to go.
    fn process(&mut self, message: HandshakeMessage, packet_sender: Sender<Packet>) -> Result<bool, Box<dyn Error>> {
        match message {
            HandshakeMessage::Signature(their_sig) => { 
                // Store prelude and sig for later in case they come in the wrong order.
                if their_sig.verify(self.clients_prelude.public_key, &self.our_prelude_to_client.please_sign.to_vec(), self.clients_prelude.version) {
                    let response = HandshakeMessage::Response(HandshakeResponse::Accepted);
                    packet_sender.send(Packet::reliable_unordered(self.addr, serialize(&response)?))?;
                    self.we_accepted_them = true;
                }
            },
            HandshakeMessage::Response(response) => match response {
                //Handshake accepted! We're good to go.
                HandshakeResponse::Accepted => {
                    self.they_accepted_us = true;
                },
                _ => return Err(Box::new(ClientConnectError::Rejected(response))),
            },
            _ => {},
        };
        if self.they_accepted_us && self.we_accepted_them {
            // Both identities have been confirmed.
            return Ok(true);
        }
        Ok(false)
    }
    fn complete(&self) -> ConnectionToClient { 
        ConnectionToClient {
            identity: self.clients_prelude.public_key,
            addr: self.addr,
            name: String::from(""),
        }
    }
}

pub struct ServerNet {
    keys: SelfIdentity,
    pub clients: HashMap<SocketAddr, ConnectionToClient>,
    pub client_identities: HashMap<Identity, SocketAddr>,
    preauth_clients: HashSet<SocketAddr>, 
    handshake_clients: HashMap<SocketAddr, IncompleteClient>, 
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
            preauth_clients: HashSet::new(), 
            handshake_clients: HashMap::new(),
            our_address:our_address,
            sender:sender,
            incoming:receiver,
        })
    }

    fn process_packet(&mut self, packet: &Packet) -> Result<(), Box<dyn Error>> {
        if IP_BANS.lock().contains(&packet.addr().ip()) {
            //Deny connection somehow. 
            info!("A client attempted to connect from {}, but that IP is banned.", packet.addr().ip());
        }
        //They have been recorded as "Connecting" but have not yet sent a package.
        else if self.preauth_clients.contains(&packet.addr()) {
            let handshake_message : HandshakeMessage = deserialize(packet.payload())?;
            //They have to send us a prelude *first*. 
            if let HandshakeMessage::Prelude(prelude) = handshake_message {
                //We got a prelude, now send them one of our own.
                let our_prelude = HandshakePrelude::new(self.keys.public_key.clone(),NetworkRole::Server)?;
                self.sender.send(Packet::reliable_unordered(packet.addr().clone(), 
                    serialize(&HandshakeMessage::Prelude(our_prelude))?))?;
                //Also, we can pretty much immediately send them a signature on our version and the nonce they sent us.
                let our_sig = HandshakeSignature::reply_to(self.keys.clone(), NetworkRole::Server, &prelude)?;
                self.sender.send(Packet::reliable_unordered(packet.addr().clone(), 
                    serialize(&HandshakeMessage::Signature(our_sig))?))?;
                //Do bookkeeping - client is now in the auth phase.
                self.preauth_clients.remove(&packet.addr());
                self.handshake_clients.insert(packet.addr(), 
                    IncompleteClient::new(packet.addr(), prelude, our_prelude));
            }
        }
        else if self.handshake_clients.contains_key(&packet.addr()) {
            let handshake_message : HandshakeMessage = deserialize(packet.payload())?;
            info!("Got another handshake message from {:?}! \n Its contents are: {:?}", packet.addr(), handshake_message);
            // Safe to unwrap - look at the if block we're in.
            let is_done = self.handshake_clients.get_mut(&packet.addr()).unwrap().process(handshake_message, self.sender.clone())?;
            
            // This handshake process completed, add it to the real clients list.
            if is_done { 
                info!("{:?} is now authorized.", packet.addr());
                let client = self.handshake_clients.get(&packet.addr()).unwrap().complete();
                self.clients.insert(packet.addr(), client);
                self.handshake_clients.remove(&packet.addr());
            }
        }
        else if self.clients.contains_key(&packet.addr()) {
            //This is the block where we handle traffic from already-authenticated clients.
            let msg = packet.payload();

            let msg = String::from_utf8_lossy(msg);
            let ip = packet.addr().ip();
            info!("Received {:?} from {:?}", msg, ip);
        }
        else {
            warn!("Abnormal packet from {:?}", packet.addr());
        }
        Ok(())
    }

    pub fn poll(&mut self) -> Result<(), Box<dyn Error>> {
        // Check for a packet. 
        loop { 
            match self.incoming.try_recv() {
                Ok(event) => match event { 
                    SocketEvent::Connect(client_address) => { 
                        info!("Incoming connection from {:?}!", client_address);
                        //Is this a NEW connection? Laminar seems to fire this event on every
                        //new message.
                        if (!self.clients.contains_key(&client_address) 
                                && !self.preauth_clients.contains(&client_address))
                                && !self.handshake_clients.contains_key(&client_address) {
                            info!("Queueing client {:?} for authorization.", client_address);
                            self.preauth_clients.insert(client_address);
                        }
                    },
                    SocketEvent::Packet(packet) => {
                        match self.process_packet(&packet) {
                            // If we got an error, drop the client rather than breaking our net system.
                            Err(e) => {
                                error!("Got an error while processing a packet from {:?}. \n The error is: {}" , packet.addr(), e);
                                //Encounter an error, get rid of all corresponding entries.
                                if self.preauth_clients.contains(&packet.addr()) {
                                    self.preauth_clients.remove(&packet.addr());
                                }
                                if self.handshake_clients.contains_key(&packet.addr()) {
                                    self.handshake_clients.remove(&packet.addr());
                                }
                                if self.clients.contains_key(&packet.addr()) {
                                    self.clients.remove(&packet.addr());
                                }

                            },
                            Ok(_) => {},
                        }
                    },
                    SocketEvent::Timeout(client_address) => {
                        info!("Client timed out: {:?}", client_address);
                        if self.preauth_clients.contains(&client_address) {
                            self.preauth_clients.remove(&client_address);
                        }
                        if self.handshake_clients.contains_key(&client_address) {
                            self.handshake_clients.remove(&client_address);
                        }
                        if self.clients.contains_key(&client_address) {
                            self.clients.remove(&client_address);
                        }
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