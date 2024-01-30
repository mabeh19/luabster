#![macro_use]
pub static mut LOG_LEVEL: LogLevel = LogLevel::Debug;

#[macro_export]
macro_rules! log {
    ( $level:expr, $( $fmt:expr ),* ) => {
        #[cfg(debug_assertions)]
        unsafe {
            if ($level as usize) >= (LOG_LEVEL as usize) {
                print!("[{}] ", $level);
                println!($( $fmt, )* );
            }
        }
    };
}

#[derive(Clone, Copy, Debug)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
    Fatal = 4
}

pub fn set_loglevel(level: LogLevel) {
    unsafe {
        LOG_LEVEL = level;
    }
} 


impl std::str::FromStr for LogLevel {
    type Err = ();

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_lowercase().as_str() {
            "debug" => Ok(Self::Debug),
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            "fatal" => Ok(Self::Fatal),
            _ => Ok(Self::Info)
        }
    }
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name: &str;
        match self {
            Self::Debug => name = "Debug",
            Self::Info  => name = "Info ",
            Self::Warn  => name = "Warn ",
            Self::Error => name = "Error",
            Self::Fatal => name = "Fatal",
        };
        write!(f, "{}", name)
    }
}
