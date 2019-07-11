extern crate log;
extern crate lazy_static;
extern crate std;
extern crate chrono;
extern crate crossbeam;
extern crate parking_lot;

use log::{SetLoggerError, Level, LevelFilter, Record, Metadata};
//use std::collections::vec_deque::VecDeque;
use parking_lot::Mutex;

use crossbeam::crossbeam_channel::{unbounded, Sender, Receiver}; //iSendError, RecvError, TryRecvError};

pub struct GameLoggerState {
    pub filter_print : LevelFilter,
    pub filter_to_file : LevelFilter, 
    pub filter_game_console : LevelFilter,
    pub console_sender : Sender<String>,
    pub console_receiver : Receiver<String>, 
    /// Used to print current game logic tick, which makes things much more informative.
    pub current_tick : u64,
}

impl GameLoggerState { 
    /// This can also be used to put non-log messages to the game console. 
    /// For example, you probably don't want to log the result of the command
    /// you just typed, however, you probably do want to see it in the console.
    fn push_to_console(&mut self, message : String) {
        self.console_sender.send(message.clone());
    }
}

lazy_static! {
    pub static ref GAME_LOGGER_STATE : Mutex<GameLoggerState> = {
        let (s, r) = unbounded();
        Mutex::new(GameLoggerState { 
            filter_print : LevelFilter::max(),  
            filter_to_file : LevelFilter::max(),
            filter_game_console : LevelFilter::max(),
            console_sender : s, 
            console_receiver : r,
            current_tick : 0,
        })
    };
}

struct GameLogger;
impl GameLogger { 
    /// Used internally, factored out in case we need to use time from a different source. 
    fn time_string(gls : &GameLoggerState) -> String { 
        format!("{} (tick {})", chrono::Local::now().format("%m/%d/%Y %H:%M:%S"), gls.current_tick) 
    }
    fn make_log_entry(gls : &GameLoggerState, record : &Record) -> String { 
        //Commenting out the version with module, too verbose.
        //format!("[{}] [{}] {{{}}} : {} ", GameLogger::time_string(&gls), record.level().to_string(), record.module_path().unwrap_or_default(), record.args())
        format!("[{}] [{}] : {} ", GameLogger::time_string(&gls), record.level().to_string(), record.args())
    }
}
impl log::Log for GameLogger {
    // Always enabled - handles multiple levels.
    fn enabled(&self, metadata: &Metadata) -> bool { true }
    fn log(&self, record: &Record) {
        let mut gls = GAME_LOGGER_STATE.lock(); //.expect("Unable to acquire game logger state mutex while logging a message!");
        let message = GameLogger::make_log_entry(&gls, &record); 
        // Print to stderr, if and only if this is at error level.
        if record.level() == Level::Error {
            eprintln!("{}", message); 
        }
        // Print to stdout
        else if record.level() <= gls.filter_print {
            println!("{}", message);
        }
        
        // TODO: Write to a log file.
        // gls.filter_to_file
        // Put messages to the tilde-accessible game console. 
        // (Rendered by polling (gls.game_console_log).
        if record.level() <= gls.filter_game_console {
            gls.push_to_console(message.clone());
        }
        drop(gls);
    }
    fn flush(&self) {}
}

static GAME_LOGGER : GameLogger = GameLogger;

pub fn init_logger() -> Result<(), SetLoggerError> {
    log::set_logger(&GAME_LOGGER)
        .map(|()| log::set_max_level(LevelFilter::max()))
}
