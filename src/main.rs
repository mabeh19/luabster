use core::mem;
use std::{
    process,
    io::{self, Write},
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
};


type Command = Vec<String>;
type Commands = Vec<Command>;

macro_rules! log {
    ( $level:expr, $( $fmt:expr ),* ) => {
        if ($level as usize) >= (LOG_LEVEL as usize) {
            print!("[{}] ", $level);
            println!( $( $fmt, )* );
        }
    };
}

static LOG_LEVEL: LogLevel = LogLevel::Debug;
const WELCOME_MSG: &str = "
    Hello, and welcome to 🦞 LAUBSTER 🦞
";

const PROMPT: &str = "@hackerman >> ";

fn main() -> Result<(), Box<dyn Error>> {
    println!("{}", WELCOME_MSG);

    loop {
        display_prompt();

        let command = get_input();

        log!(LogLevel::Debug, "Input received: {}", command);

        match check_quit(&command) {
            Err(e) => {
                break;
            },
            Ok(()) => {

            }
        };

        let args: (Commands, Option<Box<dyn Output>>) = parse_input(&command);

        let mut commands = spawn_commands(&args.0);

        pipe_commands(&mut commands);

        let mut children = execute_commands(&mut commands);
        
        if children.is_ok() {
            wait_for_children_to_finish(children.unwrap());
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

fn check_quit(input: &str) -> Result<(), Errors> {
    if input == "exit" {
        Err(Errors::Exit) 
    } else {
        Ok(())
    }
}

fn parse_input(command: &str) -> (Vec<Vec<String>>, Option<Box<dyn Output>>) {
    let mut arguments: Commands = Vec::new();
    let mut output: Option<Box<dyn Output>> = None;
    for arg in command.split("|") {
        match parse_command(arg) {
            Ok(cmd) => arguments.push(cmd),
            Err(e) => {
                println!("{:?}", e); 
                break;
            }
        };
    }

    if let Some(_index) = command.find(">") {
        output = Some(create_output(command));
    }

    return (arguments, output);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError;

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        f.write_str("missing closing quote")
    }
}

impl std::error::Error for ParseError {}

enum State {
    /// Within a delimiter.
    Delimiter,
    /// After backslash, but before starting word.
    Backslash,
    /// Within an unquoted word.
    Unquoted,
    /// After backslash in an unquoted word.
    UnquotedBackslash,
    /// Within a single quoted word.
    SingleQuoted,
    /// Within a double quoted word.
    DoubleQuoted,
    /// After backslash inside a double quoted word.
    DoubleQuotedBackslash,
}

fn parse_command(command: &str) -> Result<Vec<String>, ParseError> {
    use State::*;

    let mut words = Vec::new();
    let mut word = String::new();
    let mut chars = command.chars();
    let mut state = Delimiter;

    loop {
        let c = chars.next();
        state = match state {
            Delimiter => match c {
                None => break,
                Some('\'') => SingleQuoted,
                Some('\"') => DoubleQuoted,
                Some('\\') => Backslash,
                Some('\t') | Some(' ') | Some('\n') => Delimiter,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            Backslash => match c {
                None => {
                    word.push('\\');
                    words.push(mem::take(&mut word));
                    break;
                }
                Some('\n') => Delimiter,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            Unquoted => match c {
                None => {
                    words.push(mem::take(&mut word));
                    break;
                }
                Some('\'') => SingleQuoted,
                Some('\"') => DoubleQuoted,
                Some('\\') => UnquotedBackslash,
                Some('\t') | Some(' ') | Some('\n') => {
                    words.push(mem::take(&mut word));
                    Delimiter
                }
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            UnquotedBackslash => match c {
                None => {
                    word.push('\\');
                    words.push(mem::take(&mut word));
                    break;
                }
                Some('\n') => Unquoted,
                Some(c) => {
                    word.push(c);
                    Unquoted
                }
            },
            SingleQuoted => match c {
                None => return Err(ParseError),
                Some('\'') => Unquoted,
                Some(c) => {
                    word.push(c);
                    SingleQuoted
                }
            },
            DoubleQuoted => match c {
                None => return Err(ParseError),
                Some('\"') => Unquoted,
                Some('\\') => DoubleQuotedBackslash,
                Some(c) => {
                    word.push(c);
                    DoubleQuoted
                }
            },
            DoubleQuotedBackslash => match c {
                None => return Err(ParseError),
                Some('\n') => DoubleQuoted,
                Some(c @ '$') | Some(c @ '`') | Some(c @ '"') | Some(c @ '\\') => {
                    word.push(c);
                    DoubleQuoted
                }
                Some(c) => {
                    word.push('\\');
                    word.push(c);
                    DoubleQuoted
                }
            },
        }
    }

    Ok(words)
}


//fn parse_command(command: &str) -> Vec<String> {
//    command.split(" ").collect()
//}

fn create_output(command: &str) -> Box<dyn Output> {
    todo!();
    
}

fn spawn_commands(commands: &Commands) -> Vec<std::process::Command> {
    let mut spawned_commands: Vec<std::process::Command> = Vec::new();

    for cmd in commands {
        spawned_commands.push(spawn_command(cmd));
    }

    return spawned_commands;
}

fn spawn_command(command: &Vec<String>) -> std::process::Command {
    let mut process = std::process::Command::new(command[0].clone());
    
    if command.len() > 1 {
        process.args(&command[1..]);
    }

    return process;
}

fn pipe_commands(commands: &mut Vec<std::process::Command>) {
    if commands.len() > 1 {
        todo!();
    }
}

fn execute_commands(commands: &mut Vec<std::process::Command>) -> Result<Vec<std::process::Child>, std::io::Error> {
    let mut retval: Result<Vec<std::process::Child>, std::io::Error> = Err(std::io::Error::new(std::io::ErrorKind::Other, "???"));
    let mut children: Vec<std::process::Child> = Vec::new();
    let mut error_encountered = false;

    for cmd in commands {
        match execute_command(cmd) {
            Ok(child) => children.push(child),
            Err(e)    => {
                retval = Err(e); 
                error_encountered = true; 
                
                /* If an error is encountered, stop early */
                break;
            }
        };
    }

    if error_encountered == false {
        retval = Ok(children);
    }

    return retval;
}

fn execute_command(command: &mut std::process::Command) -> Result<std::process::Child, std::io::Error>{
    log!(LogLevel::Debug, "Executing: {:?}", command);
    
    return command.spawn();
}

fn wait_for_children_to_finish(children: Vec<std::process::Child>) {
    for cmd in children {
        cmd.wait_with_output();
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

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name: &str;
        match self {
            Self::Debug => name = "Debug",
            Self::Info  => name = "Info ",
            Self::Warn  => name = "Warn ",
            Self::Error => name = "Error",
            Self::Fatal => name = "Fatal",
            _           => name = "?????"
        };
        write!(f, "{}", name)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Errors {
    Exit,
    NoProgramFound,
}

pub trait Output {

}
