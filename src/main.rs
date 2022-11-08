use core::mem;
use std::{
    process,
    io::{self, Write},
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
};

use home;
use hostname;

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

const LUA_PREFIX: &str = "!";

const PROMPT: &str = "@hackerman";

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

        let mut args: (Commands, Option<Box<dyn Output>>) = parse_input(&command);

        let mut commands = spawn_commands(&args.0);

        //pipe_commands(&mut commands, &mut args.1);

        let mut children = execute_commands(&mut commands);
        
        if children.is_ok() {
            wait_for_children_to_finish(children.unwrap());
        }
    }

    Ok(())
}

fn display_prompt() {
    const username_key: &str = "USER";

    if let Ok(cur_dir) = std::env::current_dir() {
        if let Ok(prompt) = hostname::get() {
            print!("[{}@{}]: [{}] >> ", std::env::var(username_key).unwrap(), prompt.into_string().unwrap(), cur_dir.display());
        } else {
            print!("[{}] {} >> ", PROMPT, cur_dir.display());
        }  
        io::stdout().flush();
    } else {
        print!("[{}] ??? >> ", PROMPT);
        io::stdout().flush();
    }
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
    let mut args_and_output = command.split(">");

    for arg in args_and_output.nth(0).unwrap().split("|") {
        match parse_command(arg) {
            Ok(cmd) => arguments.push(cmd),
            Err(e) => {
                println!("{:?}", e); 
                break;
            }
        };
    }

    if let Some(file) = args_and_output.nth(0) {
        log!(LogLevel::Debug, "Creating output {}\n", file);
        output = create_output(command);
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

fn create_output(command: &str) -> Option<Box<dyn Output>> {
    let mut output: Option<Box<dyn Output>>;
    if command.starts_with(LUA_PREFIX) {
        // LUA
        todo!();
    } else {
        if let Some(_index) = command.find(">>") {
            // If we're appending
            todo!();
        } else {
            // If we're overwriting
            output = overwrite_file(command).ok();
        }
    }

    return output;
}

fn overwrite_file(command: &str) -> Result<Box<dyn Output>, Errors> {
    let mut file_name = command.split(">");
    let file = OutFile::new(file_name.nth(1).unwrap().trim());

    match file {
        Ok(f) => Ok(f),
        Err(e) => Err(Errors::FileOverwriteError)
    }
}

fn spawn_commands(commands: &Commands) -> Vec<std::process::Command> {
    let mut spawned_commands: Vec<std::process::Command> = Vec::new();

    for cmd in commands {
        if check_builtin_command(cmd) == false {
            spawned_commands.push(spawn_command(cmd));
        }
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

fn pipe_commands(commands: &mut Vec<std::process::Command>, outfile: &mut Option<Box<dyn Output>>) -> Result<(), Errors> {
//    if commands.len() > 1 {
//        do_piping(commands)?;
//    }

    if outfile.is_some() {
        pipe_to_output(commands, outfile)?;
    }

    Ok(())
}

//fn pipe_and_execute(commands: &mut Vec<std::process::Child>) -> Result<(), Errors> {
//
//    let mut prev_stdout: std::process::ChildStdout = std::process::inherit();
//    for cmd in commands {
//        cmd.stdin.set(prev_stdout);
//        prev_stdout = cmd.stdout.take().unwrap();
//    }
//
//    Ok(())
//}

fn pipe_to_output(commands: &mut Vec<std::process::Command>, outfile: &mut Option<Box<dyn Output>>) -> Result<(), Errors> {
    if let Some(last_cmd) = commands.last_mut() {
        let mut file_stdio = outfile.as_mut().unwrap().to_stdio();
        last_cmd.stdout(file_stdio);
    }

    Ok(())
}

fn execute_commands(commands: &mut Vec<std::process::Command>) -> Result<Vec<std::process::Child>, std::io::Error> {
    let mut retval: Result<Vec<std::process::Child>, std::io::Error> = Err(std::io::Error::new(std::io::ErrorKind::Other, "???"));

    if let Ok((mut children, prev_stdout)) = pipe_children(commands) {
        let mut last_cmd: &mut std::process::Command = commands.last_mut().unwrap();
        last_cmd.stdin(prev_stdout);
        last_cmd.stdout(std::process::Stdio::inherit());
        match execute_command(last_cmd) {
            Ok(mut last_child) => children.push(last_child),
            Err(e) => return Err(e)
        };
        retval = Ok(children);
    }

    return retval;
}

fn pipe_children(commands: &mut Vec<std::process::Command>) -> Result<(Vec<std::process::Child>, std::process::Stdio), std::io::Error> {
    let mut children: Vec<std::process::Child> = Vec::new();
    let mut prev_stdout: std::process::Stdio = std::process::Stdio::inherit();
    
    for i in 0..(commands.len() - 1) {
        let mut cmd: &mut std::process::Command = &mut commands[i];
        cmd.stdin(prev_stdout);
        cmd.stdout(std::process::Stdio::piped());
        
        match execute_command(cmd) {
            Ok(child) => {
                prev_stdout = child.stdout.unwrap().into();
                children.push(child);
            },
            Err(e) => {
                return Err(e); 
            }
        };
    }
    
    Ok((children, prev_stdout))
}

fn cd(command: &Command) {
    let mut dir: String = "~".to_string();
    if command.len() > 1 {
        dir = command[1].clone();
    }
    if dir == "~" {
        match home::home_dir() {
            Some(p) => dir = p.display().to_string(),
            None => println!("Home directory not found")
        };
    }
    std::env::set_current_dir(dir);
}

fn check_builtin_command(command: &Command) -> bool {
    let mut is_builtin = true;
    match command[0].as_str() {
        "cd" => {
            cd(command); 
        },
        _ => {
            is_builtin = false;
        }
    }
    
    return is_builtin;
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
    FileOverwriteError,
    FileAppendError,
    PipeFailure
}

pub trait Output {
    fn to_stdio(&mut self) -> std::process::Stdio;
}

struct OutFile {
    file: std::fs::File
}

impl OutFile {
    pub fn new(file_name: &str) -> std::io::Result<Box<Self>>  {
        let file = std::fs::File::create(file_name)?;
        Ok(Box::new(Self { file }))
    }
}

impl Output for OutFile {
    fn to_stdio(&mut self) -> std::process::Stdio {
        std::process::Stdio::from(self.file.try_clone().unwrap())
    }
}
