extern crate parking_lot;
extern crate log;
extern crate crossbeam;
extern crate serde;
extern crate serde_json;
extern crate rand;

//use std::sync::Arc;
//use self::parking_lot::{Mutex, RwLock};
use std::time::Duration;
use std::hash::{Hash, Hasher};
use std::collections::HashMap;
use std::cmp::Ordering;
use std::error::Error;
use std::result::Result;
use std::net::{SocketAddr, TcpStream, TcpListener, Shutdown};
use std::io::{Read, Write};
use std::io;
use std::fmt;
use std::fmt::Display;

use serde::{Serialize, Deserialize};

//use self::crossbeam::crossbeam_channel::{unbounded, after};
//use self::crossbeam::crossbeam_channel::{Sender, Receiver};

use entity::EntityID;
use voxel::voxelevent::*;
use world::TileID;

//Latest major version / breaking change revision number of our network protocol.
#[allow(dead_code)]
pub const PROTOCOL_VERSION: u32 = 0;

/// A unique identifier for a player or a server. Currently this is just a dummy - eventually this will be a public key.
#[derive(Clone, PartialEq, Eq, PartialOrd, Hash, Serialize, Deserialize, Debug)] 
pub struct Identity {
    _id : u64,
}

impl Display for Identity {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}", self._id) }
}

/// An error reported upon trying to send a packet to a client that doesn't exist. 
#[derive(Debug, Clone)]
pub struct NoClientError {
    id : Identity,
}
impl NoClientError {
    #[allow(dead_code)]
    fn new(id : Identity) -> Self { NoClientError{ id: id } }
}
impl Display for NoClientError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Attempted to send a packet to identity {}, but no such client is connected.", self.id)
    }
}
impl Error for NoClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

/// An error reported when a client attempts to connect but does not identify in time. 
#[derive(Debug, Clone)]
pub struct DidNotIdentifyError {
    ip : SocketAddr,
}
impl DidNotIdentifyError {
    #[allow(dead_code)]
    fn new(ip : SocketAddr) -> Self { DidNotIdentifyError{ ip: ip } }
}
impl Display for DidNotIdentifyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "A client connected from address {}, but did not provide any identity.", self.ip)
    }
}
impl Error for DidNotIdentifyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

// Sometimes we need to know which client created this packet,
// so we can avoid broadcasting a client's own event back to it.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualifiedToClientPacket {
    pub client_id: Identity, 
    pub pak: ToClientPacket,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToClientPacket {
    pub data: ToClientPacketData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToClientPacketData {
    Ping,
    Pong,
    Kick,
    ChatMsg(Identity, String),
    Ready, //Is this server fully started?
    NotReady, //Wait a minute, server is still starting.
    VoxEv(VoxelEvent<TileID, i32>),
    UpdateEntity(EntityID, [f32; 3]),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QualifiedToServerPacket {
    pub client_id: Identity, 
    pub pak: ToServerPacket,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToServerPacket {
    pub data: ToServerPacketData,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ToServerPacketData {
    Ping,
    Pong,
    Disconnect,
    ChatMsg(String),
    Join(Identity), //Tell the server we're here and who we are.
    SetName(String),
    VoxEv(VoxelEvent<TileID, i32>),
    UpdateMyPosition([f32; 3]),
}

/// A ClientInfo is the server's way of keeping track of a client.
#[derive(Clone,PartialEq,Eq)]
pub struct ClientInfo {
    pub player_id : Identity,
    pub client_ip : SocketAddr,
    pub bound_entity : EntityID,
    pub name : String,
}

//Compare on ID since this must be unique by definition. 
impl PartialOrd for ClientInfo {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.player_id.partial_cmp(&other.player_id)
    }
}
impl Hash for ClientInfo {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.player_id.hash(state);
    }
}

#[allow(dead_code)]
pub struct Server { 
    listener : TcpListener,
    clients : HashMap<Identity, (ClientInfo, TcpStream)>,
    ready : bool, // Should clients start connecting to this server, or is it still starting up?
    addr : SocketAddr,
    broadcast_list : Vec<QualifiedToClientPacket>,
    to_drop : Vec<Identity>,
    messages_received : Vec<QualifiedToServerPacket>,
}

#[allow(dead_code)]
impl Server {
    pub fn new(addr: SocketAddr) -> Result<Server, Box<dyn Error>> {
        //Bind our listener.
        let listener = TcpListener::bind(addr)?; //.map_err(|err| {error!("{}", err)})
        info!("Server listening on port {}", addr.port());
        //Set our listener as nonblocking so we can poll it rather than blocking when no new clients are connecting.
        listener.set_nonblocking(true)?;
        Ok( Server{
            listener : listener,
            clients : HashMap::new(),
            ready : false,
            addr : addr,
            broadcast_list : Vec::new(),
            to_drop : Vec::new(),
            messages_received : Vec::new(),
            }
        )
    }
    pub fn set_ready(&mut self, val : bool) { self.ready = val }

    pub fn send_to_client(&self, packet: QualifiedToClientPacket) -> Result<(), Box<dyn Error>> {
        if let Some((_, stream)) = self.clients.get(&packet.client_id) {
            Self::send_packet(&mut stream.try_clone().unwrap(), packet.pak)?;
            Ok(())
        }
        else {
            Err(Box::new(NoClientError::new(packet.client_id.clone())))
        }
    }
    pub fn queue_broadcast(&mut self, packet: QualifiedToClientPacket) {
        self.broadcast_list.push(packet);
    }

    fn read_incoming_packet(stream: &mut TcpStream) -> Result<ToServerPacket, std::io::Error> {
        //At first I toyed with conditional compilation to get this to accept any usize,
        //but then I realized - what am I doing? If we have a packet larger than 2 GB
        //something is horribly wrong. So, it's a u32 rather than a usize.
        let mut buf : [u8; 4] = [0; 4];
        stream.read_exact(&mut buf)?;
        let msg_len = u32::from_le_bytes(buf);
        //If we already got a packet size, we should finish and read the rest of the packet.
        stream.set_nonblocking(false)?;
        debug!("Receiving packet of size {} from {:?}", msg_len, stream.peer_addr());

        let mut buf : Vec<u8> = vec![0; msg_len as usize];
        stream.read_exact(&mut buf)?;

        let text = std::str::from_utf8(buf.as_slice()).unwrap();

        stream.set_nonblocking(true)?; //Set back to nonblocking mode for the listen process.

        Ok(serde_json::from_str::<ToServerPacket>(text)?)
    }

    fn send_packet(stream: &mut TcpStream, packet: ToClientPacket) -> Result<(), std::io::Error> {
        let msg = serde_json::to_string(&packet)?;
        let size = msg.len() as u32;
        stream.write(&size.to_le_bytes())?;
        stream.write(msg.as_bytes())?;
        stream.flush()?;
        Ok(())
    }

    /// Accept any incoming client connections.
    pub fn accept_step(&mut self) -> Result<(), Box<dyn Error>> {
        match self.listener.accept() {
            Ok(stream_tuple) => {
                let (mut stream, ip) = stream_tuple;
                info!("Client connecting from {}", ip);
                
                //The next thing the client does MUST be to send us a join packet.
                //We shouldn't let them tie up the network thread with it, either,
                //so we need it within 350 ms (the highest ping I've ever had joining 
                //a game server, and that was US to Australia).
                let timeout = Some(Duration::from_millis(350));
                stream.set_read_timeout(timeout)?;
                stream.set_nonblocking(false)?; //Definitely block, we need a Join packet here.
                //Get protocol version.         
                let mut buf : [u8; 4] = [0; 4];
                stream.read_exact(&mut buf)?;
                let prot = u32::from_le_bytes(buf);
                info!("This client is connecting with protocol version {}. Ours is {}.", prot, PROTOCOL_VERSION);
                assert_eq!(prot, PROTOCOL_VERSION);
                //Now let's get a join packet.
                let packet = Self::read_incoming_packet(&mut stream)?;
                if let ToServerPacketData::Join(id) = packet.data {
                    let player = ClientInfo{ player_id : id.clone(),
                                                client_ip : ip,
                                                bound_entity : 0,
                                                name : "Player".to_owned(),
                                                };
                    stream.set_nonblocking(true)?;
                    self.clients.insert(id, (player, stream));
                }
                else {
                    return Err(Box::new(DidNotIdentifyError::new(ip)));
                }
            }, 
            Err(error) => {
                match error.kind() {
                    std::io::ErrorKind::WouldBlock => {return Ok(()); /* Do nothing, nobody's connecting. */}, 
                    _ => error!("Got an error while trying to accept a client connection: {}", error),
                }
            },
        }
        Ok(())
    }

    ///Send and receive data from already connected clients.
    pub fn stream_step(&mut self) -> Result<(), Box<dyn Error>> {
        //Iterate over all clients to send and receive messages.
        for (id, (client, stream)) in self.clients.iter_mut() { 
            //First, receive. Are there any messages sent to us from this client?
            match Self::read_incoming_packet(stream) {
                Ok(pak) => {
                    debug!("Received {:?} from {:?}", pak, client.client_ip);
                    // TODO: Actually dispatch these packets somewhere.
                    match pak.data {
                        ToServerPacketData::Disconnect => self.to_drop.push(id.clone()),
                        ToServerPacketData::SetName(ref name) => { client.name = name.clone();
                            self.messages_received.push(QualifiedToServerPacket{client_id: id.clone(), pak:pak});
                        },
                        _ => self.messages_received.push(QualifiedToServerPacket{client_id: id.clone(), pak:pak}),
                    }
                },
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { /* Nothing to read right now, check again later. */ },
                Err(e) => { 
                    error!("Encountered IO while  from a client stream: {}", e);
                    return Err(Box::new(e));
                },
            };
            // Now, let's send any queued messages out to this client.
            // Each broadcast message gets sent to each client.
            for pak in self.broadcast_list.iter() {
                // Make sure we're not sending their own events back to them.
                if pak.client_id != *id {
                    Self::send_packet(stream, pak.pak.clone())?;
                }
            }
        }
        //We have flushed the buffer of messages to broadcast to all clients, clear it.
        self.broadcast_list.clear();
        Ok(())
    }
    pub fn cleanup_step(&mut self)  -> Result<(), Box<dyn Error>> {
        //Remove everyone who disconnected
        for id in self.to_drop.iter() {
            {
                let (_client, stream) = self.clients.get(&id).unwrap();
                stream.shutdown(Shutdown::Both)?;
            }
            self.clients.remove(&id);
        }
        self.to_drop.clear();
        Ok(())
    }
    pub fn poll (&mut self) -> Vec<QualifiedToServerPacket> { self.messages_received.drain(..).collect() }
}

//We only get one of these upon connecting
struct _ClientInner {
    stream : TcpStream,
}

#[allow(dead_code)]
pub struct Client {
    inner: Option<_ClientInner>,
    name: String,
    ident: Identity,
    messages_received : Vec<ToClientPacket>,

}
#[allow(dead_code)]
impl Client {
    pub fn new() -> Self {
        Client {inner: None, name: "Player".to_owned(), ident: Identity{_id: rand::random()}, messages_received: Vec::new() }
    }

    fn read_incoming_packet(stream: &mut TcpStream) -> Result<ToClientPacket, std::io::Error> {
        let mut buf : [u8; 4] = [0; 4];
        stream.read_exact(&mut buf)?;
        let msg_len = u32::from_le_bytes(buf);
        //If we already got a packet size, we should finish and read the rest of the packet.
        stream.set_nonblocking(false)?;
        debug!("Receiving packet of size {} from {:?}", msg_len, stream.peer_addr());

        let mut buf : Vec<u8> = vec![0; msg_len as usize];
        stream.read_exact(&mut buf)?;

        let text = std::str::from_utf8(buf.as_slice()).unwrap();

        stream.set_nonblocking(true)?; //Set back to nonblocking mode for the listen process.

        Ok(serde_json::from_str::<ToClientPacket>(text)?)
    }

    pub fn send_packet(&mut self, packet: ToServerPacket) -> Result<(), std::io::Error> { 
        if let Some(ref mut inner) = self.inner {
            let msg = serde_json::to_string(&packet)?;
            let size = msg.len() as u32;
            inner.stream.write(&size.to_le_bytes())?;
            inner.stream.write(msg.as_bytes())?;
            inner.stream.flush()?;
        } else {
            warn!("Attempted to send a packet while not connected to a server: {:?}", packet);
        }
        Ok(())
    }
    pub fn connect(&mut self, addr: SocketAddr) -> Result<(), std::io::Error> {
        match TcpStream::connect(addr) {
            Ok(mut stream) => {
                debug!("Successfully connected to server at {}.", addr);
                //First, tell the server what protocol breaking change we're using.
                stream.write(&PROTOCOL_VERSION.to_le_bytes())?;
                self.inner = Some(_ClientInner{stream:stream.try_clone().unwrap()});

                //Now let's send a join packet.
                stream.set_nonblocking(false)?;
                let packet = ToServerPacket { 
                    data: ToServerPacketData::Join(self.ident.clone()),
                };
                self.send_packet(packet)?;
                
                let packet = ToServerPacket { 
                    data: ToServerPacketData::SetName(self.name.clone()),
                };
                self.send_packet(packet)?;

                stream.set_nonblocking(true)?;
                Ok(())
            },
            Err(e) => Err(e),
        }
    }
    pub fn disconnect(&mut self) -> Result<(), Box<dyn Error>> { 
        if let Some(ref mut inner) = self.inner {

            let packet = ToServerPacket { 
                data: ToServerPacketData::Disconnect,
            };

            let msg = serde_json::to_string(&packet)?;
            let size = msg.len() as u32;
            inner.stream.write(&size.to_le_bytes())?;
            inner.stream.write(msg.as_bytes())?;
            inner.stream.flush()?;

            inner.stream.shutdown(Shutdown::Both)?;
        }
        else {
            warn!("Attempted to disconnect from a server while not connected to a server.");
        }
        self.inner = None;
        Ok(())
    }
    pub fn receive_step(&mut self) -> Result<(), Box<dyn Error>> {
        if self.inner.is_none() {
            warn!("Attempted to receive packets from a server while not connected to a server.");
            return Ok(());
        }
        let stream = self.inner.as_ref().unwrap().stream.try_clone()?;
        match Self::read_incoming_packet(&mut stream.try_clone()?) {
            Ok(pak) => {
                debug!("Received {:?} from server.", pak);
                // TODO: Actually dispatch these packets somewhere.
                match pak.data {
                    ToClientPacketData::Kick => {
                        stream.shutdown(Shutdown::Both)?;
                        self.inner = None;
                        self.messages_received.push(pak);
                        return Ok(());
                    },
                    ToClientPacketData::Ping => self.send_packet(ToServerPacket{data: ToServerPacketData::Pong,})?,
                    _ => self.messages_received.push(pak),
                }
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => { /* Nothing to read right now, check again later. */ },
            Err(e) => { 
                error!("Encountered IO while  from a client stream: {}", e);
                return Err(Box::new(e));
            },
        };
        Ok(())
    }
    pub fn poll (&mut self) -> Vec<ToClientPacket> { self.messages_received.drain(..).collect() }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.disconnect().unwrap();
    }
}