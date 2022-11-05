use std::{marker::PhantomData};

use crate::{message::{MessageSender, BroadcastSender, SendError, self, BroadcastChannel, Message}, net::{InboundNetMsg, NetMsgDomain}, common::identity::NodeIdentity};

use self::net_send_channel::PACKET_TO_SESSION;

use super::{NetMsg, PacketIntermediary};

pub struct NetSendChannel<T> where T: Send + NetMsg { 
    pub(in crate::net::net_channels) inner: BroadcastSender<PacketIntermediary>,
    //pub(in crate::net::net_channel) peer_addr: SocketAddr,
    _t: PhantomData<T>,
}

impl<T> NetSendChannel<T>  where T: Send + NetMsg {
    pub fn new(sender: BroadcastSender<PacketIntermediary>) -> Self { 
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

impl<T,R> MessageSender<T> for NetSendChannel<R> where T: Message + Into<R>, R: Message + Send + NetMsg {
    fn send_multi<V>(&self, messages: V) -> Result<(), crate::message::SendError> where V: IntoIterator<Item=T> {
        let mut packets: Vec<PacketIntermediary> = Vec::default();

        for message in messages {
            let packet = message.into().construct_packet()
                .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", R::net_msg_name(), e)))?;
            packets.push(packet);
        }

        self.send_multi_untyped(packets)
            .map_err(|_e| SendError::NoReceivers)?;

        Ok(())
    }

    fn would_block(&self) -> bool {
        self.inner.would_block()
    }
}

pub mod net_send_channel { 
    use super::*;

    use crate::{common::identity::NodeIdentity, message::{GlobalChannelError, SendError, sender_subscribe_domain, self, receiver_subscribe_domain, BroadcastReceiver, BroadcastChannel}};

    global_domain_channel!(BroadcastChannel, PACKET_TO_SESSION, PacketIntermediary, NodeIdentity, 4096);

    // Subscribe
    pub fn subscribe_sender<T>(peer: &NodeIdentity) -> Result<NetSendChannel<T>, GlobalChannelError>
            where T: Clone + Send + NetMsg {
        let intermediary = sender_subscribe_domain(&PACKET_TO_SESSION, peer)?;
        Ok(NetSendChannel::new(intermediary))
    }
    pub(in crate::net) fn subscribe_receiver(peer: &NodeIdentity) -> Result<BroadcastReceiver<PacketIntermediary>, GlobalChannelError> {
        receiver_subscribe_domain(&PACKET_TO_SESSION, peer)
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

    pub fn send_one_to_all<T>(message: T) -> Result<(), SendError>
            where T: NetMsg {
        let packet = message.construct_packet()
            .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
        message::send_one_to_all(packet, &PACKET_TO_SESSION)
    }

    pub fn send_multi_to_all<T, V>(messages: V) -> Result<(), SendError>
            where T: NetMsg, V: IntoIterator<Item=T> {
        let mut packets = Vec::new();
        for message in messages {
            let packet = message.construct_packet()
                .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
            packets.push(packet);
        }
        message::send_multi_to_all(packets, &PACKET_TO_SESSION)
    }

    pub fn send_one_to_all_except<T>(message: T, exclude: &NodeIdentity) -> Result<(), SendError>
            where T: NetMsg {
        let packet = message.construct_packet()
            .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
        message::send_one_to_all_except(packet, &PACKET_TO_SESSION, exclude)
    }

    pub fn send_multi_to_all_except<T, C, D, V>(messages: V, exclude: &NodeIdentity) -> Result<(), SendError> 
            where T: NetMsg, V: IntoIterator<Item=T> { 
        let mut packets = Vec::new();
        for message in messages {
            let packet = message.construct_packet()
                .map_err(|e| SendError::Encode(format!("Could not convert packet of type {} into a packet intermediary: {:?}", T::net_msg_name(), e)))?;
            packets.push(packet);
        }
        message::send_multi_to_all_except(packets, &PACKET_TO_SESSION, exclude)
    }
}

pub const NET_MSG_CHANNEL_CAPACITY: usize = 1024;

global_domain_channel!(BroadcastChannel, INBOUND_NET_MESSAGES, InboundNetMsg, NetMsgDomain, NET_MSG_CHANNEL_CAPACITY);

pub type InboundMsgSender = BroadcastSender<InboundNetMsg>; 

pub mod net_recv_channel {
    use std::marker::PhantomData;

    use crate::{net::{InboundNetMsg, NetMsg, netmsg::NetMsgRecvError}, message::{BroadcastReceiver, GlobalChannelError, receiver_subscribe_domain, self, MessageReceiver, MessageReceiverAsync}, common::identity::NodeIdentity};

    use super::INBOUND_NET_MESSAGES;


    pub struct NetMsgReceiver<T> { 
        pub inner: BroadcastReceiver<InboundNetMsg>,
        _t: PhantomData<T>,
    }
    impl<T: NetMsg> NetMsgReceiver<T> { 
        pub fn new(inner: BroadcastReceiver<InboundNetMsg>) -> Self { 
            NetMsgReceiver { 
                inner, 
                _t: PhantomData::default(),
            }
        }
        
        pub(crate) fn decode(inbound: Vec<InboundNetMsg>) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> {
            let mut output = Vec::with_capacity(inbound.len());
            for message in inbound { 
                if T::net_msg_id() != message.message_type_id { 
                    return Err(NetMsgRecvError::WrongType(T::net_msg_id(), T::net_msg_name(), message.message_type_id));
                }
                else {
                    let InboundNetMsg{peer_identity, message_type_id: _, payload } = message;
                    let payload: T = rmp_serde::from_read(&payload[..])?;
                    output.push((peer_identity, payload));
                }
            }
            Ok(output)
        }

        pub fn recv_poll(&mut self) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> { 
            Self::decode(
                self.inner.recv_poll()?
            )
        }

        pub async fn recv_wait(&mut self) -> Result<Vec<(NodeIdentity, T)>, NetMsgRecvError> { 
            Self::decode( self.inner.recv_wait().await? ) 
        }

        pub fn resubscribe<U>(&self) -> NetMsgReceiver<U> where U: NetMsg { 
            NetMsgReceiver{ 
                inner: self.inner.resubscribe(),
                _t: PhantomData::default(),
            }
        }
    }
    
    pub fn subscribe<T>() -> Result<NetMsgReceiver<T>, GlobalChannelError> where T: NetMsg {
        //let domain = &(peer.clone(), );
        message::add_domain(&INBOUND_NET_MESSAGES, &T::net_msg_id());
        receiver_subscribe_domain(&INBOUND_NET_MESSAGES, &T::net_msg_id()).map(|inner| { 
            NetMsgReceiver::new(inner)
        })
    }
    // TODO: Better system of net messge registration.
}

pub fn register_peer(peer: &NodeIdentity) { 
    message::add_domain(&PACKET_TO_SESSION, peer);
}
pub fn drop_peer(peer: &NodeIdentity) { 
    message::drop_domain(&PACKET_TO_SESSION, peer);
}
