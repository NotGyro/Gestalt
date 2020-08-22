use std::sync::{Arc, Mutex};
use std::result::Result;
use std::error::Error;
use std::fmt::Debug;

use ustr::*;
use crossbeam_channel::*;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;
use custom_error::custom_error;
use std::collections::VecDeque;

/// Runtime type identifier for a type of message.
pub type MsgTypeId = Ustr;
pub type MsgData = Vec<u8>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Message {
    //Message type
    pub msg_ty: MsgTypeId,
    //Message data
    pub data: MsgData,
}

pub trait MsgBounds: Clone + Debug + Serialize + DeserializeOwned {}

pub trait RegisteredMessage: MsgBounds {
    fn msg_ty() -> MsgTypeId;
    fn unpack(msg: MsgData) -> Result<Self, Box<dyn Error>>;
    fn construct_message(&self) -> Result<Message, Box<dyn Error>>;
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


pub struct MsgReceiver {
    receiver: Receiver<Message>,
    // "id" and "dropchannel" for cleanup purposes.
    dropper: Option<(usize, Sender<usize>)>,
}
impl MsgReceiver {
    #[inline(always)]
    fn poll(&mut self) -> Option<Message> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for MsgReceiver {
    fn drop(&mut self) {
        //Clean up if we need to clean up
        if let Some((id, dropchannel)) = &self.dropper {
            dropchannel.send(*id).unwrap();
        }
    }
}

pub struct MsgReceiverFilterable {
    inner: MsgReceiver,
    queue: VecDeque<Message>,
}

impl MsgReceiverFilterable {
    //Gets any type of message.
    #[inline]
    pub fn poll_any(&mut self) -> Option<Message> { 
        self.poll_inner();
        self.queue.pop_front()
    }
    #[inline]
    pub fn poll(&mut self, filter: &MsgTypeFilter) -> Option<Message> { 
        if *filter == MsgTypeFilter::Any { 
            self.poll_any()
        }
        else {
            self.poll_inner();
            let mut iter = 0;
            while iter < self.queue.len() {
                if self.poll_find_step(iter, filter) {
                    return self.queue.remove(iter);
                }
                iter = iter + 1;
            }
            None
        }
    }
    #[inline(always)]
    pub fn poll_to<T: RegisteredMessage>(&mut self) -> Option<T> {
        self.poll(&MsgTypeFilter::Single(T::msg_ty())).map(|m| T::unpack(m.data).ok() ).flatten()
    }
    #[inline(always)]
    fn poll_find_step(&mut self, iter: usize, filter: &MsgTypeFilter) -> bool {
        filter.suitable(&self.queue[iter].msg_ty)
    }
    #[inline(always)]
    fn poll_inner(&mut self) {
        while let Some(msg) = self.inner.poll() {
            self.queue.push_back(msg)
        }
    }
}


impl From<MsgReceiver> for MsgReceiverFilterable {
    fn from(rec: MsgReceiver) -> Self {
        MsgReceiverFilterable{inner: rec, queue : VecDeque::new()}
    }
}
