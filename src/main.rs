use std::{
    process,
    io::{self, Write},
    error::Error,
};

macro_rules! log {
    ( $level:expr, $( $fmt:expr ),* ) => {
        if ($level as usize) >= (LOG_LEVEL as usize) {
            print!("[{:?}]", $level);
            println!( $( $fmt, )* );
        }
    };
}

static LOG_LEVEL: LogLevel = LogLevel::Debug;
const WELCOME_MSG: &str = "
    Hello, and welcome to SHELLY
";

const PROMPT: &str = "@hackerman >> ";

fn main() -> Result<(), Box<dyn Error>> {
    println!("{}", WELCOME_MSG);

    loop {
        display_prompt();

        let command = get_input();

        log!(LogLevel::Info, "Input received: {}", command);

        match checkQuit(&command) {
            Err(e) => {
                break;
            },
            Ok(()) => {

            }
        };

        let args: [[&str]] = parse_input(&command);

        for cmd in args {
            execute_command(&cmd);
        }
    }

    Ok(())
}

fn display_prompt() {
    print!("{}", PROMPT);
    io::stdout().flush();
}

fn get_input() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input);
    input = input.trim().to_string();
    return input;
}

fn checkQuit(input: &str) -> Result<(), Errors> {
    if input == "exit" {
        Err(Errors::Exit) 
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LogLevel {
    Debug = 0,
    Info = 1,
    Warn = 2,
    Error = 3,
    Fatal = 4
}

#[derive(Clone, Copy, Debug)]
pub enum Errors {
    Exit,
    NoProgramFound,
}
