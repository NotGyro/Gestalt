use std::{marker::PhantomData};

use crate::message::{MessageSender, SenderAccepts, SendError};

use super::{NetMsg, PacketIntermediary};

pub struct NetSendChannel<T> where T: Send + NetMsg { 
    pub(in crate::net::net_channels) inner: MessageSender<PacketIntermediary>,
    //pub(in crate::net::net_channel) peer_addr: SocketAddr,
    _t: PhantomData<T>,
}

impl<T> NetSendChannel<T>  where T: Send + NetMsg {
    pub fn new(sender: MessageSender<PacketIntermediary>) -> Self { 
        NetSendChannel{ 
            inner: sender,
            //peer_addr,
            _t: PhantomData::default(),
        }
    }
    pub fn send_untyped(&self, packet: PacketIntermediary) -> Result<(), SendError> { 
        self.inner.send_one(packet)
    }
    pub fn send_multi_untyped<V>(&self, packets: V) -> Result<(), SendError> where V: IntoIterator<Item=PacketIntermediary> { 
        self.inner.send_multi(packets)
    }
    
    pub fn resubscribe<U>(&self) -> NetSendChannel<U> where U: Send + NetMsg { 
        NetSendChannel::new(self.inner.clone() )
    }
}

impl<T,R> SenderAccepts<T> for NetSendChannel<R> where T: Clone + Into<R>, R: Clone + Send + NetMsg {
    fn send_multi<V>(&self, messages: V) -> Result<(), crate::message::SendError> where V: IntoIterator<Item=T> {
        let mut packets: Vec<PacketIntermediary> = Vec::default();

        for message in messages {
            let packet = message.into().construct_packet()
                .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", R::net_msg_name(), e)))?;
            packets.push(packet);
        }

        self.inner.send(packets)
            .map_err(|_e| SendError::NoReceivers)?;

        Ok(())
    }
}

pub mod net_msg_channel { 
    use super::*;

    use crate::{common::identity::NodeIdentity, message::{GlobalChannelError, SendError, DomainMultiChannel, sender_subscribe_domain, self, receiver_subscribe_domain, MessageReceiver}};

    domain_channel!(PACKET_TO_SESSION, PacketIntermediary, NodeIdentity, 4096);

    // Subscribe
    pub fn subscribe_sender<T>(peer: &NodeIdentity) -> Result<NetSendChannel<T>, GlobalChannelError>
            where T: Clone + Send + NetMsg {
        let intermediary = sender_subscribe_domain(&PACKET_TO_SESSION, peer)?;
        Ok(NetSendChannel::new(intermediary))
    }
    pub(in crate::net) fn subscribe_receiver(peer: &NodeIdentity) -> Result<MessageReceiver<PacketIntermediary>, GlobalChannelError> {
        receiver_subscribe_domain(&PACKET_TO_SESSION, peer)
    }

    pub fn register_peer(peer: &NodeIdentity) { 
        message::add_domain(&PACKET_TO_SESSION, peer);
    }
    pub fn drop_peer(peer: &NodeIdentity) { 
        message::drop_domain(&PACKET_TO_SESSION, peer);
    }

    // Send helpers
    pub fn send_to<T>(message: T, peer: &NodeIdentity) -> Result<(), SendError>
            where T: NetMsg {

        let packet = message.construct_packet()
            .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
            
        message::send_to(packet, &PACKET_TO_SESSION, peer)
    }

    pub fn send_multi_to<T, V>(messages: V, peer: &NodeIdentity) -> Result<(), SendError>
            where T: NetMsg, V: IntoIterator<Item=T> {
        let mut packets = Vec::new();
        for message in messages {
            let packet = message.construct_packet()
                .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
            packets.push(packet);
        }
        message::send_multi_to(packets, &PACKET_TO_SESSION, peer)
    }

    pub fn send_to_all<T>(message: T) -> Result<(), SendError>
            where T: NetMsg {
        let packet = message.construct_packet()
            .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
        message::send_to_all(packet, &PACKET_TO_SESSION)
    }

    pub fn send_to_all_multi<T, V>(messages: V) -> Result<(), SendError>
            where T: NetMsg, V: IntoIterator<Item=T> {
        let mut packets = Vec::new();
        for message in messages {
            let packet = message.construct_packet()
                .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
            packets.push(packet);
        }
        message::send_to_all_multi(packets, &PACKET_TO_SESSION)
    }

    pub fn send_to_all_except<T>(message: T, exclude: &NodeIdentity) -> Result<(), SendError>
            where T: NetMsg {
        let packet = message.construct_packet()
            .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
        message::send_to_all_except(packet, &PACKET_TO_SESSION, exclude)
    }

    pub fn send_to_all_multi_except<T, C, D, V>(messages: V, channel: &C, exclude: &D) -> Result<(), SendError> 
            where T: NetMsg, V: IntoIterator<Item=T> { 
        let mut packets = Vec::new();
        for message in messages {
            let packet = message.construct_packet()
                .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
            packets.push(packet);
        }
        message::send_to_all_multi(packets, &PACKET_TO_SESSION)
    }
}