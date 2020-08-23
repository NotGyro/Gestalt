use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::result::Result;
use std::error::Error;
use std::fmt::Debug;
use std::collections::VecDeque;
use std::thread::Thread;
use std::thread;
use std::io::*;
use std::time::Duration;

use ustr::*;
use crossbeam_channel::*;
use custom_error::custom_error;
use hashbrown::HashMap;
//use linear_map::LinearMap;
use parking_lot::Mutex;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

// Dependencies for testing
use rand::Rng;
use rand::thread_rng;

/// Runtime type identifier for a type of message.
pub type MsgTypeId = Ustr;
pub type MsgData = Vec<u8>;

pub type ChannelId = Ustr;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    //Message type
    pub ty: MsgTypeId,
    //Message data
    pub data: MsgData,
}

pub trait RegisteredMessage: Clone + Debug + Serialize + DeserializeOwned + Send + Sync {
    fn msg_ty() -> MsgTypeId;
    fn unpack(msg: &MsgData) -> Result<Self, Box<dyn Error>>;
    fn construct_message(&self) -> Result<Message, Box<dyn Error>>;
    fn unpack_from(msg: Message) -> Result<Self, Box<dyn Error>>;
}

//MsgSender gets to be pretty lightweight. MsgReceiver wishes it could be this lucky.
///Thin wrapper over a crossbeam::Sender<Message>.
#[derive(Clone)]
pub struct MsgSender(Sender<Message>);
impl MsgSender {
    #[inline(always)]
    pub fn send<T: RegisteredMessage>(&self, to_send: T) -> Result<(), Box<dyn Error>> { 
        Ok(self.send_raw(to_send.construct_message()?)?)
    }
    #[inline(always)]
    pub fn send_raw(&self, to_send: Message) -> Result<(), Box<dyn Error>> { 
        Ok(self.0.send(to_send)?)
    }
}

//Used to specify supported messages, or type of messages requested. 
#[derive(Clone, PartialEq, Eq)]
pub enum MsgTypeFilter {
    Any,
    Single(MsgTypeId),
    Multi(UstrSet),
}

impl MsgTypeFilter {
    #[inline(always)]
    pub fn suitable(&self, ty: &MsgTypeId) -> bool {
        match &self { 
            MsgTypeFilter::Any => true,
            MsgTypeFilter::Single(our_ty) => ty == our_ty,
            MsgTypeFilter::Multi(list) => list.contains(ty),
        }
    }
}

struct MsgQueueThreaded {
    pub queue: Mutex<Vec<Message>>,
    pub is_empty: AtomicBool,
}
impl MsgQueueThreaded {
    ///Non-blockingly attempts to lock the mutex. Returns None if empty or cannot lock.
    #[inline(always)]
    pub fn poll(&self) -> Option<VecDeque<Message>> {
        if self.is_empty.load(Ordering::Relaxed) {
            return None;
        }
        else if let Some(mut guard) = self.queue.try_lock() {
            self.is_empty.store(true, Ordering::Release);
            return Some(guard.drain( .. ).collect());
        }
        else {
            return None;
        }
    }
    ///Blockingly sends
    #[inline(always)]
    pub fn send(&self, msg: Message) {
        let mut guard = self.queue.lock();
        guard.push(msg);
        self.is_empty.store(false, Ordering::Release);
    }
}

type MsgReceiverInternal = Arc<MsgQueueThreaded>;

pub struct MsgReceiver {
    receiver: MsgReceiverInternal,
    /// We maintain our own (cached?) queue so that we don't have the overhead of locking a mutex on every invocation of poll().
    pub our_queue: VecDeque<Message>,
}
impl MsgReceiver {
    #[inline(always)]
    fn poll_inner(&mut self) {
        if let Some(ref mut rece) = self.receiver.poll() {
            self.our_queue.append(rece);
        }
    }
    #[inline(always)]
    pub fn poll(&mut self) -> Option<Message> { 
        self.poll_inner();
        self.our_queue.pop_front()
    }
    /// Polls to get the most recent event of an even type that mactches filter
    #[inline]
    pub fn poll_filtered(&mut self, filter: &MsgTypeFilter) -> Option<Message> { 
        if *filter == MsgTypeFilter::Any { 
            self.poll()
        }
        else {
            self.poll_inner();
            let mut res : Option<Message> = None;
            let mut pop_idx = 0;
        
            //See if we've got one.
            for index in 0..self.our_queue.len() {
                if filter.suitable(&self.our_queue[index].ty) {
                    res = Some(self.our_queue[index].clone());
                    pop_idx = index;
                    break;
                }
            }

            //Pop the one we got if we got it.
            if res.is_some() {
                self.our_queue.remove(pop_idx);
            }

            res
        }
    }
    /// Polls to get the most recent event of type T
    #[inline(always)]
    pub fn poll_to<T: RegisteredMessage>(&mut self) -> Option<T> {
        self.poll_filtered(&MsgTypeFilter::Single(T::msg_ty())).map(|m| T::unpack(&m.data).ok() ).flatten()
    }
}

/// An event bus that multicasts incoming events out to all consumers.
pub struct EventBus { 
    /// This is where events sent to the bus / journal will go.
    our_receiver : Receiver<Message>,
    /// Used to clone repeatedly for senders to this bus
    sender_template : Sender<Message>,
    /// A list of registered subscribers. Each receiving queue is owned by the consumer.
    subscribers : Vec<MsgReceiverInternal>
}

impl EventBus {
    #[allow(dead_code)]
    pub fn new() -> EventBus {
        let (s_in, r_in) = unbounded();
        EventBus { our_receiver : r_in, sender_template : s_in, subscribers : Vec::new(),}
    }

    /// Gives you a Crossbeam Sender to push events to this bus.
    #[allow(unused_mut)]
    pub fn get_sender(&mut self) -> MsgSender {
        MsgSender(self.sender_template.clone())
    }

    /// Gives you a Crossbeam Receiver where you can poll events from this bus, and an ID you can
    /// use to unsubscribe later.
    pub fn subscribe(&mut self) -> MsgReceiver { 
        let resl = Arc::new( MsgQueueThreaded {
            is_empty: AtomicBool::new(true),
            queue: Mutex::new(Vec::new()),
        });
        self.subscribers.push(resl.clone());

        return MsgReceiver{receiver: resl, our_queue: VecDeque::new()};
    }
    /// If a MsgReceiver has been dropped on the other end, clean up our reference to it.
    #[allow(unused_must_use)]
    pub fn garbage_collect(&mut self) {
        //Filter out anything where we hold the only reference.
        self.subscribers.drain_filter(|rec| Arc::strong_count(&rec) <= 1).collect::<Vec<_>>();
    }
    
    /// Take received events in, multicast to consumers.
    #[allow(dead_code)]
    pub fn process(&self) {
        //Broadcast events
        for ev in self.our_receiver.try_iter() {
            self.broadcast(ev);
        }
    }
    /// Broadcasts an event to all subscribers - used inside of process
    #[allow(dead_code)]
    pub fn broadcast(&self, message: Message) {
        //Broadcast event
        for subscriber in self.subscribers.iter() {
            subscriber.send(message.clone());
        }
    }
}

custom_error!{MessageError
    MissingChannel{channel: ChannelId} = "Attempted to access channel {channel}, which does not exist.",
    CreateChannelAlreadyExists{channel: ChannelId} = "Attempted to create channel {channel}, which exists already.",
    MessageCastFailure{target: MsgTypeId, src: MsgTypeId} = "Attempted to downcast a {src} message into {target}."
}

pub struct MessageSystem {
    //namespaces: UstrSet,
    //A list of every channel ID put into a particular namespace, by the namespace name.
    //ns_channels: UstrMap<Vec<UstrSet>>,
    channels: UstrMap<EventBus>,
}

impl MessageSystem {
    pub fn process(&self) {
        for (_,chan) in self.channels.iter() {
            chan.process();
        }
    }
    pub fn send(&self, chan: ChannelId, message: Message) -> Result<(), Box<dyn Error>> {
        Ok(self.channels.get(&chan).ok_or(
            Box::new(MessageError::MissingChannel{channel: chan.clone()})
        )?.broadcast(message)
        )
    }
    pub fn sender_for(&mut self, chan: ChannelId) -> Result<MsgSender, Box<dyn Error>> {
        Ok(self.channels.get_mut(&chan).ok_or(
            Box::new(MessageError::MissingChannel{channel: chan.clone()})
        )?.get_sender()
        )
    }
    pub fn subscribe_to(&mut self, chan: ChannelId) -> Result<MsgReceiver, Box<dyn Error>> {
        Ok(self.channels.get_mut(&chan).ok_or(
            Box::new(MessageError::MissingChannel{channel: chan.clone()})
        )?.subscribe()
        )
    }
    pub fn make_channel(&mut self, chan: &ChannelId) -> Result<(), Box<dyn Error>> {
        if self.channels.contains_key(chan) {
            Err(Box::new(MessageError::CreateChannelAlreadyExists{channel: chan.clone()}))
        }
        else {
            let (s,r) = unbounded();
            self.channels.insert(chan.clone(), EventBus { 
                our_receiver : r,
                sender_template : s,
                subscribers : Vec::new(),
            });
            Ok(())
        }
    }
    //No function to delete a channel. Channels should stick around.
}


lazy_static! {
    pub static ref MESSAGE_SYSTEM: Mutex<MessageSystem> = Mutex::new(MessageSystem{channels: UstrMap::default()} );
    pub static ref MESSANGER_THREAD : std::thread::JoinHandle<()> = thread::spawn(move || {
        loop {
            MESSAGE_SYSTEM.lock().process();
        }
    });
}

pub fn send_message(chan: ChannelId, msg: Message) -> Result<(), Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().send(chan, msg)
}
pub fn channel_sender(chan: ChannelId) -> Result<MsgSender, Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().sender_for(chan)
}
pub fn subscribe_channel(chan: ChannelId) -> Result<MsgReceiver, Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().subscribe_to(chan)
}
pub fn make_channel(chan: ChannelId) -> Result<(), Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().make_channel(&chan)
}

#[test]
fn test_make_channel() {
    make_channel(ustr("chan_test_1")).unwrap();
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TestMessage(String);

impl RegisteredMessage for TestMessage {
    fn msg_ty() -> MsgTypeId { ustr("test_string") }
    fn unpack(msg: &MsgData) -> Result<Self, Box<dyn Error>>{
        Ok( TestMessage{ 0: bincode::deserialize_from(msg.as_slice())? } )
    }
    fn construct_message(&self) -> Result<Message, Box<dyn Error>> {
        Ok(Message{ty: Self::msg_ty(), data: bincode::serialize(&self.0)?})
    }

    fn unpack_from(msg: Message) -> Result<Self, Box<dyn Error>> {
        if msg.ty != Self::msg_ty() {
            Err(Box::new(MessageError::MessageCastFailure{target: Self::msg_ty(), src: msg.ty.clone()}))
        }
        else {
            Ok(Self::unpack(&msg.data)?)
        }
    }
}

#[allow(unused_must_use)]
#[test]
fn test_send_message() {
    let channel = ustr("chan_test_2");
    make_channel(channel);

    let mut receiver = subscribe_channel(channel).unwrap();
    
    let msg1 = TestMessage{ 0: String::from("msg_test")};

    send_message(channel, msg1.construct_message().unwrap()).unwrap();

    //thread::sleep(std::time::Duration::from_millis(100));
    let mut count = 0;
    while let Some(msg) = receiver.poll() {
        count += 1;
        println!("{}", msg.ty);
        if msg.ty != ustr("test_string") {panic!()}
    }
    assert_eq!(count, 1);
    for i in 0..10 {
        let msg = TestMessage{ 0: format!("test.{}",i) };

        send_message(channel, msg.construct_message().unwrap()).unwrap();
    }
    while let Some(msg) = receiver.poll() {
        count += 1;
        println!("{}", msg.ty);
        println!("{}", bincode::deserialize::<String>(msg.data.as_slice()).unwrap());
    }
    assert_eq!(count, 11);
}