
use std::error::Error;
use std::fmt::Debug;
use std::pin::Pin;
use std::ops::Deref;
use std::hash::Hash;

use futures::{Future, TryFutureExt};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::world::WorldId;

use super::identity::NodeIdentity;

#[derive(thiserror::Error, Debug, Clone)]
pub enum SendError {
    #[error("Failed to send a message onto a message channel, because there are no remaining receivers associated with this sender.")]
    NoReceivers,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum RecvError {
    #[error("Failed to send a message onto a message channel, because there are no remaining receivers associated with this sender.")]
    NoSenders,
    #[error("A channel hit its maximum number of stored messages and this channel was keeping alive old messages. {0} messages have been skipped and can no longer be retrieved.")]
    Lagged(u64)
}

pub type MessageSender<T> = tokio::sync::broadcast::Sender<Vec<T>>; 
type UnderlyingReceiver<T> = tokio::sync::broadcast::Receiver<Vec<T>>;

pub struct MessageReceiver<T> where T: Clone {
    inner: UnderlyingReceiver<T>, 
}

impl<T> MessageReceiver<T> where T: Clone {
    pub fn new(to_wrap: tokio::sync::broadcast::Receiver<Vec<T>>) -> Self { 
        MessageReceiver {
            inner: to_wrap,
        }
    }
    /// Nonblockingly polls for new messages, returning an empty vector if the channel is empty.  
    pub fn recv_poll(&mut self) -> Result<Vec<T>, RecvError> { 
        match self.inner.try_recv() { 
            Ok(value) => Ok(value),
            Err(TryRecvError::Empty) => Ok(Vec::new()), 
            Err(TryRecvError::Closed) => Err(RecvError::NoSenders),
            Err(TryRecvError::Lagged(count)) => Err(RecvError::Lagged(count)),
        }
    }
    /// Receives a new message (or several new messages), waiting for a message if the channel is currently empty.
    pub async fn recv_wait(&mut self) -> Result<Vec<T>, RecvError> { 
        self.inner.recv().map_err(|e| match e { 
            broadcast::error::RecvError::Closed => RecvError::NoSenders,
            broadcast::error::RecvError::Lagged(count) => RecvError::Lagged(count),
        }).await
    }
}
/*
pub fn create_channel<T>(capacity: usize) -> (MessageSender<T>, MessageReceiver<T>) where T: Clone { 
    tokio::sync::broadcast::channel(capacity)
}*/
//pub trait Message : Clone + Debug + Send + Sync + Serialize + DeserializeOwned {
//
//}

pub trait ChannelDomain: Clone + PartialEq + Eq + PartialOrd + Hash {}

impl ChannelDomain for WorldId {}
impl ChannelDomain for NodeIdentity {}

pub trait SenderAccepts<T> {
    fn send_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T>;
    fn send_one(&self, message: T) -> Result<(), SendError> { 
        self.send_multi(vec![message])
    }
}

impl<T, R> SenderAccepts<T> for MessageSender<R> where T: Clone + Into<R>, R: Clone { 
    fn send_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T> { 
        self.send(messages.into_iter().map(|val| val.into()).collect()).map_err(|_e| SendError::NoReceivers).map(|_val| ())
    }
}

pub struct MessageChannel<T> where T: Clone {
    sender: MessageSender<T>,
    /// It's a bad idea to just have a copy of a broadcast::Receiver around forever,
    /// because then the channel will be perpetually full even when it doesn't need to be. 
    /// So, we initialize with one, and it immediately gets taken by the first to try to subscribe.
    retained_receiver: Option<UnderlyingReceiver<T>>,
}
impl<T> MessageChannel<T> where T: Clone { 
    /// Argument is how long of a backlog the channel can have. 
    pub fn new(capacity: usize) -> Self { 
        let (sender, receiver) = tokio::sync::broadcast::channel(capacity);
        MessageChannel { 
            sender, 
            retained_receiver: Some(receiver),
        }
    }
    pub fn sender_subscribe(&mut self) -> MessageSender<T> { 
        self.sender.clone()
    }
    pub fn reciever_subscribe(&mut self) -> MessageReceiver<T> { 
        MessageReceiver::new(if self.retained_receiver.is_some() { 
            self.retained_receiver.take().unwrap()
        }
        else { 
            self.sender.subscribe()
        }) 
    }
}

pub struct DomainMultiChannel<T, D> where T: Clone, D: ChannelDomain {
    /// This is carried into any channels we will initialize
    capacity: usize,

    channels: std::collections::HashMap<D, MessageChannel<T>>,
}

impl<T, D> DomainMultiChannel<T, D>  where T: Clone, D: ChannelDomain { 
    /// Argument is how long of a backlog the channel can have. 
    pub fn new(capacity: usize) -> Self {
        DomainMultiChannel {
            capacity,
            channels: std::collections::HashMap::new(),
        }
    }
    pub fn sender_subscribe(&mut self, domain: D) -> MessageSender<T> { 
        let entry = self.channels.entry(domain);
        entry.or_insert_with(|| { MessageChannel::new(self.capacity) }).sender_subscribe()
    }
    pub fn reciever_subscribe(&mut self, domain: D) -> MessageReceiver<T> {
        let entry = self.channels.entry(domain);
        entry.or_insert_with(|| { MessageChannel::new(self.capacity) }).reciever_subscribe()
    }
    pub fn drop_domain(&mut self, domain: D) { 
        self.channels.remove(&domain);
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum GlobalChannelError {
    #[error("Attempted to subscribe on a channel, but the channel's mutex was poisoned.")]
    MutexErr,
}

/// Used to disambiguate from situations where std::sync::Mutex<T> or tokio::sync::Mutex<T> are also being used.
pub type ChannelMutex<T> = parking_lot::Mutex<T>;

// Regular channels 
pub fn sender_subscribe<T, C>(channel: &C) -> Result<MessageSender<T>, GlobalChannelError>
        where T: Clone, C: Deref<Target=crate::message::ChannelMutex<MessageChannel<T>>> { 
    let mut channel_guard = channel.deref().lock();
    let result = channel_guard.sender_subscribe();
    drop(channel_guard);
    Ok(result)
}
pub fn receiver_subscribe<T, C>(channel: &C) -> Result<MessageReceiver<T>, GlobalChannelError>
        where T: Clone, C: Deref<Target=crate::message::ChannelMutex<MessageChannel<T>>> { 
    let mut channel_guard = channel.deref().lock();
    let result = channel_guard.reciever_subscribe();
    drop(channel_guard);
    Ok(result)
}

// Domain-separated channels 
pub fn sender_subscribe_domain<T, C, D>(channel: &C, domain: D) -> Result<MessageSender<T>, GlobalChannelError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let mut channel_guard = channel.deref().lock();
    let result = channel_guard.sender_subscribe(domain);
    drop(channel_guard);
    Ok(result)
}
pub fn receiver_subscribe_domain<T, C, D>(channel: &C, domain: D) -> Result<MessageReceiver<T>, GlobalChannelError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let mut channel_guard = channel.deref().lock();
    let result = channel_guard.reciever_subscribe(domain);
    drop(channel_guard);
    Ok(result)
}

macro_rules! channel {
    ($name:ident, $message:ty, $capacity:expr) => {
        lazy_static!{
            pub static ref $name: ChannelMutex<MessageChannel<$message>> = {
                ChannelMutex::new(MessageChannel::new($capacity))
            };
        }
    };
}
macro_rules! channel_domain {
    ($name:ident, $message:ty, $domain:ty, $capacity:expr) => {
        lazy_static!{
            pub static ref $name: ChannelMutex<DomainMultiChannel<$message, $domain>> = {
                ChannelMutex::new(DomainMultiChannel::new($capacity))
            };
        }
    };
}

#[cfg(test)]
pub mod test { 
    //use std::sync::Mutex;

    use crate::common::identity::IdentityKeyPair;

    use super::*; 
    use lazy_static::lazy_static;

    #[derive(Clone)]
    pub struct MessageA { 
        pub msg: String,
    }

    #[derive(Clone)]
    pub struct MessageB { 
        pub msg: String,
    }

    channel!(TEST_CHANNEL, MessageA, 16);

    channel_domain!(TEST_DOMAIN_CHANNEL, MessageB, NodeIdentity, 16);

    #[tokio::test(flavor = "multi_thread")]
    async fn send_into() { 
        #[derive(Debug, Clone)]
        struct Foo { 
            first: u32,
        }
        #[derive(Debug, Clone)]
        struct Bar { 
            second: u64
        }
    
        impl Into<Bar> for Foo {
            fn into(self) -> Bar {
                Bar { 
                    second: self.first as u64,
                }
            }
        }
    
        let test_struct = Foo { first: 1234 }; 
    
        let mut channel: MessageChannel<Bar> = MessageChannel::new(16);
        let sender = channel.sender_subscribe();
        let mut receiver = channel.reciever_subscribe();
        let mut second_receiver = channel.reciever_subscribe();
        //send_one
        sender.send_one(test_struct).unwrap();
    
        let out = receiver.recv_wait().await.unwrap(); 
        let out = out.first().unwrap();
        assert_eq!(out.second, 1234);
    
        let out2 = second_receiver.recv_wait().await.unwrap(); 
        let out2 = out2.first().unwrap();
        assert_eq!(out2.second, 1234);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn global_subscribe() {
        let sender = sender_subscribe(&TEST_CHANNEL).unwrap();
        let mut receiver = receiver_subscribe(&TEST_CHANNEL).unwrap();

        sender.send_one(MessageA{ msg: String::from("Hello, world!") }).unwrap(); 
        let mut output = receiver.recv_wait().await.unwrap();
        assert_eq!(output.len(), 1);
        let out_msg = output.drain(0..1).next().unwrap();

        assert_eq!(out_msg.msg, String::from("Hello, world!"));
    }
    
    #[tokio::test(flavor = "multi_thread")]
    async fn domain_channels() {
        let player_identity = IdentityKeyPair::generate_for_tests().public;
        let some_other_player_identity = IdentityKeyPair::generate_for_tests().public;

        let sender = sender_subscribe_domain(&TEST_DOMAIN_CHANNEL, player_identity.clone()).unwrap();
        let mut receiver = receiver_subscribe_domain(&TEST_DOMAIN_CHANNEL, player_identity.clone()).unwrap();

        let other_channel_sender = sender_subscribe_domain(&TEST_DOMAIN_CHANNEL, some_other_player_identity.clone()).unwrap();
        let mut other_channel_receiver = receiver_subscribe_domain(&TEST_DOMAIN_CHANNEL, some_other_player_identity.clone()).unwrap();

        sender.send_one(MessageB{ msg: String::from("Hello, player1!") }).unwrap(); 
        other_channel_sender.send_one(MessageB{ msg: String::from("Hello, player2!") }).unwrap(); 

        {
            let mut output = receiver.recv_wait().await.unwrap();
            assert_eq!(output.len(), 1);
            let out_msg = output.drain(0..1).next().unwrap();
            assert_eq!(out_msg.msg, String::from("Hello, player1!"));
        }
        
        {
            let mut output = other_channel_receiver.recv_wait().await.unwrap();
            assert_eq!(output.len(), 1);
            let out_msg = output.drain(0..1).next().unwrap();
            assert_eq!(out_msg.msg, String::from("Hello, player2!"));
        }
    }
}