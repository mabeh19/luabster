use core::mem;
use std::{
    io::{stdout, Write},
    fmt::{Display, Formatter, Result as FmtResult}
};

use crate::log;
use crate::log::*;
use crate::lua_parser;

type Command = Vec<String>;
type Commands = Vec<Command>;

const LUA_PREFIX: &str = "!";


#[derive(Clone, Copy, Debug)]
pub enum Errors {
    Exit,
    NoProgramFound,
    FileOverwriteError,
    FileAppendError,
    PipeFailure
}

pub fn parse_inputs(command: &str, lua_parser: &mut lua_parser::LuaParser) {
    let mut args: (Commands, Option<Box<dyn Output>>) = parse_input(command, lua_parser);

    let mut commands = spawn_commands(&args.0, lua_parser);
    
    match execute_commands(&mut commands, &mut args.1) {
        Ok( children) => wait_for_children_to_finish(children),
        Err(e) => drop(e)//println!("{:?}", e)
    };
    
    lua_parser.save_vars_to_memory();
}

fn parse_input(command: &str, lua_parser: &mut lua_parser::LuaParser) -> (Vec<Vec<String>>, Option<Box<dyn Output>>) {
    let mut arguments: Commands = Vec::new();
    let mut output: Option<Box<dyn Output>> = None;
    let mut args_and_output = command.split(">");

    for arg in args_and_output.nth(0).unwrap().split("|") {
        if is_lua_command(arg) {
            arguments.push(vec![arg.to_owned()]);
            continue;
        }
        match parse_command(arg) {
            Ok(cmd) => arguments.push(cmd),
            Err(e) => {
                println!("{:?}", e); 
                break;
            }
        };
    }

    if let Some(file) = args_and_output.nth(0) {
        log!(LogLevel::Debug, "Creating output {}", file);
        output = create_output(command, lua_parser);
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

enum OutputType {
    NoOutput,
    OverwriteFile(String),
    AppendFile(String),
    NewVariable(String),
    AppendVariable(String)
}

fn is_lua_command(command: &str) -> bool {
    command.starts_with(LUA_PREFIX)
}

fn get_output_file(command: &str) -> OutputType {
    let redir: String = command.chars().filter(|c| *c == '>').collect();
    let mut out = OutputType::NoOutput;

    if redir.len() > 0 {
        let out_name = command.split(&redir).nth(1).unwrap().trim();

        if is_lua_command(out_name) {
            if redir.len() == 2 {
                out = OutputType::AppendVariable(out_name.to_string());    
            } else {
                out = OutputType::NewVariable(out_name.to_string());
            }
        } else {
            if redir.len() == 2 {
                out = OutputType::AppendFile(out_name.to_string());
            } else {
                out = OutputType::OverwriteFile(out_name.to_string());
            }
        }
    }

    return out;
}

fn create_output(command: &str, lua_parser: &mut lua_parser::LuaParser) -> Option<Box<dyn Output>> {
    let _output: Option<Box<dyn Output>> = None;

    match get_output_file(command) {
        OutputType::AppendVariable(n) => lua_parser.append_to_variable(&n),
        OutputType::NewVariable(n)    => lua_parser.output_to_variable(&n),       
        OutputType::AppendFile(n)     => append_file(&n).ok(),
        OutputType::OverwriteFile(n)  => overwrite_file(&n).ok(),
        OutputType::NoOutput          => None
    }
}

fn overwrite_file(command: &str) -> Result<Box<dyn Output>, Errors> {
    let mut file_name = command.split(">");
    let file = OutFile::new(file_name.nth(1).unwrap().trim());

    match file {
        Ok(f) => Ok(f),
        Err(_) => Err(Errors::FileOverwriteError)
    }
}

fn append_file(command: &str) -> Result<Box<dyn Output>, Errors> {
    let mut file_name = command.split(">>");
    let file = OutFile::open(file_name.nth(1).unwrap().trim());

    match file {
        Ok(f) => Ok(f),
        Err(_) => Err(Errors::FileAppendError)
    }
}

fn spawn_commands(commands: &Commands, lua_parser: &mut lua_parser::LuaParser) -> Vec<std::process::Command> {
    let mut spawned_commands: Vec<std::process::Command> = Vec::new();

    for cmd in commands {
        if check_builtin_command(cmd) == true || lua_parser.parse(&cmd[0]) {
            continue;
        }
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

fn pipe_to_output(last_command: &mut std::process::Command, outfile: &mut Option<Box<dyn Output>>) -> Result<(), Errors> {
    
    let file_stdio = outfile.as_mut().unwrap().to_stdio();
    last_command.stdout(file_stdio);
    
    Ok(())
}

fn execute_commands(commands: &mut Vec<std::process::Command>, outfile: &mut Option<Box<dyn Output>>) -> Result<Vec<std::process::Child>, std::io::Error> {
    let mut retval: Result<Vec<std::process::Child>, std::io::Error> = Err(std::io::Error::new(std::io::ErrorKind::Other, "No Children"));
    
    if commands.len() > 0 {
        if let Ok((mut children, prev_stdout)) = pipe_children(commands) {
            let last_cmd: &mut std::process::Command = commands.last_mut().unwrap();
            last_cmd.stdin(prev_stdout);
            
            if outfile.is_some() {
                if let Err(e) = pipe_to_output(last_cmd, outfile) {
                    write!(stdout(), "{:?}", e)?;
                    return retval;
                }
            } else {
                last_cmd.stdout(std::process::Stdio::inherit());
            }

            match execute_command(last_cmd) {
                Ok(last_child) => children.push(last_child),
                Err(e) => return Err(e)
            };
            retval = Ok(children);
        }
    } 

    return retval;
}

use std::os::unix::io::*;

fn pipe_children(commands: &mut Vec<std::process::Command>) -> Result<(Vec<std::process::Child>, std::process::Stdio), std::io::Error> {
    let mut children: Vec<std::process::Child> = Vec::new();
    let mut prev_stdout: std::process::Stdio = std::process::Stdio::inherit();

    for i in 0..(commands.len() - 1) {
        let cmd: &mut std::process::Command = &mut commands[i];
        cmd.stdin(prev_stdout);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::inherit());

        match execute_command(cmd) {
            Ok(mut child) => {
                let stdout: std::os::unix::io::RawFd = child.stdout.take().unwrap().into_raw_fd();
                children.push(child);
                unsafe {
                    prev_stdout = std::process::Stdio::from_raw_fd(stdout);
                }
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
    if let Err(e) = std::env::set_current_dir(dir) {
        println!("{}\r\n", e);
    }
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
        if let Err(e) = cmd.wait_with_output() {
            println!("{}", e);
        }
    }
}


pub trait Output {
    fn to_stdio(&mut self) -> std::process::Stdio;
    fn close(self);
}

struct OutFile {
    file: std::fs::File
}

impl OutFile {
    pub fn new(file_name: &str) -> std::io::Result<Box<Self>>  {
        let file = std::fs::File::create(file_name)?;
        Ok(Box::new(Self { file }))
    }

    pub fn open(file_name: &str) -> std::io::Result<Box<Self>> {
        let file = std::fs::OpenOptions::new().create(true).append(true).open(file_name)?;
        Ok(Box::new(Self { file }))
    }
}

impl Output for OutFile {
    fn to_stdio(&mut self) -> std::process::Stdio {
        std::process::Stdio::from(self.file.try_clone().unwrap())
    }

    fn close(self) {
        
    }
}
