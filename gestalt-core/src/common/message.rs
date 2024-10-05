use std::any::Any;
use std::collections::hash_map::Entry;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use futures::{Future, TryFutureExt};
use log::{error, info, trace};
use tokio::sync::broadcast::error::TryRecvError as BroadcastTryRecvError;
use tokio::sync::mpsc::error::TryRecvError as MpscTryRecvError;
use tokio::sync::{broadcast, mpsc};

use crate::world::WorldId;

use super::identity::NodeIdentity;
use super::{new_fast_hash_map, FastHashMap};

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum RecvError {
	#[error("Failed to send a message onto a message channel, because there are no remaining receivers associated with this sender.")]
	NoSenders,
	#[error("A channel hit its maximum number of stored messages and this channel was keeping alive old messages. {0} messages have been skipped and can no longer be retrieved.")]
	Lagged(u64),
	#[error("Implementation-specific channel error: {0}.")]
	Other(String),
}

pub trait Message: Debug {}
impl<T> Message for T where T: Debug {}

pub type BroadcastSender<T> = tokio::sync::broadcast::Sender<T>;
type UnderlyingBroadcastReceiver<T> = tokio::sync::broadcast::Receiver<T>;

#[derive(thiserror::Error, Debug, Clone)]
pub enum SendError {
	#[error("Failed to send a message onto a message channel, because there are no remaining receivers associated with this sender.")]
	NoReceivers,
	#[error("Could not send on a channel because domain {0} is not registered yet")]
	MissingDomain(String),
	#[error("Unable to encode a message so it could be sent on channel: {0}.")]
	Encode(String),
	#[error("Failed to send on a channel, because that channel's buffer is full of messages.")]
	Full,
	#[error("Implementation-specific channel error: {0}.")]
	Other(String),
}

impl<T> From<tokio::sync::broadcast::error::SendError<T>> for SendError {
	fn from(_value: tokio::sync::broadcast::error::SendError<T>) -> Self {
		SendError::NoReceivers
	}
}
impl<T> From<tokio::sync::mpsc::error::TrySendError<T>> for SendError {
	fn from(value: tokio::sync::mpsc::error::TrySendError<T>) -> Self {
		match value {
			mpsc::error::TrySendError::Full(_val) => SendError::Full,
			mpsc::error::TrySendError::Closed(_val) => SendError::NoReceivers,
		}
	}
}

pub trait MessageReceiver<T>
where
	T: Message,
{
	/// Nonblockingly polls for new messages, returning an empty vector if the channel is empty.  
	fn recv_poll(&mut self) -> Result<Option<T>, RecvError>;
}
pub trait MessageReceiverAsync<T>: MessageReceiver<T>
where
	T: Message,
{
	fn recv_wait(&mut self) -> impl Future<Output = Result<T, RecvError>> + '_;
}

pub struct BroadcastReceiver<T>
where
	T: Message + Clone,
{
	pub(in crate::common::message) inner: UnderlyingBroadcastReceiver<T>,
}

impl<T> BroadcastReceiver<T>
where
	T: Message + Clone,
{
	pub fn new(to_wrap: tokio::sync::broadcast::Receiver<T>) -> Self {
		BroadcastReceiver { inner: to_wrap }
	}

	pub fn resubscribe(&self) -> Self {
		BroadcastReceiver {
			inner: self.inner.resubscribe(),
		}
	}

	async fn recv_wait_inner(&mut self) -> Result<T, RecvError> {
		self.inner
			.recv()
			.map_err(|e| match e {
				broadcast::error::RecvError::Closed => RecvError::NoSenders,
				broadcast::error::RecvError::Lagged(count) => RecvError::Lagged(count),
			})
			.await
	}
}

impl<T> MessageReceiver<T> for BroadcastReceiver<T>
where
	T: Message + Clone,
{
	/// Nonblockingly polls for new messages, returning an empty vector if the channel is empty.  
	fn recv_poll(&mut self) -> Result<Option<T>, RecvError> {
		match self.inner.try_recv() {
			Ok(val) => Ok(Some(val)),
			Err(err) => match err {
				BroadcastTryRecvError::Empty => Ok(None),
				BroadcastTryRecvError::Closed => Err(RecvError::NoSenders),
				BroadcastTryRecvError::Lagged(count) => Err(RecvError::Lagged(count)),
			},
		}
	}
}

impl<T> MessageReceiverAsync<T> for BroadcastReceiver<T>
where
	T: Message + Clone,
{
	/// Receives new messages batch, waiting for a message if the channel is currently empty.
	fn recv_wait(&mut self) -> impl Future<Output = Result<T, RecvError>> + '_ {
		self.recv_wait_inner()
	}
}

pub trait ChannelDomain: Send + Clone + PartialEq + Eq + PartialOrd + Hash + Debug + Any {}
impl<A, B> ChannelDomain for (A, B)
where
	A: ChannelDomain,
	B: ChannelDomain,
{
}

impl ChannelDomain for WorldId {}
impl ChannelDomain for NodeIdentity {}

pub trait MessageWithDomain<D>: Message
where
	D: ChannelDomain,
{
	fn get_domain(&self) -> &D;
}

impl<T, D> MessageWithDomain<D> for (T, D)
where
	T: Message,
	D: ChannelDomain,
{
	fn get_domain(&self) -> &D {
		&self.1
	}
}

pub trait MessageSender<T>
where
	T: Message,
{
	/// Send a batch of messages. If the underlying
	fn send(&self, message: T) -> Result<(), SendError>;
}

pub trait DomainMessageSender<T, D>
where
	T: Message,
	D: ChannelDomain,
{
	/// Send one message to one domain
	fn send_to(&self, message: T, domain: &D) -> Result<(), SendError>;

	/// Send one message to every domain
	fn send_to_all(&self, message: T) -> Result<(), SendError>;

	/// Send one message to every domain, excluding the domain 'exclude'
	fn send_to_all_except(&self, message: T, exclude: &D) -> Result<(), SendError>;
}

impl<T> MessageSender<T> for BroadcastSender<T>
where
	T: Message,
{
	fn send(&self, message: T) -> Result<(), SendError> {
		self.send(message).map(|_| ()).map_err(|e| e.into())
	}
}

/// Used to disambiguate from situations where std::sync::Mutex<T> or tokio::sync::Mutex<T> are also being used.
type ChannelMutex<T> = parking_lot::Mutex<T>;

/// Trait that lets you get a sender to send into a message-passing channel.
/// This is separate from ReceiverChannel because some types
/// of channels, for example any mpsc channel, might let you make
/// many senders but there would be only one receiver
/// (so you can't subscribe additional receivers into existence).
pub trait SenderChannel<T> 
where
	T: Message,
{ 
	type Sender: MessageSender<T>;
}

pub trait SenderSubscribe<T> : SenderChannel<T>
where
	T: Message,
{
	// The trait does not include the Receiver because an
	// mpsc channel will only have one consumer - so, the
	// receiver is not something we can subscribe to.

	fn sender_subscribe(&self) -> Self::Sender;
}

pub trait ReceiverChannel<T>
where
	T: Message,
{
	type Receiver: MessageReceiver<T>;
}
/// Trait that lets you get a receiver to receive from a message-passing channel.
/// This is separate from SenderChannel because some types
/// of channels, for example any mpsc channel, might let you make
/// many senders but there would be only one receiver
/// (so you can't subscribe additional receivers into existence).
pub trait ReceiverSubscribe<T> : ReceiverChannel<T>
where
	T: Message,
{
	fn receiver_subscribe(&self) -> Self::Receiver;
}
/// Trait that lets you take the one and only channel permitted to a channel.
/// Be careful with this one.
pub trait TakeReceiver<T> : ReceiverChannel<T>
where
	T: Message,
{
	fn take_receiver(&self) -> Result<Self::Receiver, DomainSubscribeErr<String>>;
}

pub trait MpmcChannel<T: Message>: SenderSubscribe<T> + ReceiverChannel<T> {}
impl<T, U> MpmcChannel<T> for U
where
	T: Message,
	U: SenderSubscribe<T> + ReceiverChannel<T>,
{
}

pub trait ChannelInit: Sized {
	fn new(capacity: usize) -> Self;
}

/// Any channel we can retrieve the number of CURRENTLY ACTIVE
/// receivers for.
pub trait ReceiverCount {
	fn receiver_count(&self) -> usize;
}

pub struct BroadcastChannel<T>
where
	T: Message,
{
	// Does not need a mutex because you can clone it without mut.
	sender: BroadcastSender<T>,

	/// It's a bad idea to just have a copy of a broadcast::Receiver around forever,
	/// because then the channel will be perpetually full even when it doesn't need to be.
	/// So, we initialize with one, and it immediately gets taken by the first to try to subscribe.
	///
	/// The reason we need to hold onto one reference is so that
	/// attempts to send before anyone has grabbed a receiver do not
	/// instantly fail.
	retained_receiver: Arc<ChannelMutex<Option<UnderlyingBroadcastReceiver<T>>>>,
}
impl<T> BroadcastChannel<T>
where
	T: Message + Clone,
{
	/// Construct a new channel.
	/// The argument is the channel's capacity - how long of a backlog can this channel hold?
	pub fn new(capacity: usize) -> Self {
		let (sender, receiver) = tokio::sync::broadcast::channel(capacity);
		BroadcastChannel {
			sender,
			retained_receiver: Arc::new(ChannelMutex::new(Some(receiver))),
		}
	}
}

// Implementing Clone in the Arc<T> sense here, so Clone is just creating another reference to the same
// underlying synchronized structure.
impl<T> Clone for BroadcastChannel<T>
where
	T: Message,
{
	fn clone(&self) -> Self {
		Self {
			sender: self.sender.clone(),
			retained_receiver: self.retained_receiver.clone(),
		}
	}
}

impl<T> ReceiverCount for BroadcastChannel<T>
where
	T: Message,
{
	fn receiver_count(&self) -> usize {
		let lock = self.retained_receiver.lock();
		let has_retained = lock.is_some();
		drop(lock);

		if has_retained {
			self.sender.receiver_count() - 1
		} else {
			self.sender.receiver_count()
		}
	}
}

impl<T> SenderChannel<T> for BroadcastChannel<T> where T: Message { 
	type Sender = BroadcastSender<T>;
}

impl<T> SenderSubscribe<T> for BroadcastChannel<T>
where
	T: Message + Clone,
{
	fn sender_subscribe(&self) -> BroadcastSender<T> {
		self.sender.clone()
	}
}
impl<T> ReceiverChannel<T> for BroadcastChannel<T> where T: Message + Clone,
{
	type Receiver = BroadcastReceiver<T>;
}
impl<T> ReceiverSubscribe<T> for BroadcastChannel<T>
where
	T: Message + Clone,
{
	fn receiver_subscribe(&self) -> BroadcastReceiver<T> {
		let mut lock = self.retained_receiver.lock();
		let mut retained_maybe = lock.take();
		drop(lock);

		BroadcastReceiver::new(if retained_maybe.is_some() {
			retained_maybe.take().unwrap()
		} else {
			self.sender.subscribe()
		})
	}
}

//Note that sending directly on a channel rather than subscribing a sender will always be slower than getting a sender for bulk operations.
impl<T, R> MessageSender<T> for BroadcastChannel<R>
where
	T: Into<R> + Message,
	R: Message + Clone,
{
	fn send(&self, message: T) -> Result<(), SendError> {
		self.sender
			.send(message.into())
			.map_err(|_e| SendError::NoReceivers)
			.map(|_val| ())
	}
}

impl<T> ChannelInit for BroadcastChannel<T>
where
	T: Message + Clone,
{
	fn new(capacity: usize) -> Self {
		BroadcastChannel::new(capacity)
	}
}

pub type MpscSenderInner<T> = tokio::sync::mpsc::Sender<T>;
type UnderlyingMpscReceiver<T> = mpsc::Receiver<T>;

pub struct MpscReceiver<T>
where
	T: Message,
{
	pub(in crate::common::message) inner: UnderlyingMpscReceiver<T>,
}

impl<T> MpscReceiver<T>
where
	T: Message,
{
	pub fn new(to_wrap: tokio::sync::mpsc::Receiver<T>) -> Self {
		MpscReceiver { inner: to_wrap }
	}

	async fn recv_wait_inner(&mut self) -> Result<T, RecvError> {
		self.inner.recv().await.ok_or(RecvError::NoSenders)
	}
}

impl<T> MessageReceiver<T> for MpscReceiver<T>
where
	T: Message,
{
	/// Nonblockingly polls for new messages, returning an empty vector if the channel is empty.  
	fn recv_poll(&mut self) -> Result<Option<T>, RecvError> {
		match self.inner.try_recv() {
			Ok(val) => Ok(Some(val)),
			Err(e) => match e {
				MpscTryRecvError::Empty => Ok(None),
				MpscTryRecvError::Disconnected => Err(RecvError::NoSenders),
			},
		}
	}
}

impl<T> MessageReceiverAsync<T> for MpscReceiver<T>
where
	T: Message,
{
	/// Receives new messages batch, waiting for a message if the channel is currently empty.
	fn recv_wait(&mut self) -> impl Future<Output = Result<T, RecvError>> + '_ {
		self.recv_wait_inner()
	}
}
pub struct MpscSender<T> where T: Message {
	pub(super) inner: MpscSenderInner<T>,
}
impl<T> Clone for MpscSender<T> where T: Message {
	fn clone(&self) -> Self {
		Self { inner: self.inner.clone() }
	}
}

impl<T> MessageSender<T> for MpscSender<T>
where
	T: Message,
{
	fn send(&self, message: T) -> Result<(), SendError> {
		self.inner.try_send(message).map(|_| ()).map_err(|e| e.into())
	}
}
// TODO AFTER SHOPPING TRIP: THIS !! 
impl<T> SenderChannel<T> for MpscSender<T> where T: Message { 
	type Sender = MpscSender<T>;
}
impl<T> SenderSubscribe<T> for MpscSender<T> where T: Message { 
	fn sender_subscribe(&self) -> Self::Sender {
		Self {
			inner: self.inner.clone(),
		}
	}
}

pub struct MpscChannel<T>
where
	T: Message,
{
	// Does not need a mutex because you can clone it without mut.
	sender: MpscSender<T>,

	/// This will be taken once and only once.
	retained_receiver: Arc<ChannelMutex<Option<UnderlyingMpscReceiver<T>>>>,
}
impl<T> MpscChannel<T>
where
	T: Message,
{
	/// Construct a new channel.
	/// The argument is the channel's capacity - how long of a backlog can this channel hold?
	pub fn new(capacity: usize) -> Self {
		let (sender, receiver) = tokio::sync::mpsc::channel(capacity);
		MpscChannel {
			sender: MpscSender { inner: sender },
			retained_receiver: Arc::new(ChannelMutex::new(Some(receiver))),
		}
	}

	/// Attempt to take the single consumer in this multi-producer single-consumer message channel.
	pub fn take_receiver(&self) -> Option<MpscReceiver<T>> {
		let mut inner_receiver = self.retained_receiver.lock();
		inner_receiver.take().map(|r| MpscReceiver::new(r))
	}
}

// Implementing Clone in the Arc<T> sense here, so Clone is just creating another reference to the same
// underlying synchronized structure.
impl<T> Clone for MpscChannel<T>
where
	T: Message,
{
	fn clone(&self) -> Self {
		Self {
			sender: self.sender.clone(),
			retained_receiver: self.retained_receiver.clone(),
		}
	}
}

impl<T> ReceiverCount for MpscChannel<T>
where
	T: Message,
{
	fn receiver_count(&self) -> usize {
		let lock = self.retained_receiver.lock();
		let has_retained = lock.is_some();
		drop(lock);

		if has_retained || self.sender.inner.is_closed() {
			0
		} else {
			1
		}
	}
}

impl<T> SenderChannel<T> for MpscChannel<T>
where
	T: Message, {
	type Sender = MpscSender<T>;
}
impl<T> SenderSubscribe<T> for MpscChannel<T>
where
	T: Message,
{
	fn sender_subscribe(&self) -> MpscSender<T> {
		self.sender.clone()
	}
}
// This has been decoupled from subscribing, and so we can impl this here. 
impl<T> ReceiverChannel<T> for MpscChannel<T> where T: Message {
	type Receiver = MpscReceiver<T>;
}

impl<T> TakeReceiver<T> for MpscChannel<T> where T: Message {
	fn take_receiver(&self) -> Result<Self::Receiver, DomainSubscribeErr<String>> {
		MpscChannel::take_receiver(self).ok_or(DomainSubscribeErr::TakeTakenReceiver(String::from("<NoDomain>")))
	}
}

//Note that sending directly on a channel rather than subscribing a sender will always be slower than getting a sender for bulk operations.
impl<T, R> MessageSender<T> for MpscChannel<R>
where
	T: Into<R> + Message,
	R: Message,
{
	fn send(&self, message: T) -> Result<(), SendError> {
		self.sender.send(message.into())
	}
}

impl<T> ChannelInit for MpscChannel<T>
where
	T: Message,
{
	fn new(capacity: usize) -> Self {
		MpscChannel::new(capacity)
	}
}

impl<T> From<MpscChannel<T>> for MpscSender<T> where T: Message {
	fn from(value: MpscChannel<T>) -> Self {
		value.sender_subscribe()
	}
}
impl<T> From<BroadcastChannel<T>> for BroadcastSender<T> where T: Message + Clone {
	fn from(value: BroadcastChannel<T>) -> Self {
		value.sender_subscribe()
	}
}
impl<T> From<BroadcastChannel<T>> for BroadcastReceiver<T> where T: Message + Clone {
	fn from(value: BroadcastChannel<T>) -> Self {
		value.receiver_subscribe()
	}
}

impl<T> SenderChannel<T> for BroadcastSender<T> where T: Message + Clone {
	type Sender = <BroadcastChannel<T> as SenderChannel<T>>::Sender;
}
impl<T> SenderSubscribe<T> for BroadcastSender<T> where T: Message + Clone + Debug {
	fn sender_subscribe(&self) -> Self::Sender {
		self.clone()
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum DomainSubscribeErr<D>
where
	D: Debug + Clone,
{
	#[error("Cannot subscribe to a channel in domain {0:?} because that domain has not been registered.")]
	NoDomain(D),
	#[error("Attempted to take receiver for domain {0:?} but this receiver was already taken.")]
	TakeTakenReceiver(D),
}

impl<D> DomainSubscribeErr<D> where D: Debug + Clone {
	pub fn to_string_form(self) -> DomainSubscribeErr<String> { 
		match self {
			DomainSubscribeErr::NoDomain(d) => DomainSubscribeErr::NoDomain(format!("{d:#?}")),
			DomainSubscribeErr::TakeTakenReceiver(d) => DomainSubscribeErr::TakeTakenReceiver(format!("{d:#?}")),
		}
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum NewDomainErr<D>
where
	D: Debug + Clone,
{
	#[error("Attempted to add domain {0:?} to a channel, but this domain already exists.")]
	AlreadyExists(D),
}

#[derive(Clone)]
pub struct MultiDomainSender<T, D, C> where
	T: Message,
	D: ChannelDomain, {
	channels: Arc<ChannelMutex<std::collections::HashMap<D, C>>>,
	_message_ty_phantom: PhantomData<T>,
}

impl<T,D,C> From<Arc<ChannelMutex<std::collections::HashMap<D, C>>>> for MultiDomainSender<T,D,C>  where
	T: Message,
	D: ChannelDomain, {
	fn from(value: Arc<ChannelMutex<std::collections::HashMap<D, C>>>) -> Self {
		Self { 
			channels: value, 
			_message_ty_phantom: PhantomData,
		}
	}
}

impl<T,D,C> DomainMessageSender<T, D> for MultiDomainSender<T, D, C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: ChannelInit + MessageSender<T>,
{
	fn send_to(&self, message: T, domain: &D) -> Result<(), SendError> {
		match self.channels.lock().get(domain) {
			Some(chan) => chan
				.send(message)
				.map_err(|_e| SendError::NoReceivers)
				.map(|_val| ()),
			None => Err(SendError::MissingDomain(format!("{:?}", domain))),
		}
	}

	fn send_to_all(&self, message: T) -> Result<(), SendError> {
		for chan in self.channels.lock().values() {
			chan.send(message.clone())?;
		}
		Ok(())
	}

	fn send_to_all_except(&self, message: T, exclude: &D) -> Result<(), SendError> {
		for (domain, chan) in self.channels.lock().iter() {
			if domain != exclude {
				chan.send(message.clone())?;
			}
		}
		Ok(())
	}
}
#[derive(Clone)]
pub struct DomainMultiChannel<T, D, C>
where
	T: Message,
	D: ChannelDomain,
{
	/// This is carried into any channels we will initialize
	capacity: usize,

	channels: Arc<ChannelMutex<std::collections::HashMap<D, C>>>,

	_message_ty_phantom: PhantomData<T>,
}

impl<T, D, C> DomainMultiChannel<T, D, C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: SenderSubscribe<T> + ChannelInit,
{
	pub fn sender_subscribe(&self, domain: &D) -> Result<C::Sender, DomainSubscribeErr<D>> {
		Ok(self
			.channels
			.lock()
			.get_mut(domain)
			.ok_or_else(|| DomainSubscribeErr::NoDomain(domain.clone()))?
			.sender_subscribe())
	}

	pub fn sender_subscribe_all(&self) -> MultiDomainSender<T,D, C> {
		MultiDomainSender::from(self.channels.clone())
	}

	/// Adds a new domain if it isn't there yet, takes no action if one is already present.
	pub fn init_domain(&self, domain: D) -> Result<(), NewDomainErr<D>> {
		self.add_channel(domain, C::new(self.capacity))
	}
	/// Adds a channel generated externally to this domain-multi-channel
	pub fn add_channel(&self, domain: D, channel: C) -> Result<(), NewDomainErr<D>> {
		let mut lock = self.channels.lock();
		let entry = lock.entry(domain.clone());
		if let Entry::Occupied(_v) = &entry { 
			Err(NewDomainErr::AlreadyExists(domain))
		} else {
			entry.or_insert(channel);
			Ok(())
		}
	}
	pub fn drop_domain(&self, domain: &D) {
		let lock = self.channels.lock();
		let contains = lock.contains_key(domain);
		drop(lock);
		if contains {
			self.channels.lock().remove(domain);
		}
	}

	pub fn get_capacity(&self) -> usize { 
		self.capacity
	}
}

impl<T, D, C> DomainMultiChannel<T, D, C>
where
	T: Message,
	D: ChannelDomain,
	C: ReceiverSubscribe<T>,
{
	pub fn receiver_subscribe(&self, domain: &D) -> Result<C::Receiver, DomainSubscribeErr<D>> {
		Ok(self
			.channels
			.lock()
			.get_mut(domain)
			.ok_or_else(|| DomainSubscribeErr::NoDomain(domain.clone()))?
			.receiver_subscribe())
	}
}
impl<T, D, C> DomainMultiChannel<T, D, C>
where
	T: Message,
	D: ChannelDomain,
	C: TakeReceiver<T>,
{
	pub fn take_receiver(&self, domain: &D) -> Result<C::Receiver, DomainSubscribeErr<D>> {
		self.channels
			.lock()
			.get_mut(domain)
			.ok_or_else(|| DomainSubscribeErr::NoDomain(domain.clone()))?
			.take_receiver()
			.map_err(|_e| DomainSubscribeErr::TakeTakenReceiver(domain.clone()))
	}
}

impl<T, D, C> DomainMessageSender<T, D> for DomainMultiChannel<T, D, C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: SenderSubscribe<T> + ChannelInit + MessageSender<T>,
{
	fn send_to(&self, message: T, domain: &D) -> Result<(), SendError> {
		match self.channels.lock().get(domain) {
			Some(chan) => chan
				.send(message)
				.map_err(|_e| SendError::NoReceivers)
				.map(|_val| ()),
			None => Err(SendError::MissingDomain(format!("{:?}", domain))),
		}
	}

	fn send_to_all(&self, message: T) -> Result<(), SendError> {
		for chan in self.channels.lock().values() {
			chan.send(message.clone())?;
		}
		Ok(())
	}

	fn send_to_all_except(&self, message: T, exclude: &D) -> Result<(), SendError> {
		for (domain, chan) in self.channels.lock().iter() {
			if domain != exclude {
				chan.send(message.clone())?;
			}
		}
		Ok(())
	}
}

impl<T, D, R, C> MessageSender<T> for DomainMultiChannel<R, D, C>
where
	T: Into<R> + MessageWithDomain<D>,
	D: ChannelDomain,
	R: MessageWithDomain<D> + Clone,
	C: SenderSubscribe<R> + ChannelInit + MessageSender<R>,
{
	fn send(&self, message: T) -> Result<(), SendError> {
		let message = message.into();
		let domain = message.get_domain().clone();
		self.send_to(message, &domain)
			.map_err(|_e| SendError::NoReceivers)
	}
}

impl<T,D,C> ChannelInit for DomainMultiChannel<T,D,C>
where
	T: Message,
	D: ChannelDomain
{
	/// Construct a Domain Multichannel system.
	fn new(capacity: usize) -> Self {
		DomainMultiChannel {
			capacity,
			channels: Arc::new(ChannelMutex::new(std::collections::HashMap::new())),
			_message_ty_phantom: Default::default(),
		}
	}
}

impl<T, D, C> SenderChannel<T> for DomainMultiChannel<T, D, C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: SenderChannel<T>,
{
	type Sender = C::Sender;
}

impl<T, D, C> ReceiverChannel<T> for DomainMultiChannel<T, D, C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: ReceiverChannel<T>,
{
	type Receiver = C::Receiver;
}

pub trait DomainSenderSubscribe<T, D> : SenderChannel<T>
where
	T: Message, D: ChannelDomain
{
	// The trait does not include the Receiver because an
	// mpsc channel will only have one consumer - so, the
	// receiver is not something we can subscribe to.

	fn sender_subscribe_domain(&self, domain: &D) -> Result<Self::Sender, DomainSubscribeErr<D>>;
}
pub trait DomainReceiverSubscribe<T, D> : ReceiverChannel<T>
where
	T: Message, D: ChannelDomain
{
	// The trait does not include the Receiver because an
	// mpsc channel will only have one consumer - so, the
	// receiver is not something we can subscribe to.

	fn receiver_subscribe_domain(&self, domain: &D) -> Result<Self::Receiver, DomainSubscribeErr<D>>;
}

impl<T, D, C> DomainSenderSubscribe<T, D> for DomainMultiChannel<T, D, C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: SenderSubscribe<T> + ChannelInit,
{
	fn sender_subscribe_domain(&self, domain: &D) -> Result<Self::Sender, DomainSubscribeErr<D>> {
		DomainMultiChannel::sender_subscribe(self, domain)
	}
}
impl<T, D, C> DomainReceiverSubscribe<T, D> for DomainMultiChannel<T, D, C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: ReceiverSubscribe<T> + ChannelInit,
{
	fn receiver_subscribe_domain(&self, domain: &D) -> Result<Self::Receiver, DomainSubscribeErr<D>> {
		DomainMultiChannel::receiver_subscribe(self, domain)
	}
}
pub trait DomainTakeReceiver<T, D> : ReceiverChannel<T>
where
	T: Message, D: ChannelDomain
{
	fn take_receiver_domain(&self, domain: &D) -> Result<Self::Receiver, DomainSubscribeErr<D>>;
}

impl<T, D, C> DomainTakeReceiver<T, D> for DomainMultiChannel<T,D,C>
where
	T: Message + Clone,
	D: ChannelDomain,
	C: TakeReceiver<T>,
{
	fn take_receiver_domain(&self, domain: &D) -> Result<Self::Receiver, DomainSubscribeErr<D>> {
		self.take_receiver(domain)
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum GlobalChannelError {
	#[error("Attempted to subscribe on a channel, but the channel's mutex was poisoned.")]
	MutexErr,
	#[error("Could not subscribe to a channel, separated into domains, due to an error: {0}")]
	DomainSubscribe(String),
}

macro_rules! global_channel {
	($chanty:ident, $name:ident, $message:ty, $capacity:expr) => {
		lazy_static::lazy_static! {
			pub static ref $name: $chanty<$message> = {
				$chanty::new($capacity)
			};
		}
	};
}
#[allow(unused_macros)]
macro_rules! global_domain_channel {
	($chanty:ident, $name:ident, $message:ty, $domain:ty, $capacity:expr) => {
		lazy_static::lazy_static! {
			pub static ref $name: crate::common::message::DomainMultiChannel<$message, $domain, $chanty<$message>> = {
				crate::common::message::DomainMultiChannel::new($capacity)
			};
		}
	};
}

#[derive(Clone, Debug)]
pub struct ChannelCapacityConf {
	pub chans: FastHashMap<String, usize>,
}

impl ChannelCapacityConf { 
	pub fn new() -> Self {
		ChannelCapacityConf {
			chans: new_fast_hash_map(),
		}
	}
	pub fn set<T>(&mut self, capacity: usize) where T: StaticChannelAtom {
		self.chans.insert(T::get_static_name().to_string(), capacity);
	}
	pub fn get_or_default<T>(&self) -> usize where T: StaticChannelAtom {
		*self.chans.get(T::get_static_name()).unwrap_or_else(|| &T::DEFAULT_CAPACITY)
	}
}

// A few *very universal* channels are allowed to be globals.
global_channel!(BroadcastChannel, START_QUIT, (), 1);
global_channel!(BroadcastChannel, READY_FOR_QUIT, (), 4096);

#[derive(Clone)]
#[warn(unused_must_use)]
pub struct QuitReadyNotifier {
	inner: BroadcastSender<()>,
}

impl QuitReadyNotifier {
	pub fn notify_ready(self) {
		trace!("Sending quit-ready notification.");
		let _ = self.inner.send(());
	}
}

pub struct QuitReceiver {
	inner: BroadcastReceiver<()>,
}
impl QuitReceiver {
	pub fn new() -> QuitReceiver {
		let receiver = START_QUIT.receiver_subscribe();
		QuitReceiver { inner: receiver }
	}
	/// Future does not complete until the quit process has been initiated.
	pub async fn wait_for_quit(&mut self) -> QuitReadyNotifier {
		let _ = self.inner.recv_wait().await;
		let sender = READY_FOR_QUIT.sender_subscribe();
		QuitReadyNotifier { inner: sender }
	}
}

/// Causes the engine to quit and then wait for as many READY_FOR_SHUTDOWN responses as there are START_SHUTDOWN receivers
/// Only errors if the initial message to start a shutdown cannot start.
pub async fn quit_game(deadline: Duration) -> Result<(), SendError> {
	let mut ready_receiver = READY_FOR_QUIT.receiver_subscribe();
	START_QUIT.send(())?;
	let num_receivers = START_QUIT.receiver_count();

	info!(
		"Attempting to shut down. Waiting on responses from {} listeners on the START_QUIT channel.",
		num_receivers
	);

	let mut timeout_future = Box::pin(tokio::time::sleep(deadline));

	let mut count_received = 0;

	while count_received < num_receivers {
		tokio::select! {
			reply_maybe = ready_receiver.recv_wait() => {
				match reply_maybe {
					Ok(_) => {
						trace!("Received {} quit ready notifications.", count_received);
						count_received += 1;
					}
					Err(e) => {
						error!("Error polling for READY_FOR_QUIT messages, exiting immediately. Error was: {:?}", e);
						return Ok(());
					}
				}
			}
			_ = (&mut timeout_future) => {
				error!("Waiting for disparate parts of the engine to be ready for quit took longer than {:?}, exiting immediately.", deadline);
				return Ok(());
			}
		}
	}

	Ok(())
}

// Intended constraints for ChannelSet:
// * Good ergonomics (should be able to get a channel by name without too much boilerplate)
// * No performance overhead compared to global channels for compiled-in channels. Should compile to
// just accessing the channel directly / no-middle-man for static channels.
// * Ergonomic "clone-into-subset" method
// * Introspectable? would be neat for scripting for later. Not required now but build around 
// the expectation

/// Type system hax to treat a unit struct as a compile-time-valid statically-known channel name / identifier.
/// This should be a zero-sized type
pub trait StaticChannelAtom {
	type Channel;
	type Message : Message;
	const DEFAULT_CAPACITY: usize;  
	fn get_static_name() -> &'static str;
	fn get_static_msg_ty() -> &'static str;
}

pub trait StaticDomainChannelAtom : StaticChannelAtom { 
	type Domain : ChannelDomain;
	fn get_static_domain_ty() -> &'static str;
}

macro_rules! static_channel_atom {
	($name:ident, $chan:ty, $message:ty, $capacity:literal) => {
		pub struct $name;
		impl crate::common::message::StaticChannelAtom for $name {
			type Channel = $chan;
			type Message = $message;
			const DEFAULT_CAPACITY: usize = $capacity;
			fn get_static_name() -> &'static str { 
				stringify!($name)
			}
			fn get_static_msg_ty() -> &'static str { 
				stringify!($message)
			}
		}
	};
	($name:ident, $chan:ty, $message:ty, $domain:ty, $capacity:literal) => {
		pub struct $name;
		impl crate::common::message::StaticChannelAtom for $name {
			type Channel = $chan;
			type Message = $message;
			const DEFAULT_CAPACITY: usize = $capacity;
			fn get_static_name() -> &'static str { 
				stringify!($name)
			}
			fn get_static_msg_ty() -> &'static str { 
				stringify!($message)
			}
		}
		impl crate::common::message::StaticDomainChannelAtom for $name {
			type Domain = $domain;
			fn get_static_domain_ty() -> &'static str { 
				stringify!($domain)
			}
		}
	};
}

pub trait HasChannel<C> where C: StaticChannelAtom {
	fn get_channel(&self) -> &C::Channel;
}
pub trait HasSender<C> where C: StaticChannelAtom, C::Channel: SenderChannel<C::Message> {
	fn get_sender(&self) -> &<C::Channel as SenderChannel<C::Message>>::Sender;
}
pub trait HasReceiver<C> where C: StaticChannelAtom, C::Channel: ReceiverChannel<C::Message> {
	fn get_receiver(&self) -> &<C::Channel as ReceiverChannel<C::Message>>::Receiver;
}

pub trait StaticSenderSubscribe<C> where C: StaticChannelAtom, C::Channel: SenderChannel<C::Message> { 
	fn sender_subscribe(&self) -> <<C as StaticChannelAtom>::Channel as SenderChannel<C::Message>>::Sender;
}

pub trait StaticReceiverSubscribe<C> where C: StaticChannelAtom, C::Channel: ReceiverChannel<C::Message> { 
	fn receiver_subscribe(&self) -> <<C as StaticChannelAtom>::Channel as ReceiverChannel<C::Message>>::Receiver;
}
pub trait StaticTakeReceiver<C> where C: StaticChannelAtom, C::Channel: ReceiverChannel<C::Message> { 
	fn take_receiver(&self) -> Result< <<C as StaticChannelAtom>::Channel as ReceiverChannel<C::Message>>::Receiver, DomainSubscribeErr<String> >;
}

pub trait StaticDomainSenderSubscribe<C> where C: StaticDomainChannelAtom, C::Channel: SenderChannel<C::Message> { 
	fn sender_subscribe(&self, domain: &C::Domain) -> Result<<<C as StaticChannelAtom>::Channel as SenderChannel<C::Message>>::Sender, DomainSubscribeErr<C::Domain>>;
}

pub trait StaticDomainReceiverSubscribe<C> where C: StaticDomainChannelAtom, C::Channel: ReceiverChannel<C::Message> { 
	fn receiver_subscribe(&self, domain: &C::Domain) -> Result<<<C as StaticChannelAtom>::Channel as ReceiverChannel<C::Message>>::Receiver, DomainSubscribeErr<C::Domain>>;
}

// Things may be getting a bit overcomplicated here. 
pub trait StaticDomainTakeReceiver<C> where C: StaticDomainChannelAtom, C::Channel: DomainTakeReceiver<C::Message, C::Domain> { 
	fn take_receiver(&self, domain: &C::Domain) -> Result<<<C as StaticChannelAtom>::Channel as ReceiverChannel<C::Message>>::Receiver, DomainSubscribeErr<C::Domain>>;
}

// Such gnarly type signatures are allowed only when needed for advanced procmacro shenanigans.
impl<T, C> StaticSenderSubscribe<C> for T where T: HasChannel<C>,
	C: StaticChannelAtom,
	C::Channel: SenderSubscribe<C::Message>, {
	fn sender_subscribe(&self) -> <<C as StaticChannelAtom>::Channel as SenderChannel<<C as StaticChannelAtom>::Message>>::Sender {
		self.get_channel().sender_subscribe()
	}
}

impl<T, C> StaticReceiverSubscribe<C> for T where T: HasChannel<C>,
	C: StaticChannelAtom,
	C::Channel: ReceiverSubscribe<C::Message>, {
	fn receiver_subscribe(&self) -> <<C as StaticChannelAtom>::Channel as ReceiverChannel<<C as StaticChannelAtom>::Message>>::Receiver {
		self.get_channel().receiver_subscribe()
	}
}
impl<T, C> StaticTakeReceiver<C> for T where T: HasChannel<C>,
	C: StaticChannelAtom,
	C::Channel: TakeReceiver<C::Message>, {
	fn take_receiver(&self) -> Result< <<C as StaticChannelAtom>::Channel as ReceiverChannel<C::Message>>::Receiver, DomainSubscribeErr<String> > {
		self.get_channel().take_receiver()
	}
}
impl<T, C> StaticDomainSenderSubscribe<C> for T where T: HasChannel<C>,
	C: StaticDomainChannelAtom,
	C::Channel: DomainSenderSubscribe<C::Message, C::Domain>, {
	fn sender_subscribe(&self, domain: &C::Domain) -> Result<<<C as StaticChannelAtom>::Channel as SenderChannel<<C as StaticChannelAtom>::Message>>::Sender, DomainSubscribeErr<<C as StaticDomainChannelAtom>::Domain>> {
		self.get_channel().sender_subscribe_domain(domain)
	}
}
impl<T, C> StaticDomainReceiverSubscribe<C> for T where T: HasChannel<C>,
	C: StaticDomainChannelAtom,
	C::Channel: DomainReceiverSubscribe<C::Message, C::Domain>, {
	fn receiver_subscribe(&self, domain: &C::Domain) -> Result<<<C as StaticChannelAtom>::Channel as ReceiverChannel<<C as StaticChannelAtom>::Message>>::Receiver, DomainSubscribeErr<<C as StaticDomainChannelAtom>::Domain>> {
		self.get_channel().receiver_subscribe_domain(domain)
	}
}
impl<T, C> StaticDomainTakeReceiver<C> for T where T: HasChannel<C>,
	C: StaticDomainChannelAtom,
	C::Channel: DomainTakeReceiver<C::Message, C::Domain> {
	fn take_receiver(&self, domain: &C::Domain) -> Result<<<C as StaticChannelAtom>::Channel as ReceiverChannel<<C as StaticChannelAtom>::Message>>::Receiver, DomainSubscribeErr<<C as StaticDomainChannelAtom>::Domain>> {
		self.get_channel().take_receiver_domain(domain)
	}
}

pub trait ChannelSet {
	type StaticBuilder;
}

/// T is the parent type - this is implemented on the smaller set, which can be narrowed down
/// from the greater set.
pub trait CloneSubset<T> : ChannelSet + Sized {
	/// Clones another channelset such that:
	/// 1. Every channel that this type has, and the original channel set has,
	/// gets cloned into this channel (via clone.().into())
	/// 2. Channels that the upstream struct has but our channel set does not get ignored.
	/// This is useful for restricting to a more-and-more fine-grained set of channels.
	fn build_from(parent: &T, builder: SubsetBuilder<Self::StaticBuilder>) -> Result<Self, DomainSubscribeErr<String>>;
}

pub trait BuildSubset<C> where C: ChannelSet {
	fn build_subset(&self, builder: SubsetBuilder<C::StaticBuilder>) -> Result<C, DomainSubscribeErr<String>> where Self: Sized;
}

impl<P, C> BuildSubset<C> for P where C: CloneSubset<P> + ChannelSet {
	fn build_subset(&self, builder: SubsetBuilder<C::StaticBuilder>) -> Result<C, DomainSubscribeErr<String>> {
		C::build_from(self, builder)
	}
}

pub struct SubsetBuilder<T> where T: Sized {
	pub static_fields: T,
	pub capacity_conf: ChannelCapacityConf,
}

impl<T> SubsetBuilder<T> where T: Sized {
	pub fn new(static_fields: T) -> Self {
		Self {
			static_fields,
			capacity_conf: ChannelCapacityConf::new(), 
		}
	}
}

#[cfg(test)]
pub mod test {
	use gestalt_proc_macros::ChannelSet;

	use crate::common::identity::IdentityKeyPair;

	use super::*;

	#[derive(Clone, Debug)]
	pub struct MessageA {
		pub msg: String,
	}

	#[derive(Clone, Debug)]
	pub struct MessageB {
		pub msg: String,
	}

	global_channel!(BroadcastChannel, TEST_CHANNEL, MessageA, 16);

	global_domain_channel!(BroadcastChannel, TEST_DOMAIN_CHANNEL, MessageB, NodeIdentity, 16);

	#[tokio::test(flavor = "multi_thread")]
	async fn send_into() {
		#[derive(Debug, Clone)]
		struct Foo {
			first: u32,
		}
		#[derive(Debug, Clone)]
		struct Bar {
			second: u64,
		}

		impl Into<Bar> for Foo {
			fn into(self) -> Bar {
				Bar {
					second: self.first as u64,
				}
			}
		}

		let test_struct = Foo { first: 1234 };

		let channel: BroadcastChannel<Bar> = BroadcastChannel::new(16);
		let sender = channel.sender_subscribe();
		let mut receiver = channel.receiver_subscribe();
		let mut second_receiver = channel.receiver_subscribe();
		//send_one
		sender.send(test_struct.into()).unwrap();

		let out = receiver.recv_wait().await.unwrap();
		assert_eq!(out.second, 1234);

		let out2 = second_receiver.recv_wait().await.unwrap();
		assert_eq!(out2.second, 1234);
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn global_subscribe() {
		let sender = TEST_CHANNEL.sender_subscribe();
		let mut receiver = TEST_CHANNEL.receiver_subscribe();

		sender
			.send(MessageA {
				msg: String::from("Hello, world!"),
			})
			.unwrap();
		let out_msg = receiver.recv_wait().await.unwrap();

		assert_eq!(out_msg.msg, String::from("Hello, world!"));
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn domain_channels() {
		let player_identity = IdentityKeyPair::generate_for_tests().public;
		let some_other_player_identity = IdentityKeyPair::generate_for_tests().public;

		let _ = TEST_DOMAIN_CHANNEL.init_domain(player_identity.clone());
		let _ = TEST_DOMAIN_CHANNEL.init_domain(some_other_player_identity.clone());
		let sender = TEST_DOMAIN_CHANNEL
			.sender_subscribe(&player_identity)
			.unwrap();
		let mut receiver = TEST_DOMAIN_CHANNEL
			.receiver_subscribe(&player_identity)
			.unwrap();

		let other_channel_sender = TEST_DOMAIN_CHANNEL
			.sender_subscribe(&some_other_player_identity)
			.unwrap();
		let mut other_channel_receiver = TEST_DOMAIN_CHANNEL
			.receiver_subscribe(&some_other_player_identity)
			.unwrap();

		sender
			.send(MessageB {
				msg: String::from("Hello, player1!"),
			})
			.unwrap();
		other_channel_sender
			.send(MessageB {
				msg: String::from("Hello, player2!"),
			})
			.unwrap();

		{
			let out_msg = receiver.recv_wait().await.unwrap();
			assert_eq!(out_msg.msg, String::from("Hello, player1!"));
		}

		{
			let out_msg = other_channel_receiver.recv_wait().await.unwrap();
			assert_eq!(out_msg.msg, String::from("Hello, player2!"));
		}
	}
	#[tokio::test(flavor = "multi_thread")]
	async fn channel_set_subset() { 
		static_channel_atom!(EvenSpecialerString, MpscChannel<u32>, u32, 64);
		static_channel_atom!(SpecialStringGoesHere, BroadcastChannel<u32>, u32, 128);
	
		#[derive(ChannelSet)]
		struct ChannelSetTestA {
			#[channel(EvenSpecialerString)]
			pub chan1: MpscChannel<u32>,
			#[channel(SpecialStringGoesHere)]
			pub chan2: BroadcastChannel<u32>,
		}
		#[derive(ChannelSet)]
		struct ChannelSetTestB {
			#[channel(EvenSpecialerString)]
			pub foo: MpscChannel<u32>,
			#[receiver(SpecialStringGoesHere)]
			pub bar: BroadcastReceiver<u32>,
		}
		#[derive(ChannelSet)]
		struct ChannelSetTestC {
			#[sender(EvenSpecialerString)]
			pub foo: MpscSender<u32>,
		}

		let foo_channel: MpscChannel<u32> = MpscChannel::new(12);
		let bar_channel: BroadcastChannel<u32> = BroadcastChannel::new(20);

		let top_level = ChannelSetTestA { 
			chan1: foo_channel,
			chan2: bar_channel,
		};

		let mut middle_level: ChannelSetTestB = top_level.build_subset(SubsetBuilder::new(())).unwrap();
		let bottom_level: ChannelSetTestC = middle_level.build_subset(SubsetBuilder::new(())).unwrap();

		let mut foo_receiver = top_level.chan1.take_receiver().unwrap();

		let testnum = 42;
		bottom_level.foo.send(testnum).unwrap();
		let number = foo_receiver.recv_wait().await.unwrap();
		assert_eq!(testnum, number);

		let testnum = 13;
		top_level.chan2.send(testnum).unwrap();
		let number = middle_level.bar.recv_wait().await.unwrap();
		assert_eq!(testnum, number);
	}
	#[tokio::test(flavor = "multi_thread")]
	async fn channel_set_domains() {
		use super::MessageReceiver;
		static_channel_atom!(DividedChannel, DomainMultiChannel<String, NodeIdentity, MpscChannel<String>>, String, NodeIdentity, 128);
	
		#[derive(ChannelSet)]
		struct ChannelSetTestA {
			#[channel(DividedChannel, new_channel)]
			pub foo: <DividedChannel as StaticChannelAtom>::Channel,
		}
		#[derive(ChannelSet)]
		struct CChannels {
			#[take_receiver(DividedChannel, domain = "our_ident")]
			pub foo: <MpscChannel<String> as ReceiverChannel<String>>::Receiver,
		}
		#[derive(ChannelSet)]
		struct SChannels {
			#[sender(DividedChannel, domain = "client")]
			pub bar: <MpscChannel<String> as SenderChannel<String>>::Sender,
		}

		let parent_set = ChannelSetTestA::new(SubsetBuilder::new(()));
		let client_a_keys = IdentityKeyPair::generate_for_tests();
		let client_b_keys = IdentityKeyPair::generate_for_tests();
		//let server_keys =  = IdentityKeyPair::generate_for_tests();

		parent_set.foo.init_domain(client_a_keys.public.clone()).unwrap();
		parent_set.foo.init_domain(client_b_keys.public.clone()).unwrap();

		let mut client_a: CChannels = parent_set.build_subset(SubsetBuilder::new(CChannelsFields{
			our_ident_domain: client_a_keys.public.clone(),
		})).unwrap();

		let mut client_b: CChannels = parent_set.build_subset(SubsetBuilder::new(CChannelsFields{
			our_ident_domain: client_b_keys.public.clone(),
		})).unwrap();

		let server_to_client_a: SChannels = parent_set.build_subset(SubsetBuilder::new(SChannelsFields{
			client_domain: client_a_keys.public.clone(),
		})).unwrap();
		let server_to_client_b: SChannels = parent_set.build_subset(SubsetBuilder::new(SChannelsFields{
			client_domain: client_b_keys.public.clone(),
		})).unwrap();

		let message_to_a = String::from("William Lawrence");
		let message_to_b = String::from("Temeraire");

		server_to_client_a.bar.send(message_to_a.clone()).unwrap();

		assert_eq!(client_a.foo.recv_poll().as_ref().map(|o| o.as_ref()), Ok(Some(&message_to_a)));
		// Only one message.
		assert_eq!(client_a.foo.recv_poll(), Ok(None));
		// Only to one domain. 
		assert_eq!(client_b.foo.recv_poll(), Ok(None));

		server_to_client_b.bar.send(message_to_b.clone()).unwrap();

		assert_eq!(client_b.foo.recv_poll().as_ref().map(|o| o.as_ref()), Ok(Some(&message_to_b)));
		// Only one message.
		assert_eq!(client_b.foo.recv_poll(), Ok(None));
		// Only to one domain. 
		assert_eq!(client_a.foo.recv_poll(), Ok(None));

		let broadcaster = parent_set.foo.sender_subscribe_all();

		let message = String::from("Vizlet");

		broadcaster.send_to_all(message.clone()).unwrap(); 
		assert_eq!(client_a.foo.recv_poll().as_ref().map(|o| o.as_ref()), Ok(Some(&message)));
		assert_eq!(client_b.foo.recv_poll().as_ref().map(|o| o.as_ref()), Ok(Some(&message)));
		assert_eq!(client_a.foo.recv_poll(), Ok(None));
		assert_eq!(client_b.foo.recv_poll(), Ok(None));

	}
}
