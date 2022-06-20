use std::collections::VecDeque;
use std::error::Error;
use std::fmt::Debug;
use std::result::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crossbeam_channel::*;
use hashbrown::{HashMap, HashSet};
use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::script::ModuleId;
use crate::world::WorldId;

/// Runtime type identifier for a type of message.
pub type MsgTypeId = Uuid;
pub type MsgData = Vec<u8>;

pub type ChannelId = Uuid;

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

    fn unpack_from(msg: Message) -> Result<Self, Box<dyn Error>> {
        if msg.ty != Self::msg_ty() {
            Err(Box::new(MessageError::MessageCastFailure(
                Self::msg_ty(),
                msg.ty,
            )))
        } else {
            Ok(Self::unpack(&msg.data)?)
        }
    }
}

//MsgSender gets to be pretty lightweight. MsgReceiver wishes it could be this lucky.
///Thin wrapper over a crossbeam::Sender<Message>.
#[derive(Clone)]
pub struct MsgSender(Sender<Message>);
impl MsgSender {
    #[inline(always)]
    pub fn send<T: RegisteredMessage>(&self, to_send: T) -> Result<(), Box<dyn Error>> {
        self.send_raw(to_send.construct_message()?)
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
    Multi(HashSet<MsgTypeId>),
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
            None
        } else if let Some(mut guard) = self.queue.try_lock() {
            self.is_empty.store(true, Ordering::Release);
            Some(guard.drain(..).collect())
        } else {
            None
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
        } else {
            self.poll_inner();
            let mut res: Option<Message> = None;
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
        self.poll_filtered(&MsgTypeFilter::Single(T::msg_ty()))
            .and_then(|m| T::unpack(&m.data).ok())
    }
}

pub struct TypedMsgReceiver<T: RegisteredMessage> {
    receiver: MsgReceiverInternal,
    /// We maintain our own (cached?) queue so that we don't have the overhead of locking a mutex on every invocation of poll().
    pub our_queue: VecDeque<T>,
}
impl<T> TypedMsgReceiver<T>
where
    T: RegisteredMessage,
{
    #[inline(always)]
    fn poll_inner(&mut self) {
        if let Some(ref mut rece) = self.receiver.poll() {
            //TODO: More graceful error handling here.
            self.our_queue.append(
                &mut rece
                    .iter_mut()
                    .map(|m| T::unpack(&m.data).unwrap())
                    .collect(),
            );
        }
    }
    #[inline(always)]
    pub fn poll(&mut self) -> Option<T> {
        self.poll_inner();
        self.our_queue.pop_front()
    }
    pub fn accepted_type() -> MsgTypeId {
        T::msg_ty()
    }
}

/// An event bus that multicasts incoming events out to all consumers.
pub struct MessageBus {
    /// This is where events sent to the bus / journal will go.
    our_receiver: Receiver<Message>,
    /// Used to clone repeatedly for senders to this bus
    sender_template: Sender<Message>,
    /// A list of registered subscribers. Each receiving queue is owned by the consumer.
    subscribers: Vec<MsgReceiverInternal>,
    /// A map of MsgTypeIDs -> Single-type subscribers.
    typed_subscribers: HashMap<MsgTypeId, Vec<MsgReceiverInternal>>,
}

impl MessageBus {
    #[allow(dead_code)]
    pub fn new() -> MessageBus {
        let (s_in, r_in) = unbounded();
        MessageBus {
            our_receiver: r_in,
            sender_template: s_in,
            subscribers: Vec::new(),
            typed_subscribers: HashMap::default(),
        }
    }

    /// Gives you a Crossbeam Sender to push events to this bus.
    #[allow(unused_mut)]
    pub fn get_sender(&mut self) -> MsgSender {
        MsgSender(self.sender_template.clone())
    }

    /// Gives you a receiver you can use to poll events from this bus, and an ID you can
    /// use to unsubscribe later.
    pub fn subscribe(&mut self) -> MsgReceiver {
        let resl = Arc::new(MsgQueueThreaded {
            is_empty: AtomicBool::new(true),
            queue: Mutex::new(Vec::new()),
        });
        self.subscribers.push(resl.clone());

        MsgReceiver {
            receiver: resl,
            our_queue: VecDeque::new(),
        }
    }
    /// Gives you a recevier that only accepts a single type of message.
    pub fn subscribe_typed<T: RegisteredMessage>(&mut self) -> TypedMsgReceiver<T> {
        let resl = Arc::new(MsgQueueThreaded {
            is_empty: AtomicBool::new(true),
            queue: Mutex::new(Vec::new()),
        });
        match self.typed_subscribers.get_mut(&T::msg_ty()) {
            Some(list) => {
                list.push(resl.clone());
            }
            None => {
                self.typed_subscribers
                    .insert(T::msg_ty(), vec![resl.clone()]);
            }
        }

        TypedMsgReceiver {
            receiver: resl,
            our_queue: VecDeque::new(),
        }
    }

    /// If a MsgReceiver has been dropped on the other end, clean up our reference to it.
    #[allow(unused_must_use)]
    pub fn garbage_collect(&mut self) {
        //Filter out anything where we hold the only reference.
        self.subscribers
            .drain_filter(|rec| Arc::strong_count(rec) <= 1)
            .collect::<Vec<_>>();
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
        if let Some(typed_subscriber_list) = self.typed_subscribers.get(&message.ty) {
            for sub in typed_subscriber_list {
                sub.send(message.clone());
            }
        }
    }
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum MessageError {
    #[error("Attempted to access channel: {0:?}, which does not exist")]
    MissingChannel(ChannelId),
    #[error("Attempted to create channel {0}, which exists already.")]
    CreateChannelAlreadyExists(String),
    #[error("Attempted to downcast a {0:?} message into {0:?}.")]
    MessageCastFailure(MsgTypeId, MsgTypeId),
    #[error("A message of type {0:?} contained invalid data.")]
    InvalidMessage(MsgTypeId),
}

pub enum ChannelDomain {
    Global,
    World(WorldId),
    //Entity(WorldId, EntityId),
    Module(ModuleId),
    WorldModule(WorldId, ModuleId),
}

/*
pub enum ChannelDomain {
    Global,
    World(crate::world::WorldId),
}

pub struct MessageSystem {
    global_channels: UstrMap<MessageBus>,
    per_world_channels: HashMap<crate::world::WorldId, UstrMap<MessageBus>>,
}

impl MessageSystem {
    pub fn process(&self) {
        for (_,chan) in self.global_channels.iter() {
            chan.process();
        }
    }
    pub fn send(&self, chan: ChannelId, context: ChannelDomain, message: Message) -> Result<(), Box<dyn Error>> {
        match context {
            ChannelDomain::Global => {
                Ok(self.global_channels.get(&chan).ok_or(
                    Box::new(MessageError::MissingChannel{channel: chan.clone()})
                )?.broadcast(message)
                )
            },
            ChannelDomain::World(id) => {
                Ok(self.per_world_channels.get(&id).ok_or(Box::new(MessageError::MissingChannel{channel: chan.clone()}))?
                    .get(&chan).ok_or(
                        Box::new(MessageError::MissingChannel{channel: chan.clone()})
                    )?.broadcast(message)
                )
            }
        }
    }
    pub fn sender_for(&mut self, chan: ChannelId, context: ChannelDomain) -> Result<MsgSender, Box<dyn Error>> {
        Ok(self.global_channels.get_mut(&chan).ok_or(
            Box::new(MessageError::MissingChannel{channel: chan.clone()})
        )?.get_sender()
        )
    }
    pub fn subscribe_to(&mut self, chan: ChannelId, context: ChannelDomain) -> Result<MsgReceiver, Box<dyn Error>> {
        Ok(self.global_channels.get_mut(&chan).ok_or(
            Box::new(MessageError::MissingChannel{channel: chan.clone()})
        )?.subscribe()
        )
    }
    pub fn make_channel(&mut self, chan: &ChannelId, context: ChannelDomain) -> Result<(), Box<dyn Error>> {
        if self.global_channels.contains_key(chan) {
            Err(Box::new(MessageError::CreateChannelAlreadyExists{channel: chan.clone()}))
        }
        else {
            let (s,r) = unbounded();
            self.global_channels.insert(chan.clone(), MessageBus {
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
            if let Some(system) = MESSAGE_SYSTEM.try_lock() {
                system.process()
            }
        }
    });
}

pub fn send_message(chan: ChannelId, context: ChannelDomain, msg: Message) -> Result<(), Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().send(chan, context, msg)
}
pub fn channel_sender(chan: ChannelId, context: ChannelDomain) -> Result<MsgSender, Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().sender_for(chan, context)
}
pub fn subscribe_channel(chan: ChannelId, context: ChannelDomain) -> Result<MsgReceiver, Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().subscribe_to(chan, context)
}
pub fn make_channel(chan: ChannelId, context: ChannelDomain) -> Result<(), Box<dyn Error>> {
    MESSAGE_SYSTEM.lock().make_channel(&chan, context)
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
*/
#[test]
fn test_make_channel() {
    let (s, r) = unbounded();
    let _channel = MessageBus {
        our_receiver: r,
        sender_template: s,
        subscribers: Vec::new(),
        typed_subscribers: HashMap::default(),
    };
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TestMessage(String);
lazy_static::lazy_static! {
    pub static ref TEST_UUID_1 : Uuid = Uuid::new_v4();
    pub static ref TEST_UUID_2 : Uuid = Uuid::new_v4();
}

impl RegisteredMessage for TestMessage {
    fn msg_ty() -> MsgTypeId {
        *TEST_UUID_1
    }
    fn unpack(msg: &MsgData) -> Result<Self, Box<dyn Error>> {
        Ok(TestMessage(rmp_serde::from_read_ref(msg.as_slice())?))
    }
    fn construct_message(&self) -> Result<Message, Box<dyn Error>> {
        Ok(Message {
            ty: Self::msg_ty(),
            data: rmp_serde::to_vec_named(&self.0)?,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TestMessage2(String);

impl RegisteredMessage for TestMessage2 {
    fn msg_ty() -> MsgTypeId {
        *TEST_UUID_2
    }
    fn unpack(msg: &MsgData) -> Result<Self, Box<dyn Error>> {
        Ok(TestMessage2(rmp_serde::from_read_ref(msg.as_slice())?))
    }
    fn construct_message(&self) -> Result<Message, Box<dyn Error>> {
        Ok(Message {
            ty: Self::msg_ty(),
            data: rmp_serde::to_vec_named(&self.0)?,
        })
    }

    fn unpack_from(msg: Message) -> Result<Self, Box<dyn Error>> {
        if msg.ty != Self::msg_ty() {
            Err(Box::new(MessageError::MessageCastFailure(
                Self::msg_ty(),
                msg.ty,
            )))
        } else {
            Ok(Self::unpack(&msg.data)?)
        }
    }
}

#[allow(unused_must_use)]
#[test]
fn test_send_message() {
    let mut channel = MessageBus::new();

    let mut receiver = channel.subscribe();

    let msg1 = TestMessage(String::from("msg_test"));

    channel.broadcast(msg1.construct_message().unwrap());

    //thread::sleep(std::time::Duration::from_millis(100));
    let mut count = 0;
    while let Some(msg) = receiver.poll() {
        count += 1;
        println!("{}", msg.ty);
        if msg.ty != *TEST_UUID_1 {
            panic!()
        }
    }
    assert_eq!(count, 1);
    for i in 0..10 {
        let msg = TestMessage (
            format!("test.{}", i)
        );

        channel.broadcast(msg.construct_message().unwrap());
    }
    while let Some(msg) = receiver.poll() {
        count += 1;
        println!("{}", msg.ty);
        println!(
            "{}",
            rmp_serde::from_read_ref::<[u8], String>(msg.data.as_slice()).unwrap()
        );
    }
    assert_eq!(count, 11);
}

#[allow(unused_must_use)]
#[test]
fn test_send_message_typed() {
    let mut channel = MessageBus::new();

    let mut receiver: TypedMsgReceiver<TestMessage> = channel.subscribe_typed();

    let msg1 = TestMessage (
        String::from("msg test"),
    );
    let msg2 = TestMessage2 (
        String::from("second test"),
    );

    channel.broadcast(msg1.construct_message().unwrap());
    channel.broadcast(msg2.construct_message().unwrap());

    //This is a typed receiver that only gets "TestMessage" type messages.
    //Even if we send a TestMessage2, this receiver should not get it.
    let mut count = 0;
    while let Some(msg) = receiver.poll() {
        count += 1;
        assert_eq!(msg.0, msg1.0);
    }
    assert_eq!(count, 1);

    for i in 0..10 {
        let msg = TestMessage (
            format!("test.{}", i)
        );

        channel.broadcast(msg.construct_message().unwrap());
        channel.broadcast(msg2.construct_message().unwrap());
    }
    while receiver.poll().is_some() {
        count += 1;
    }
    assert_eq!(count, 11);
}
