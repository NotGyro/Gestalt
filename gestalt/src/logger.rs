// The main purpose of this file is to define our Hemlock scopes. 
// It also contains a https://crates.io/crates/log proxy into Hemlock, so anything 
// logged using that crate's macros will show up as coming from the "Library" scope.


#[allow(non_upper_case_globals)]
pub mod hemlock_scopes {
    use hemlock::Scope;
    //use hemlock::Verbosity::*;

    lazy_static! {
        pub static ref Core:     u32 = hemlock::register_scope(Scope::new("Core"));
        // Library is used for the Log crate proxy we're about to define,
        // so anything not logged through Hemlock will show up as being logged by the "Library" scope.
        pub static ref Library:  u32 = hemlock::register_scope(Scope::new("Library"));
        pub static ref Script:   u32 = hemlock::register_scope(Scope::new("Script"));
        pub static ref Game:     u32 = hemlock::register_scope(Scope::new("Game"));
        pub static ref Test:     u32 = hemlock::register_scope(Scope::new("Test"));
        pub static ref Network:  u32 = hemlock::register_scope(Scope::new("Network"));
        pub static ref Renderer: u32 = hemlock::register_scope(Scope::new("Renderer"));
        pub static ref Mesher:   u32 = hemlock::register_scope(Scope::new("Mesher"));
    }
}

pub mod logger {
    extern crate log;
    use log::{Level, LevelFilter, Record, Metadata};
    use std::error::Error;
    struct GameLogger;
    impl log::Log for GameLogger {
        // Always enabled - handles multiple levels.
        fn enabled(&self, _metadata: &Metadata) -> bool { true }
        fn log(&self, record: &Record) {
            match record.level() {
                Level::Error => error!(Library, "{}", record.args()),
                Level::Warn => warn!(Library, "{}", record.args()),
                Level::Info => info!(Library, "{}", record.args()),
                Level::Trace => trace!(Library, "{}", record.args()),
                Level::Debug => trace!(Library, "[Debug] {}", record.args()),
            }
        }
        fn flush(&self) {}
    }

    static GAME_LOGGER : GameLogger = GameLogger;

    pub fn init_logger() -> Result<(), Box<dyn Error>> {
        Ok(log::set_logger(&GAME_LOGGER)
            .map(|()| log::set_max_level(LevelFilter::max()))?)
    }
}