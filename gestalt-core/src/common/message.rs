use std::fmt::Debug;
use std::ops::Deref;
use std::hash::Hash;

use futures::{TryFutureExt};
use log::error;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::sync::broadcast;

use crate::world::WorldId;

use super::identity::NodeIdentity;

#[derive(thiserror::Error, Debug, Clone)]
pub enum SendError {
    #[error("Failed to send a message onto a message channel, because there are no remaining receivers associated with this sender.")]
    NoReceivers,
    #[error("Could not send on a channel because domain {0} is not registered yet")]
    MissingDomain(String),
    #[error("Could not receive new incoming domains on a multi-domain sender.")]
    CouldNotRecvDomains,
    #[error("Unable to encode a message so it could be sent on channel: {0}.")]
    Encode(String),
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
    pub(in crate::common::message) inner: UnderlyingReceiver<T>, 
}

impl<T> MessageReceiver<T> where T: Clone {
    pub fn new(to_wrap: tokio::sync::broadcast::Receiver<Vec<T>>) -> Self { 
        MessageReceiver {
            inner: to_wrap,
        }
    }

    /// Nonblockingly polls for new messages, returning an empty vector if the channel is empty.  
    pub fn recv_poll(&mut self) -> Result<Vec<T>, RecvError> { 
        let mut results: Vec<T> = Vec::new();
        let mut next_value = self.inner.try_recv();
        while let Ok(mut val) = next_value { 
            if results.is_empty() { 
                results = val; 
            }
            else {
                results.append(&mut val);
            }
            next_value = self.inner.try_recv();
        }
        if let Err(err) = next_value { 
            match err { 
                TryRecvError::Empty => {}, 
                TryRecvError::Closed => return Err(RecvError::NoSenders),
                TryRecvError::Lagged(count) => return Err(RecvError::Lagged(count))
            }
        }
        Ok(results)
    }

    /// Receives new messages batch, waiting for a message if the channel is currently empty.
    pub async fn recv_wait(&mut self) -> Result<Vec<T>, RecvError> { 
        let mut resl = self.inner.recv().map_err(|e| match e { 
            broadcast::error::RecvError::Closed => RecvError::NoSenders,
            broadcast::error::RecvError::Lagged(count) => RecvError::Lagged(count),
        }).await?;
        // Check to see if there's anything else also waiting for us, but do not block for it.  
        let mut maybe_more = self.recv_poll()?;
        resl.append(&mut maybe_more);
        Ok(resl)
    }
}

pub trait ChannelDomain: Clone + PartialEq + Eq + PartialOrd + Hash + Debug {}

impl ChannelDomain for WorldId {}
impl ChannelDomain for NodeIdentity {}

pub trait MessageHasDomain<D> where D: ChannelDomain { 
    fn get_domain(&self) -> &D;
}

impl<T,D> MessageHasDomain<D> for (T, D) where T: Clone, D: ChannelDomain {
    fn get_domain(&self) -> &D {
        &self.1
    }
}

pub trait SenderAccepts<T> {
    fn send_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T>;
    fn send_one(&self, message: T) -> Result<(), SendError> {
        self.send_multi(vec![message])
    }
}

impl<T, R> SenderAccepts<T> for MessageSender<R> where T: Into<R>, R: Clone { 
    fn send_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T> { 
        self.send(messages.into_iter().map(|val| val.into()).collect()).map_err(|_e| SendError::NoReceivers).map(|_val| ())
    }
}

pub trait SenderDomainAccepts<T, D> where D: ChannelDomain {
    fn send_multi_to<V>(&self, messages: V, domain: &D) -> Result<(), SendError> where V: IntoIterator<Item=T>;
    fn send_one_to(&self, message: T, domain: &D) -> Result<(), SendError> {
        self.send_multi_to(vec![message], domain)
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

impl<T, R> SenderAccepts<T> for MessageChannel<R> where T: Into<R>, R: Clone { 
    fn send_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T> { 
        self.sender.send(messages.into_iter().map(|val| val.into()).collect()).map_err(|_e| SendError::NoReceivers).map(|_val| ())
    }
}
/*
pub struct MultiDomainSender<T, D> where T: Clone, D: ChannelDomain { 
    pub known_domains: RefCell<HashMap<D, MessageSender<T>>>,
    /// Half the point of having message-passing channels is so you can give different
    /// threads different recievers and senders, such that they don't have to access
    /// the same object from more than one thread at once.
    /// So, very few downsides from using a RefCell here.
    new_domain_receiver: RefCell<broadcast::Receiver<(D, MessageSender<T>)>>, 
    dropped_domain_receiver: RefCell<broadcast::Receiver<D>>, 
}

impl<T,D> MultiDomainSender<T,D> where T: Clone, D: ChannelDomain {
    pub(in crate::common::message) fn new(starting_domains: HashMap<D, MessageSender<T>>, new_domain_receiver: broadcast::Receiver<(D, MessageSender<T>)>, dropped_domain_receiver: broadcast::Receiver<D>) -> Self { 
        MultiDomainSender {
            known_domains: RefCell::new(starting_domains), 
            new_domain_receiver: RefCell::new(new_domain_receiver),
            dropped_domain_receiver: RefCell::new(dropped_domain_receiver),
        }
    }
    fn process_dropped_domains(&self) { 
        while let Ok(domain) = self.dropped_domain_receiver.borrow_mut().try_recv() { 
            self.known_domains.borrow_mut().remove(&domain);
        }
    }
    fn ingest_new_domains(&self) { 
        while let Ok((domain, sender)) = self.new_domain_receiver.borrow_mut().try_recv() { 
            self.known_domains.borrow_mut().insert(domain, sender);
        }
    }
    pub fn send_multi_to<V>(&self, messages: V, domain: &D) -> Result<(), SendError> where V: IntoIterator<Item=T> { 
        self.ingest_new_domains();
        match self.known_domains.borrow().get(domain) { 
            Some(chan) => { 
                chan.send(messages.into_iter().collect()).map_err(|_e| SendError::NoReceivers).map(|_val| ())
            }, 
            None => { 
                Err(SendError::MissingDomain(format!("{:?}", domain)))
            }
        }
    }

    pub fn send_one_to(&self, message: T, domain: &D) -> Result<(), SendError> {
        self.send_multi_to( vec![message], domain )
    }
    
    pub fn send_to_all_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T> {
        self.ingest_new_domains();
        let messagebuf: Vec<T> = messages.into_iter().collect(); 
        for domain_channel in self.known_domains.borrow().values() { 
            domain_channel.send(messagebuf.clone()).map_err(|_e| SendError::NoReceivers).map(|_val| ())?;
        }
        Ok(())
    }

    pub fn send_to_all_one(&self, message: T) -> Result<(), SendError> {
        self.ingest_new_domains();
        let messagebuf: Vec<T> = vec![message]; 
        for domain_channel in self.known_domains.borrow().values() { 
            domain_channel.send(messagebuf.clone()).map_err(|_e| SendError::NoReceivers).map(|_val| ())?;
        }
        Ok(())
    }
}

impl<T, D, R> SenderAccepts<T> for MultiDomainSender<R, D> where T: Clone + MessageHasDomain<D> + Into<R>, D: ChannelDomain, R: Clone { 
    fn send_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T> {
        // Unfortunately messages could have different domains here so this breaks batching. 
        for message in messages { 
            let domain = message.get_domain().clone(); 
            self.send_one_to(message.into(), &domain)?;
        }
        Ok(())
    }
}*/

#[derive(thiserror::Error, Debug, Clone)]
pub enum DomainSubscribeErr<D> where D: ChannelDomain {
    #[error("Cannot subscribe to a channel in domain {0:?} because that domain has not been registered.")]
    NoDomain(D),
}

pub struct DomainMultiChannel<T, D> where T: Clone, D: ChannelDomain {
    /// This is carried into any channels we will initialize
    capacity: usize,

    channels: std::collections::HashMap<D, MessageChannel<T>>,

    // Used to notify multidomain senders we're adding a domain. 
    //new_domain_sender: broadcast::Sender<(D, MessageSender<T>)>,
    // Kept around to keep the channel from being dropped
    //new_domain_receiver: broadcast::Receiver<(D, MessageSender<T>)>,
    // A similar pair
    //dropped_domain_sender: broadcast::Sender<D>,
    //dropped_domain_receiver: broadcast::Receiver<D>,
}

impl<T, D> DomainMultiChannel<T, D>  where T: Clone, D: ChannelDomain {
    /// Argument is how long of a backlog the channel can have. 
    pub fn new(capacity: usize) -> Self {
        //let (new_domain_sender, new_domain_receiver) = broadcast::channel(capacity); 
        //let (dropped_domain_sender, dropped_domain_receiver) = broadcast::channel(capacity); 
        DomainMultiChannel {
            capacity,
            channels: std::collections::HashMap::new(),
            //new_domain_sender,
            //new_domain_receiver,
            //dropped_domain_sender,
            //dropped_domain_receiver,
        }
    }

    pub fn sender_subscribe(&mut self, domain: &D) -> Result<MessageSender<T>, DomainSubscribeErr<D>> {
        Ok(self.channels.get_mut(domain)
            .ok_or_else(|| {DomainSubscribeErr::NoDomain(domain.clone())} )?
            .sender_subscribe())
    }
    
    pub fn reciever_subscribe(&mut self, domain: &D) -> Result<MessageReceiver<T>, DomainSubscribeErr<D>> {
        Ok(self.channels.get_mut(domain)
            .ok_or_else(|| {DomainSubscribeErr::NoDomain(domain.clone())} )?
            .reciever_subscribe())
    }

    /*
    pub fn sender_subscribe_all(&mut self) -> MultiDomainSender<T,D> {
        let subscribe_all: HashMap<D, MessageSender<T>> = self.channels.iter_mut().map(|(k, v)| {
            (k.clone(), v.sender_subscribe())
        }).collect();
        MultiDomainSender::new(subscribe_all, 
            self.new_domain_sender.subscribe(),
            self.dropped_domain_sender.subscribe())
    }*/

    /// Adds a new domain if it isn't there yet, takes no action if one is already present. 
    pub fn add_domain(&mut self, domain: &D) {
        if !self.channels.contains_key(domain) {
            self.channels.entry(domain.clone()).or_insert(MessageChannel::new(self.capacity));
            /*let resl = self.new_domain_sender.send((domain.clone(), channelref.sender_subscribe()));
            //Ensure our own keepalive receiver doesn't clog up the system. 
            let _ = self.new_domain_receiver.try_recv();
            if let Err(e) = resl {
                error!("Error notifying MultiDomainSenders about a new domain: {:?}", e);
            }*/
        }
    }
    pub fn drop_domain(&mut self, domain: &D) { 
        if self.channels.contains_key(domain) {
            self.channels.remove(domain);
            
            /*let resl = self.dropped_domain_sender.send(domain.clone());
            //Ensure our own keepalive receiver doesn't clog up the system. 
            let _ = self.dropped_domain_receiver.try_recv();
            if let Err(e) = resl {
                error!("Error notifying MultiDomainSenders that a domain has been dropped: {:?}", e);
            }*/
        }
    }

    pub fn send_to_all_one(&self, message: T) -> Result<(), SendError> { 
        for chan in self.channels.values() { 
            chan.send_one(message.clone())?;
        }
        Ok(())
    }
    fn send_to_all_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T> {
        let message_buf: Vec<T> = messages.into_iter().collect();
        for chan in self.channels.values() {
            chan.send_multi(message_buf.clone())?;
        }
        Ok(())
    }
    pub fn send_to_all_except(&self, message: T, exclude: &D) -> Result<(), SendError> { 
        for (domain, chan) in self.channels.iter() { 
            if domain != exclude {
                chan.send_one(message.clone())?;
            }
        }
        Ok(())
    }
    fn send_to_all_multi_except<V>(&self, messages: V, exclude: &D) -> Result<(), SendError> where V: IntoIterator<Item=T> {
        let message_buf: Vec<T> = messages.into_iter().collect();
        for (domain, chan) in self.channels.iter() {
            if domain != exclude { 
                chan.send_multi(message_buf.clone())?;
            }
        }
        Ok(())
    }
}

impl<T,D,R> SenderDomainAccepts<T, D> for DomainMultiChannel<R,D> where T: Into<R>, D: ChannelDomain, R: Clone {
    fn send_multi_to<V>(&self, messages: V, domain: &D) -> Result<(), SendError> where V: IntoIterator<Item=T> { 
        match self.channels.get(domain) { 
            Some(chan) => { 
                chan.send_multi(messages).map_err(|_e| SendError::NoReceivers).map(|_val| ())
            }, 
            None => {
                Err(SendError::MissingDomain(format!("{:?}", domain)))
            }
        }
    }
}

impl<T, D, R> SenderAccepts<T> for DomainMultiChannel<R, D> where T: Into<R> + MessageHasDomain<D>, D: ChannelDomain, R: Clone + MessageHasDomain<D> { 
    fn send_multi<V>(&self, messages: V) -> Result<(), SendError> where V: IntoIterator<Item=T> { 
        for message in messages {
            let message = message.into();
            let domain = message.get_domain().clone();
            self.send_one_to(message, &domain).map_err(|_e| SendError::NoReceivers)?;
        }
        Ok(())
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum GlobalChannelError {
    #[error("Attempted to subscribe on a channel, but the channel's mutex was poisoned.")]
    MutexErr,
    #[error("Could not subscribe to a channel, separated into domains, due to an error: {0}")]
    DomainSubscribe(String),
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

//Manual sends for regular global channels
pub fn send_multi<T, V, C>(messages: V, channel: &C) -> Result<(), SendError> 
        where V: IntoIterator<Item=T>, T: Clone, C: Deref<Target=crate::message::ChannelMutex<MessageChannel<T>>> { 
    let channel_guard = channel.deref().lock();
    let resl = channel_guard.send_multi(messages);
    drop(channel_guard);
    resl
}

pub fn send_one<T, C>(message: T, channel: &C) -> Result<(), SendError> 
        where T: Clone, C: Deref<Target=crate::message::ChannelMutex<MessageChannel<T>>> { 
    send_multi(vec![message], channel)
}

// Domain-separated channels 
pub fn add_domain<T, C, D>(channel: &C, domain: &D)
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let mut channel_guard = channel.deref().lock();
    channel_guard.add_domain(domain);
}
pub fn drop_domain<T, C, D>(channel: &C, domain: &D)
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let mut channel_guard = channel.deref().lock();
    channel_guard.drop_domain(domain);
}
pub fn sender_subscribe_domain<T, C, D>(channel: &C, domain: &D) -> Result<MessageSender<T>, GlobalChannelError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let mut channel_guard = channel.deref().lock();
    let result = channel_guard.sender_subscribe(&domain)
        .map_err(|e| GlobalChannelError::DomainSubscribe( format!("{:?}", e) ));
    drop(channel_guard);
    result
}
pub fn receiver_subscribe_domain<T, C, D>(channel: &C, domain: &D) -> Result<MessageReceiver<T>, GlobalChannelError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let mut channel_guard = channel.deref().lock();
    let result = channel_guard.reciever_subscribe(&domain)
        .map_err(|e| GlobalChannelError::DomainSubscribe( format!("{:?}", e) ));
    drop(channel_guard);
    result
}
// Manually send a message on a global domain channel. 
pub fn send_to<T, C, D>(message: T, channel: &C, domain: &D) -> Result<(), SendError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let channel_guard = channel.deref().lock();
    channel_guard.send_one_to(message, domain)?;
    drop(channel_guard);
    Ok(())
}

pub fn send_multi_to<T, C, D, V>(messages: V, channel: &C, domain: &D) -> Result<(), SendError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>>, V: IntoIterator<Item=T> { 
    let channel_guard = channel.deref().lock();
    channel_guard.send_multi_to(messages, domain)?;
    drop(channel_guard);
    Ok(())
}

pub fn send_to_all<T, C, D>(message: T, channel: &C) -> Result<(), SendError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let channel_guard = channel.deref().lock();
    channel_guard.send_to_all_one(message)?;
    drop(channel_guard);
    Ok(())
}

pub fn send_to_all_multi<T, C, D, V>(messages: V, channel: &C) -> Result<(), SendError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>>, V: IntoIterator<Item=T> { 
    let channel_guard = channel.deref().lock();
    channel_guard.send_to_all_multi(messages)?;
    drop(channel_guard);
    Ok(())
}

pub fn send_to_all_except<T, C, D>(message: T, channel: &C, exclude: &D) -> Result<(), SendError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>> { 
    let channel_guard = channel.deref().lock();
    channel_guard.send_to_all_except(message, exclude)?;
    drop(channel_guard);
    Ok(())
}

pub fn send_to_all_multi_except<T, C, D, V>(messages: V, channel: &C, exclude: &D) -> Result<(), SendError>
        where T: Clone, D: ChannelDomain, C: Deref<Target=crate::message::ChannelMutex<DomainMultiChannel<T, D>>>, V: IntoIterator<Item=T> { 
    let channel_guard = channel.deref().lock();
    channel_guard.send_to_all_multi_except(messages, exclude)?;
    drop(channel_guard);
    Ok(())
}

macro_rules! channel {
    ($name:ident, $message:ty, $capacity:expr) => {
        lazy_static::lazy_static!{
            pub static ref $name: crate::common::message::ChannelMutex<MessageChannel<$message>> = {
                crate::common::message::ChannelMutex::new(crate::common::message::MessageChannel::new($capacity))
            };
        }
    };
}
macro_rules! domain_channel {
    ($name:ident, $message:ty, $domain:ty, $capacity:expr) => {
        lazy_static::lazy_static!{
            pub static ref $name: crate::common::message::ChannelMutex<DomainMultiChannel<$message, $domain>> = {
                crate::common::message::ChannelMutex::new(crate::common::message::DomainMultiChannel::new($capacity))
            };
        }
    };
}

// A few *very universal* channels can exist in this file. 

channel!(START_SHUTDOWN, (), 1);

/// This async function is cancel-safe and awaits until the end of this session. 
/// It will not return until something has been sent on START_SHUTDOWN.
pub async fn at_quit() {
    let mut receiver = receiver_subscribe(&START_SHUTDOWN).unwrap();
    let _ = receiver.recv_wait().await;
    ()
}

#[cfg(test)]
pub mod test { 
    //use std::sync::Mutex;

    use crate::common::identity::IdentityKeyPair;

    use super::*;

    #[derive(Clone)]
    pub struct MessageA { 
        pub msg: String,
    }

    #[derive(Clone)]
    pub struct MessageB { 
        pub msg: String,
    }

    channel!(TEST_CHANNEL, MessageA, 16);

    domain_channel!(TEST_DOMAIN_CHANNEL, MessageB, NodeIdentity, 16);

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

        add_domain(&TEST_DOMAIN_CHANNEL, &player_identity);
        add_domain(&TEST_DOMAIN_CHANNEL, &some_other_player_identity);
        let sender = sender_subscribe_domain(&TEST_DOMAIN_CHANNEL, &player_identity).unwrap();
        let mut receiver = receiver_subscribe_domain(&TEST_DOMAIN_CHANNEL, &player_identity).unwrap();

        let other_channel_sender = sender_subscribe_domain(&TEST_DOMAIN_CHANNEL, &some_other_player_identity).unwrap();
        let mut other_channel_receiver = receiver_subscribe_domain(&TEST_DOMAIN_CHANNEL, &some_other_player_identity).unwrap();

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
    
    
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct MessageC { 
        pub msg: String,
        pub val: u64,
    }

    channel!(TEST_CHANNEL_C, MessageC, 128);

    #[tokio::test(flavor = "multi_thread")]
    async fn message_batching() {
        let sender = sender_subscribe(&TEST_CHANNEL_C).unwrap();
        let mut receiver = receiver_subscribe(&TEST_CHANNEL_C).unwrap();

        const NUM_MESSAGES: usize = 64;
        //Many separate sends...
        for i in 0..NUM_MESSAGES as u64 { 
            sender.send_one(MessageC{ msg: String::from("Hello, world!"), val: i }).unwrap();
        }
        
        let output = receiver.recv_poll().unwrap();
        assert_eq!(output.len(), NUM_MESSAGES);
        assert_eq!(receiver.inner.try_recv(), Err(TryRecvError::Empty) ); 

        //assert_eq!(out_msg.msg, String::from("Hello, world!"));
    }
}