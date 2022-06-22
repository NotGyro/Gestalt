use std::{marker::PhantomData, net::SocketAddr, sync::Arc};

use crate::common::identity::NodeIdentity;

use parking_lot::Mutex;
use tokio::sync::mpsc::UnboundedSender;
use std::collections::HashMap;
use lazy_static::lazy_static;

use super::{NetMsg, PacketIntermediary};

#[derive(thiserror::Error, Debug)]
pub enum NetSendError {
    #[error("Unable to send a netmsg of netmsg type {0} onto a channel: {1}")]
    SendOnChannel(&'static str, String),
    #[error("Failed to encode a packet out of netmsg type {0}. The error was: {1}")]
    ConstructPacket(&'static str, String),
    #[error("No channel established yet for peer {0}")]
    NoChannel(String),
}

pub struct NetSendChannel<T> where T: Send + NetMsg { 
    pub(in crate::net::net_channel) inner: UnboundedSender<Vec<PacketIntermediary>>,
    //pub(in crate::net::net_channel) peer_addr: SocketAddr,
    _t: PhantomData<T>,
}

impl<T> NetSendChannel<T>  where T: Send + NetMsg {
    pub fn new(sender: UnboundedSender<Vec<PacketIntermediary>>) -> Self { 
        NetSendChannel{ 
            inner: sender,
            //peer_addr,
            _t: PhantomData::default(),
        }
    }
    pub fn send(&self, message: &T) -> Result<(), NetSendError> {
        let packet = message.construct_packet()
            .map_err(|e| NetSendError::ConstructPacket(T::net_msg_name(), format!("{:?}", e)))?;
        
        self.inner.send(vec![packet])
            .map_err(|e| NetSendError::SendOnChannel(T::net_msg_name(), format!("{:?}", e)))?;

        Ok(())
    }
    pub fn send_multi(&self, messages: Vec<&T>) -> Result<(), NetSendError> {
        let mut packets: Vec<PacketIntermediary> = Vec::default();

        for message in messages {
            let packet = message.construct_packet()
                .map_err(|e| NetSendError::ConstructPacket(T::net_msg_name(), format!("{:?}", e)))?;
            packets.push(packet);
        }

        self.inner.send(packets)
            .map_err(|e| NetSendError::SendOnChannel(T::net_msg_name(), format!("{:?}", e)))?;

        Ok(())
    }
}


pub struct NetMsgSystem {
    pub sender_channels: HashMap<NodeIdentity, UnboundedSender<Vec<PacketIntermediary>>>
}

lazy_static!{
    static ref NET_MSG_SYSTEM: Arc<Mutex<NetMsgSystem>> = { 
        Arc::new(Mutex::new(NetMsgSystem{ 
            sender_channels: HashMap::new()
        }))
    };
}

#[derive(thiserror::Error, Debug)]
pub enum NetMsgSubscribeError {
    #[error("No channel established yet for peer {0}")]
    NoChannel(String),
    #[error("A channel was already registerd for peer {0}")]
    RegisterAlreadyRegistered(String),
}

pub fn register_channel(peer: NodeIdentity, sender: UnboundedSender<Vec<PacketIntermediary>>) -> Result<(), NetMsgSubscribeError>{ 
    let arc = NET_MSG_SYSTEM.clone();
    let mut system_reference = arc.lock();
    if system_reference.sender_channels.get(&peer).is_some() {
        Err(NetMsgSubscribeError::RegisterAlreadyRegistered(peer.to_base64()))
    }
    else {
        system_reference.sender_channels.insert(peer, sender);
        Ok(())
    }
}
pub fn drop_channel(peer: &NodeIdentity) -> Result<(), NetMsgSubscribeError>{ 
    let arc = NET_MSG_SYSTEM.clone();
    let mut system_reference = arc.lock();
    system_reference.sender_channels.remove(&peer);
    Ok(())
}

pub fn subscribe_typed<T: NetMsg + Send>(peer: &NodeIdentity) -> Result<NetSendChannel<T>, NetMsgSubscribeError>{ 
    let arc = NET_MSG_SYSTEM.clone();
    let system_reference = arc.lock();
    match system_reference.sender_channels.get(peer) {
        Some(sender) => {
            Ok(NetSendChannel::new(sender.clone()))
        },
        None => Err(NetMsgSubscribeError::NoChannel(peer.to_base64())),
    }
}
pub fn subscribe_untyped(peer: &NodeIdentity) -> Result<UnboundedSender<Vec<PacketIntermediary>>, NetMsgSubscribeError> {
    let arc = NET_MSG_SYSTEM.clone();
    let system_reference = arc.lock();
    match system_reference.sender_channels.get(peer) {
        Some(sender) => {
            Ok(sender.clone())
        },
        None => Err(NetMsgSubscribeError::NoChannel(peer.to_base64())),
    }
}
pub fn send_to_all<T: NetMsg + Send>(message: &T) -> Result<(), NetSendError>{ 
    let arc = NET_MSG_SYSTEM.clone();
    let system_reference = arc.lock();
    for (_peer, channel ) in system_reference.sender_channels.iter() {
        let packet = message.construct_packet()
            .map_err(|e| NetSendError::ConstructPacket(T::net_msg_name(), format!("{:?}", e)))?;
        
        channel.send(vec![packet])
            .map_err(|e| NetSendError::SendOnChannel(T::net_msg_name(), format!("{:?}", e)))?;
    }
    Ok(())
}
/// Send to all peers except the one passed in
/// Used to, for example, avoid telling a client exactly what it just told us. 
pub fn send_to_all_except<T: NetMsg + Send>(message: &T, excluded_peer: &NodeIdentity) -> Result<(), NetSendError> {
    let arc = NET_MSG_SYSTEM.clone();
    let system_reference = arc.lock();
    for (peer, channel ) in system_reference.sender_channels.iter() {
        if peer != excluded_peer {
            let packet = message.construct_packet()
                .map_err(|e| NetSendError::ConstructPacket(T::net_msg_name(), format!("{:?}", e)))?;
            
            channel.send(vec![packet])
                .map_err(|e| NetSendError::SendOnChannel(T::net_msg_name(), format!("{:?}", e)))?;
        }
    }
    Ok(())
}

pub fn send_to<T: NetMsg + Send>(message: &T, peer: &NodeIdentity) -> Result<(), NetSendError>{ 
    let arc = NET_MSG_SYSTEM.clone();
    let system_reference = arc.lock();
    match system_reference.sender_channels.get(peer) {
        Some(sender) => {
            let packet = message.construct_packet()
                .map_err(|e| NetSendError::ConstructPacket(T::net_msg_name(), format!("{:?}", e)))?;
            
            sender.send(vec![packet])
                .map_err(|e| NetSendError::SendOnChannel(T::net_msg_name(), format!("{:?}", e)))?;
                Ok(())
        },
        None => Err(NetSendError::NoChannel(peer.to_base64())),
    }
}

pub fn send_multi_to_all<T: NetMsg + Send>(messages: &Vec<T>) -> Result<(), NetSendError>{ 
    let arc = NET_MSG_SYSTEM.clone();
    let system_reference = arc.lock();

    for (_peer, channel ) in system_reference.sender_channels.iter() {
        
        let mut packets: Vec<PacketIntermediary> = Vec::default();

        for message in messages.iter() {
            let packet = message.construct_packet()
                .map_err(|e| NetSendError::ConstructPacket(T::net_msg_name(), format!("{:?}", e)))?;
            packets.push(packet);
        }

        channel.send(packets)
            .map_err(|e| NetSendError::SendOnChannel(T::net_msg_name(), format!("{:?}", e)))?;
    }
    Ok(())
}

pub fn send_multi_to<T: NetMsg + Send>(messages: &Vec<T>, peer: &NodeIdentity) -> Result<(), NetSendError>{ 
    let arc = NET_MSG_SYSTEM.clone();
    let system_reference = arc.lock();
    match system_reference.sender_channels.get(peer) {
        Some(sender) => {
            let mut packets: Vec<PacketIntermediary> = Vec::default();

            for message in messages.iter() {
                let packet = message.construct_packet()
                    .map_err(|e| NetSendError::ConstructPacket(T::net_msg_name(), format!("{:?}", e)))?;
                packets.push(packet);
            }
            
            sender.send(packets)
                .map_err(|e| NetSendError::SendOnChannel(T::net_msg_name(), format!("{:?}", e)))?;
                Ok(())
        },
        None => Err(NetSendError::NoChannel(peer.to_base64())),
    }
}