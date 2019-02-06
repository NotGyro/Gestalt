extern crate serde;
extern crate linear_map;
extern crate std;
extern crate crossbeam;

use std::fmt::Debug;
use serde::{Serialize, Deserialize};
use crossbeam::crossbeam_channel::{unbounded, Sender, Receiver, SendError, RecvError, TryRecvError};
use self::linear_map::LinearMap;
use std::result::Result;

type RevisionNumber = usize;

pub trait Event : Clone + Serialize + Send + Sync + Debug {}
impl<T> Event for T where T : Clone + Serialize + Send + Sync + Debug {}

/// An event bus is any type that sends and receives events of type T asynchronously 
pub trait EventBus <T> where T : Event {
    /// Gives you a Crossbeam Sender to push events to this bus.
    fn get_sender(&self) -> Sender<T>;
    /// Gives you a Crossbeam Receiver where you can poll events from this bus, and an ID you can
    /// use to unsubscribe later.
    fn subscribe(&mut self) -> (Receiver<T>, usize);
    /// Drops our Sender to the specified channel, stops trying to send events there.
    fn unsubscribe(&mut self, id : usize);
    /// Pushes an event directly onto this Event Bus if you're the one who owns it.
    fn push(&mut self, ev : T) -> Result<(), SendError<T>>;
}


/// An event bus that multicasts incoming events out to all consumers.
pub struct SimpleEventBus<T> where T : Event { 
    /// This is where events sent to the bus / journal will go.
    our_receiver : Receiver<T>,
    /// Used to clone repeatedly for senders to this bus
    sender_template : Sender<T>,
    /// A list of senders for registered consumers. Each Receiver is owned by the consumer.
    /// Using a Linear Map since we're going to spend a lot more time iterating over every
    /// consumer than we will accessing them by ID.
    consumers : LinearMap<usize, Sender<T>>,
    /// Incremented to come up with new consumer IDs
    consumer_count : usize,
}

impl <T> EventBus<T> for SimpleEventBus<T> where T : Event {    
    /// Gives you a Crossbeam Sender to push events to this bus.
    fn get_sender(&self) -> Sender<T> { self.sender_template.clone() }
    /// Gives you a Crossbeam Receiver where you can poll events from this bus, and an ID you can
    /// use to unsubscribe later.
    fn subscribe(&mut self) -> (Receiver<T>, usize) { 
        let (s, r) = unbounded();
        self.consumers.insert(self.consumer_count, s);
        let ret_count = self.consumer_count;
        self.consumer_count += 1;
        return (r, ret_count);
    }
    /// Drops our Sender to the specified channel, stops trying to send events there.
    fn unsubscribe(&mut self, id : usize) { self.consumers.remove(&id); }
    /// Pushes an event directly onto this Event Bus if you're the one who owns it.
    fn push(&mut self, ev : T) -> Result<(), SendError<T>> { self.sender_template.send(ev) }
}

impl <T> SimpleEventBus<T> where T : Event {
    fn new() -> SimpleEventBus<T> { 
        let (s_in, r_in) = unbounded();
        SimpleEventBus { our_receiver : r_in, sender_template : s_in, consumers : LinearMap::new(), consumer_count : 0 }
    }
    /// Take received events in, multicast to consumers.
    /// Never used when it;s an inner type in EventJournalBus
    fn process(&mut self) { 
        for ev in self.our_receiver.try_iter() {
            for (_, consumer) in self.consumers.iter_mut() { 
                consumer.send(ev.clone()).expect( format!("A SimpleEventBus failed to send an event! Event contents: {:?}", ev.clone()).as_str() );
            }
        }
    }
}

/// A common list of events of type T that have occurred so far in its context.
/// This establishes a history.
///
/// A major flaw with this: You need to keep a record of every event that has
/// ever happened in this context in memory in order to have a valid revision
/// number, since the revision number here is just an index into the event
/// vector. TODO: Paging.
/// revision_offset isn't really used much yet, it's basically a stub
/// of to-be-implemented paging functionality. 
pub struct EventJournal<T> where T : Event { 
    pub events : Vec<T>,
    revision_offset : RevisionNumber,
}

impl <T> EventJournal<T> where T : Event { 
    /// Constructs a new EventJournal containing events of type T.
    fn new() -> EventJournal<T> {
        EventJournal { events : Vec::with_capacity(128), revision_offset : 0 }
    }
    fn revision(&self) -> RevisionNumber {
        self.events.len() + self.revision_offset
    }
    fn push(&mut self, ev : T) { self.events.push(ev) }
}

/// An EventBus that Journals everything that goes through it.
pub struct EventJournalBus<T> where T : Event { 
    pub journal : EventJournal<T>,
    bus : SimpleEventBus<T>,
}
impl <T> EventBus<T> for EventJournalBus<T> where T : Event {
     /// Gives you a Crossbeam Sender to push events to this bus.
    fn get_sender(&self) -> Sender<T> { self.bus.get_sender() }
    /// Gives you a Crossbeam Receiver where you can poll events from this bus, and an ID you can
    /// use to unsubscribe later.
    fn subscribe(&mut self) -> (Receiver<T>, usize) { self.bus.subscribe() }
    /// Drops our Sender to the specified channel, stops trying to send events there.
    fn unsubscribe(&mut self, id : usize) { self.bus.unsubscribe(id) }
    /// Pushes an event directly onto this Event Bus if you're the one who owns it.
    fn push(&mut self, ev : T) -> Result<(), SendError<T>> { self.bus.push(ev); return Ok(()) }
}
impl <T> EventJournalBus<T> where T : Event { 
    fn new() -> EventJournalBus<T> { 
        EventJournalBus{ journal : EventJournal::new(), bus : SimpleEventBus::new() }
    }
    /// Take events from our input channel, push them to journal, and then send to consumers.
    fn process(&mut self) { 
        for ev in self.bus.our_receiver.try_iter() {
            self.journal.push(ev.clone());
            for (_, consumer) in self.bus.consumers.iter_mut() { 
                consumer.send(ev.clone()).expect( format!("An EventJournalBus failed to send an event! Event contents: {:?}", ev.clone()).as_str() );
            }
        }
    }
    fn revision(&self) -> RevisionNumber { self.journal.revision() }
}


#[derive(Clone, Serialize, Debug)]
struct TestEvent {
    name : String,
    apples : i32,
}

#[test]
fn TryEventJournalBus() { 
    let mut bus : EventJournalBus<TestEvent> = EventJournalBus::new();
    let (mut subscriber1, _) = bus.subscribe();
    let mut subscribers : Vec<Receiver<TestEvent>> = Vec::new(); 

    let ev1 = TestEvent{ name : "Voksa".to_string(), apples : 14 };
    let ev2 = TestEvent{ name : "Kasran".to_string(), apples : 34 };
    let ev3 = TestEvent{ name : "byte".to_string(), apples: 7 };

    for _ in 0..10 {
        let (mut s, subid) = bus.subscribe();
        subscribers.push(s);
    }
    let snd = bus.get_sender();
    assert!(snd.send(ev1).is_ok());
    assert!(snd.send(ev2).is_ok());
    assert!(bus.push(ev3).is_ok());
    bus.process();
    for sub in subscribers {
        assert_eq!(sub.len(), 3);
        assert_eq!(sub.recv().expect("Failed the test!").name, "Voksa".to_string());
        assert_eq!(sub.recv().expect("Failed the test!").name, "Kasran".to_string());
        assert_eq!(sub.recv().expect("Failed the test!").name, "byte".to_string());
    }
    assert_eq!(subscriber1.recv().expect("Failed the test!").name, "Voksa".to_string());
    assert_eq!(subscriber1.recv().expect("Failed the test!").name, "Kasran".to_string());
    assert_eq!(subscriber1.recv().expect("Failed the test!").name, "byte".to_string());
    assert_eq!(bus.revision(), 3);
}
