use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

use futures::{Future, TryFutureExt};
use log::{error, info, trace};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;

use crate::world::WorldId;

use super::identity::NodeIdentity;

#[derive(thiserror::Error, Debug, Clone)]
pub enum RecvError {
	#[error("Failed to send a message onto a message channel, because there are no remaining receivers associated with this sender.")]
	NoSenders,
	#[error("A channel hit its maximum number of stored messages and this channel was keeping alive old messages. {0} messages have been skipped and can no longer be retrieved.")]
	Lagged(u64),
	#[error("Implementation-specific channel error: {0}.")]
	Other(String),
}

pub trait Message: Clone + Send + Debug {}
impl<T> Message for T where T: Clone + Send + Debug {}

pub type BroadcastSender<T> = tokio::sync::broadcast::Sender<Vec<T>>;
type UnderlyingBroadcastReceiver<T> = tokio::sync::broadcast::Receiver<Vec<T>>;

#[derive(thiserror::Error, Debug, Clone)]
pub enum SendError {
	#[error("Failed to send a message onto a message channel, because there are no remaining receivers associated with this sender.")]
	NoReceivers,
	#[error("Could not send on a channel because domain {0} is not registered yet")]
	MissingDomain(String),
	#[error("Unable to encode a message so it could be sent on channel: {0}.")]
	Encode(String),
	#[error("Implementation-specific channel error: {0}.")]
	Other(String),
}

impl<T> From<tokio::sync::broadcast::error::SendError<T>> for SendError {
	fn from(_value: tokio::sync::broadcast::error::SendError<T>) -> Self {
		SendError::NoReceivers
	}
}

pub trait MessageReceiver<T>
where
	T: Message,
{
	/// Nonblockingly polls for new messages, returning an empty vector if the channel is empty.  
	fn recv_poll(&mut self) -> Result<Vec<T>, RecvError>;
}
pub trait MessageReceiverAsync<T>: MessageReceiver<T>
where
	T: Message,
{
	//type RecvFuture: Future<Output=Result<Vec<T>, RecvError>>;
	fn recv_wait(&mut self) -> impl Future<Output = Result<Vec<T>, RecvError>> + '_;
}

pub struct BroadcastReceiver<T>
where
	T: Message,
{
	pub(in crate::common::message) inner: UnderlyingBroadcastReceiver<T>,
}

impl<T> BroadcastReceiver<T>
where
	T: Message,
{
	pub fn new(to_wrap: tokio::sync::broadcast::Receiver<Vec<T>>) -> Self {
		BroadcastReceiver { inner: to_wrap }
	}

	pub fn resubscribe(&self) -> Self {
		BroadcastReceiver {
			inner: self.inner.resubscribe(),
		}
	}

	async fn recv_wait_inner(&mut self) -> Result<Vec<T>, RecvError> {
		let mut resl = self
			.inner
			.recv()
			.map_err(|e| match e {
				broadcast::error::RecvError::Closed => RecvError::NoSenders,
				broadcast::error::RecvError::Lagged(count) => RecvError::Lagged(count),
			})
			.await?;
		while resl.is_empty() {
			// Keep trying until we get an actual thing.
			resl = self
				.inner
				.recv()
				.map_err(|e| match e {
					broadcast::error::RecvError::Closed => RecvError::NoSenders,
					broadcast::error::RecvError::Lagged(count) => RecvError::Lagged(count),
				})
				.await?;
		}
		// Check to see if there's anything else also waiting for us, but do not block for it.
		let mut maybe_more = self.recv_poll()?;
		resl.append(&mut maybe_more);
		Ok(resl)
	}
}

impl<T> MessageReceiver<T> for BroadcastReceiver<T>
where
	T: Message,
{
	/// Nonblockingly polls for new messages, returning an empty vector if the channel is empty.  
	fn recv_poll(&mut self) -> Result<Vec<T>, RecvError> {
		let mut results: Vec<T> = Vec::new();
		let mut next_value = self.inner.try_recv();
		while let Ok(mut val) = next_value {
			if results.is_empty() {
				results = val;
			} else {
				results.append(&mut val);
			}
			next_value = self.inner.try_recv();
		}
		if let Err(err) = next_value {
			match err {
				TryRecvError::Empty => {}
				TryRecvError::Closed => return Err(RecvError::NoSenders),
				TryRecvError::Lagged(count) => return Err(RecvError::Lagged(count)),
			}
		}
		Ok(results)
	}
}

impl<T> MessageReceiverAsync<T> for BroadcastReceiver<T>
where
	T: Message,
{
	/// Receives new messages batch, waiting for a message if the channel is currently empty.
	fn recv_wait(&mut self) -> impl Future<Output = Result<Vec<T>, RecvError>> + '_ {
		self.recv_wait_inner()
	}
}

pub trait ChannelDomain: Send + Clone + PartialEq + Eq + PartialOrd + Hash + Debug {}
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
	/// Returns true if this sender would (most likely because the channel is full)
	/// block on  on an attempt to send.
	fn would_block(&self) -> bool;

	/// Send a batch of messages. If the underlying
	fn send_multi<V>(&self, messages: V) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>;

	/// Send a single message.
	fn send_one(&self, message: T) -> Result<(), SendError> {
		self.send_multi(vec![message])
	}
}

pub trait DomainMessageSender<T, D>
where
	T: Message,
	D: ChannelDomain,
{
	/// Send a batch of messages to one domain
	fn send_multi_to<V>(&self, messages: V, domain: &D) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>;

	/// Send one message to one domain
	fn send_one_to(&self, message: T, domain: &D) -> Result<(), SendError> {
		self.send_multi_to(vec![message], domain)
	}

	/// Send one message to every domain
	fn send_one_to_all(&self, message: T) -> Result<(), SendError>;

	/// Send a batch of messages to every domain
	fn send_multi_to_all<V>(&self, messages: V) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>;

	/// Send one message to every domain, excluding the domain 'exclude'
	fn send_one_to_all_except(&self, message: T, exclude: &D) -> Result<(), SendError>;

	/// Send a batch of messages to every domain, excluding the domain 'exclude'
	fn send_multi_to_all_except<V>(&self, messages: V, exclude: &D) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>;
}

impl<T> MessageSender<T> for BroadcastSender<T>
where
	T: Message,
{
	fn would_block(&self) -> bool {
		false
	}

	fn send_multi<V>(&self, messages: V) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>,
	{
		self.send(messages.into_iter().collect())
			.map(|_| ())
			.map_err(|e| e.into())
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
	// The trait does not include the Receiver because an
	// mpsc channel will only have one consumer - so, the
	// receiver is not something we can subscribe to.

	fn sender_subscribe(&self) -> Self::Sender;
}

/// Trait that lets you get a receiver to receive from a message-passing channel.
/// This is separate from SenderChannel because some types
/// of channels, for example any mpsc channel, might let you make
/// many senders but there would be only one receiver
/// (so you can't subscribe additional receivers into existence).
pub trait ReceiverChannel<T>
where
	T: Message,
{
	type Receiver: MessageReceiver<T>;
	// The trait does not include the Receiver because an
	// mpsc channel will only have one consumer - so, the
	// receiver is not something we can subscribe to.

	fn receiver_subscribe(&self) -> Self::Receiver;
}

pub trait MpmcChannel<T: Message>: SenderChannel<T> + ReceiverChannel<T> {}
impl<T, U> MpmcChannel<T> for U
where
	T: Message,
	U: SenderChannel<T> + ReceiverChannel<T>,
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
	T: Message,
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

impl<T> SenderChannel<T> for BroadcastChannel<T>
where
	T: Message,
{
	type Sender = BroadcastSender<T>;
	fn sender_subscribe(&self) -> BroadcastSender<T> {
		self.sender.clone()
	}
}

impl<T> ReceiverChannel<T> for BroadcastChannel<T>
where
	T: Message,
{
	type Receiver = BroadcastReceiver<T>;
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
	R: Message,
{
	fn send_multi<V>(&self, messages: V) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>,
	{
		self.sender
			.send(messages.into_iter().map(|val| val.into()).collect())
			.map_err(|_e| SendError::NoReceivers)
			.map(|_val| ())
	}

	fn would_block(&self) -> bool {
		false
	}
}

impl<T> ChannelInit for BroadcastChannel<T>
where
	T: Message,
{
	fn new(capacity: usize) -> Self {
		BroadcastChannel::new(capacity)
	}
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum DomainSubscribeErr<D>
where
	D: ChannelDomain,
{
	#[error("Cannot subscribe to a channel in domain {0:?} because that domain has not been registered.")]
	NoDomain(D),
}

pub struct DomainMultiChannel<T, D, C>
where
	T: Message,
	D: ChannelDomain,
	C: SenderChannel<T> + ChannelInit,
{
	/// This is carried into any channels we will initialize
	capacity: usize,

	channels: ChannelMutex<std::collections::HashMap<D, C>>,

	_message_ty_phantom: PhantomData<T>,
}

impl<T, D, C> DomainMultiChannel<T, D, C>
where
	T: Message,
	D: ChannelDomain,
	C: SenderChannel<T> + ChannelInit,
{
	/// Construct a Domain Multichannel system.
	pub fn new(capacity: usize) -> Self {
		DomainMultiChannel {
			capacity,
			channels: ChannelMutex::new(std::collections::HashMap::new()),
			_message_ty_phantom: Default::default(),
		}
	}

	pub fn sender_subscribe(&self, domain: &D) -> Result<C::Sender, DomainSubscribeErr<D>> {
		Ok(self
			.channels
			.lock()
			.get_mut(domain)
			.ok_or_else(|| DomainSubscribeErr::NoDomain(domain.clone()))?
			.sender_subscribe())
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
	pub fn add_domain(&self, domain: &D) {
		self.channels
			.lock()
			.entry(domain.clone())
			.or_insert(C::new(self.capacity));
	}
	pub fn drop_domain(&self, domain: &D) {
		let lock = self.channels.lock();
		let contains = lock.contains_key(domain);
		drop(lock);
		if contains {
			self.channels.lock().remove(domain);
		}
	}
}

impl<T, D, C> DomainMultiChannel<T, D, C>
where
	T: Message,
	D: ChannelDomain,
	C: SenderChannel<T> + ReceiverChannel<T> + ChannelInit,
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

impl<T, D, C> DomainMessageSender<T, D> for DomainMultiChannel<T, D, C>
where
	T: Message,
	D: ChannelDomain,
	C: SenderChannel<T> + ChannelInit + MessageSender<T>,
{
	fn send_multi_to<V>(&self, messages: V, domain: &D) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>,
	{
		match self.channels.lock().get(domain) {
			Some(chan) => chan
				.send_multi(messages)
				.map_err(|_e| SendError::NoReceivers)
				.map(|_val| ()),
			None => Err(SendError::MissingDomain(format!("{:?}", domain))),
		}
	}

	fn send_one_to_all(&self, message: T) -> Result<(), SendError> {
		for chan in self.channels.lock().values() {
			chan.send_one(message.clone())?;
		}
		Ok(())
	}

	fn send_multi_to_all<V>(&self, messages: V) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>,
	{
		let message_buf: Vec<T> = messages.into_iter().collect();
		for chan in self.channels.lock().values() {
			chan.send_multi(message_buf.clone())?;
		}
		Ok(())
	}

	fn send_one_to_all_except(&self, message: T, exclude: &D) -> Result<(), SendError> {
		for (domain, chan) in self.channels.lock().iter() {
			if domain != exclude {
				chan.send_one(message.clone())?;
			}
		}
		Ok(())
	}

	fn send_multi_to_all_except<V>(&self, messages: V, exclude: &D) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>,
	{
		let message_buf: Vec<T> = messages.into_iter().collect();
		for (domain, chan) in self.channels.lock().iter() {
			if domain != exclude {
				chan.send_multi(message_buf.clone())?;
			}
		}
		Ok(())
	}
}

impl<T, D, R, C> MessageSender<T> for DomainMultiChannel<R, D, C>
where
	T: Into<R> + MessageWithDomain<D>,
	D: ChannelDomain,
	R: MessageWithDomain<D>,
	C: SenderChannel<R> + ChannelInit + MessageSender<R>,
{
	fn send_multi<V>(&self, messages: V) -> Result<(), SendError>
	where
		V: IntoIterator<Item = T>,
	{
		for message in messages {
			let message = message.into();
			let domain = message.get_domain().clone();
			self.send_one_to(message, &domain)
				.map_err(|_e| SendError::NoReceivers)?;
		}
		Ok(())
	}

	fn would_block(&self) -> bool {
		false
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
macro_rules! global_domain_channel {
	($chanty:ident, $name:ident, $message:ty, $domain:ty, $capacity:expr) => {
		lazy_static::lazy_static! {
			pub static ref $name: crate::common::message::DomainMultiChannel<$message, $domain, $chanty<$message>> = {
				crate::common::message::DomainMultiChannel::new($capacity)
			};
		}
	};
}

// A few *very universal* channels can exist in this file.
global_channel!(BroadcastChannel, START_QUIT, (), 1);
global_channel!(BroadcastChannel, READY_FOR_QUIT, (), 1024);

#[derive(Clone)]
#[warn(unused_must_use)]
pub struct QuitReadyNotifier {
	inner: BroadcastSender<()>,
}

impl QuitReadyNotifier {
	pub fn notify_ready(self) {
		trace!("Sending quit-ready notification.");
		let _ = self.inner.send_one(());
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
	START_QUIT.send_one(())?;
	let num_receivers = START_QUIT.receiver_count();

	info!(
		"Attempting to shut down. Waiting on responses from {} listeners on the START_QUIT channel.",
		num_receivers
	);

	let mut timeout_future = Box::pin(tokio::time::sleep(deadline));

	let mut count_received = 0;

	while count_received < num_receivers {
		tokio::select! {
			replies_maybe = ready_receiver.recv_wait() => {
				match replies_maybe {
					Ok(v) => {
						let count = v.len();
						trace!("Received {} quit ready notifications.", count);
						count_received += count;
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

#[cfg(test)]
pub mod test {
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
		sender.send_one(test_struct.into()).unwrap();

		let out = receiver.recv_wait().await.unwrap();
		let out = out.first().unwrap();
		assert_eq!(out.second, 1234);

		let out2 = second_receiver.recv_wait().await.unwrap();
		let out2 = out2.first().unwrap();
		assert_eq!(out2.second, 1234);
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn global_subscribe() {
		let sender = TEST_CHANNEL.sender_subscribe();
		let mut receiver = TEST_CHANNEL.receiver_subscribe();

		sender
			.send_one(MessageA {
				msg: String::from("Hello, world!"),
			})
			.unwrap();
		let mut output = receiver.recv_wait().await.unwrap();
		assert_eq!(output.len(), 1);
		let out_msg = output.drain(0..1).next().unwrap();

		assert_eq!(out_msg.msg, String::from("Hello, world!"));
	}

	#[tokio::test(flavor = "multi_thread")]
	async fn domain_channels() {
		let player_identity = IdentityKeyPair::generate_for_tests().public;
		let some_other_player_identity = IdentityKeyPair::generate_for_tests().public;

		TEST_DOMAIN_CHANNEL.add_domain(&player_identity);
		TEST_DOMAIN_CHANNEL.add_domain(&some_other_player_identity);
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
			.send_one(MessageB {
				msg: String::from("Hello, player1!"),
			})
			.unwrap();
		other_channel_sender
			.send_one(MessageB {
				msg: String::from("Hello, player2!"),
			})
			.unwrap();

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

	global_channel!(BroadcastChannel, TEST_CHANNEL_C, MessageC, 128);

	#[tokio::test(flavor = "multi_thread")]
	async fn message_batching() {
		let sender = TEST_CHANNEL_C.sender_subscribe();
		let mut receiver = TEST_CHANNEL_C.receiver_subscribe();

		const NUM_MESSAGES: usize = 64;
		//Many separate sends...
		for i in 0..NUM_MESSAGES as u64 {
			sender
				.send_one(MessageC {
					msg: String::from("Hello, world!"),
					val: i,
				})
				.unwrap();
		}

		let output = receiver.recv_poll().unwrap();
		assert_eq!(output.len(), NUM_MESSAGES);
		assert_eq!(receiver.inner.try_recv(), Err(TryRecvError::Empty));
	}
}
