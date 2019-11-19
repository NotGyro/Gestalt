#[macro_use] extern crate lazy_static;
extern crate chrono;
extern crate parking_lot;

use chrono::{DateTime, Local};
use std::collections::VecDeque;
use parking_lot::Mutex;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Arc;
use std::time::Instant;
use std::sync::atomic::{Ordering, AtomicU64};


pub mod prelude {
    pub use super::{Verbosity, Scope};
}


// Public types ////////////////////////////////////////////////////////////////////////////////////


#[derive(Debug, Clone, Copy)]
pub enum Verbosity {
    Fatal = 0,
    Error = 1,
    Warning = 2,
    Info = 3,
    Verbose = 4,
    Trace = 5,
}


#[derive(Debug, Clone)]
pub struct Scope {
    name: String,
    log: Verbosity,
    print: Verbosity,
    display: Verbosity
}


impl Scope {
    pub fn new(name: &str) -> Self {
        Scope {
            name: name.to_string(),
            log: Verbosity::Info,
            print: Verbosity::Warning,
            display: Verbosity::Error,
        }
    }
    pub fn log(&self, verbosity: Verbosity) -> Self {
        Scope {
            name: self.name.clone(),
            log: verbosity,
            print: self.print,
            display: self.display,
        }
    }
    pub fn print(&self, verbosity: Verbosity) -> Self {
        Scope {
            name: self.name.clone(),
            log: self.log,
            print: verbosity,
            display: self.display,
        }
    }
    pub fn display(&self, verbosity: Verbosity) -> Self {
        Scope {
            name: self.name.clone(),
            log: self.log,
            print: self.print,
            display: verbosity,
        }
    }
}


// Private types ///////////////////////////////////////////////////////////////////////////////////


#[derive(Debug, Clone)]
struct Record {
    verbosity: Verbosity,
    time: DateTime<Local>,
    tick: u64,
    scope_id: u32,
    file: String,
    line_and_col: (u32, u32),
    message: String,
}

impl Record {
    pub fn to_csv(&self, scope_name: &str) -> String {
        format!("{},{},{},{:?},{},{},{},{}\n", self.time.format("%Y/%b/%d-%H:%M:%S%.3f"),
                self.tick, scope_name, self.verbosity,
                self.file, self.line_and_col.0, self.line_and_col.1, self.message)
    }
    pub fn to_stdout(&self, scope_name: &str) -> String {
        format!("{} ({}) [{}][{:?}] {} @ {}:{}: {}", self.time.format("%H:%M:%S%.3f"),
                self.tick, scope_name, self.verbosity,
                self.file, self.line_and_col.0, self.line_and_col.1, self.message)
    }
}


struct LoggerState {
    scopes: Vec<Scope>,
    next_scope_id: u32,
    records: VecDeque<Record>,
    log_path: String,
    last_flush: Instant,
    tick: Arc<AtomicU64>,
}

impl LoggerState {
    pub fn new(path: &str) -> Self {
        let mut file = OpenOptions::new().write(true)
                                         .create(true)
                                         .truncate(true)
                                         .open(path)
                                         .unwrap();
        write!(file, "timestamp,tick,scope,verbosity,file,line,column,message\n").unwrap();
        LoggerState {
            scopes: Vec::new(),
            next_scope_id: 0,
            records: VecDeque::new(),
            log_path: path.to_string(),
            last_flush: Instant::now(),
            tick: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn add_record(&mut self, record: Record) {
        let scope = &self.scopes[record.scope_id as usize];
        if record.verbosity as u32 <= scope.print as u32 {
            println!("{}", record.to_stdout(&scope.name));
        }
        self.records.push_back(record);
        let now = Instant::now();
        if now.duration_since(self.last_flush).as_secs_f32() > 1.0 {
            self.last_flush = now;
            self.flush();
        }
    }
    pub fn flush(&mut self) {
        if self.records.len() > 0 {
            match OpenOptions::new()
                .read(false)
                .append(true)
                .create(true)
                .truncate(false)
                .open(self.log_path.as_str()) {
                Ok(mut file) => {
                    loop {
                        match self.records.pop_front() {
                            Some(r) => {
                                write!(file, "{}", r.to_csv(&self.scopes[r.scope_id as usize].name)).unwrap();
                            }
                            None => break
                        }
                    }
                },
                Err(e) => { println!("couldn't open log file: {:?}", e); }
            }
        }
    }
}

lazy_static! {
    static ref STATE: Arc<Mutex<LoggerState>> = Arc::new(Mutex::new(LoggerState::new("log.csv")));
}

pub fn set_tick_arc(tick: Arc<AtomicU64>) {
    STATE.lock().tick = tick.clone();
}

pub fn register_scope(scope: Scope) -> u32 {
    let mut lock = STATE.lock();
    let id = lock.next_scope_id;
    lock.next_scope_id += 1;
    lock.scopes.push(scope);
    id
}


pub fn log_internal(verbosity: Verbosity, scope_id: u32, file: &str, line_and_col: (u32, u32), message: String) {
    let mut lock = STATE.lock();
    let r = Record {
        verbosity,
        time: Local::now(),
        tick: lock.tick.load(Ordering::Relaxed),
        scope_id,
        file: file.to_string(),
        line_and_col,
        message,
    };
    lock.add_record(r);
}

#[macro_export(local_inner_macros)]
macro_rules! log_macro_internal {
    ($verbosity:ident, $scope:ident, $fmtstr:literal$(, $param:expr)*) => {
        lumberjack::log_internal(::lumberjack::Verbosity::$verbosity, *crate::lumberjack_scopes::$scope, std::file!(), (std::line!(), std::column!()), std::format!($fmtstr$(, $param)*));
    }
}
#[macro_export]
macro_rules! trace {
    ($scope:ident, $fmtstr:literal$(, $param:expr)*) => {
        log_macro_internal!(Trace, $scope, $fmtstr$(, $param)*);
    }
}
#[macro_export]
macro_rules! info {
    ($scope:ident, $fmtstr:literal$(, $param:expr)*) => {
        log_macro_internal!(Info, $scope, $fmtstr$(, $param)*);
    }
}
#[macro_export]
macro_rules! warn {
    ($scope:ident, $fmtstr:literal$(, $param:expr)*) => {
        log_macro_internal!(Warning, $scope, $fmtstr$(, $param)*);
    }
}
#[macro_export]
macro_rules! error {
    ($scope:ident, $fmtstr:literal$(, $param:expr)*) => {
        log_macro_internal!(Error, $scope, $fmtstr$(, $param)*);
    }
}
#[macro_export]
macro_rules! fatal {
    ($scope:ident, $fmtstr:literal$(, $param:expr)*) => {
        log_macro_internal!(Fatal, $scope, $fmtstr$(, $param)*);
        panic!();
    }
}