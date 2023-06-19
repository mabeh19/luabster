use core::mem;
use std::{
    io::{stdout, Write},
    fmt::{Display, Formatter, Result as FmtResult},
    os::unix::io::*, 
    env,
};

use crate::log;
#[cfg(debug_assertions)]
use crate::log::*;
use crate::lua_parser;

use itertools::Itertools;
use strsim;

const LUA_PREFIX: &str = "!";
const STR_SIM_THRESHOLD: f64 = 0.95;


type Command = Vec<String>;
type Commands = Vec<Command>;
type Job = Vec<std::process::Child>;

#[derive(Clone, Debug)]
pub enum Errors {
    Exit,
    NoProgramFound(String),
    FileOverwriteError,
    FileAppendError,
    PipeFailure
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParseError;


enum OutputType {
    NoOutput,
    OverwriteFile(String),
    AppendFile(String),
    NewVariable(String),
    AppendVariable(String)
}

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


pub struct CliParser {
    jobs: Vec<Job>
}

impl CliParser {

    pub fn new() -> Self {
        Self {
            jobs: Vec::new()
        }
    }
    
    pub fn parse_inputs(&mut self, command: &str, lua_parser: &mut lua_parser::LuaParser) -> Result<(), Errors> {
        let should_wait = Self::should_wait(command);
        let mut command = command.to_string();

        if should_wait {
            command.pop(); // Remove final '&' from command
        }

        let mut args: (Commands, Option<Box<dyn Output>>) = Self::parse_input(&command, lua_parser);

        for arg in &args.0 {
            if Self::check_validity_of_program(&arg) == false {
                return Err(Errors::NoProgramFound(arg[0].clone()));
            }
        }

        let mut commands = self.spawn_commands(&args.0, lua_parser);
        
        match Self::execute_commands(&mut commands, &mut args.1) {
            Ok(children) => {
                if !should_wait {
                    Self::wait_for_children_to_finish(children);
                } else {
                    self.jobs.push(children);
                }
            }
            Err(_) => ()
        };
        
        lua_parser.save_vars_to_memory();

        Ok(())
    }

    fn parse_input(command: &str, lua_parser: &mut lua_parser::LuaParser) -> (Commands, Option<Box<dyn Output>>) {
        let mut arguments: Commands = Vec::new();
        let mut output: Option<Box<dyn Output>> = None;
        let mut args_and_output = command.split(">");

        for arg in args_and_output.nth(0).unwrap().split("|") {
            if Self::is_lua_command(arg) {
                arguments.push(vec![arg.to_owned()]);
                continue;
            }
            match Self::parse_command(arg) {
                Ok(cmd) => arguments.push(cmd),
                Err(e) => {
                    println!("{:?}", e); 
                    break;
                }
            };
        }

        #[allow(unused_variables)]
        if let Some(file) = args_and_output.nth(0) {
            log!(LogLevel::Debug, "Creating output {}", file);
            output = Self::create_output(command, lua_parser);
        }

        (arguments, output)
    }

    fn should_wait(command: &str) -> bool {
        command.ends_with("&")
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


    fn is_lua_command(command: &str) -> bool {
        command.starts_with(LUA_PREFIX)
    }

    fn get_output_file(command: &str) -> OutputType {
        let redir: String = command.chars().filter(|c| *c == '>').collect();
        let mut out = OutputType::NoOutput;

        if redir.len() > 0 {
            let out_name = command.split(&redir).nth(1).unwrap().trim();

            if Self::is_lua_command(out_name) {
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

        out
    }

    fn create_output(command: &str, lua_parser: &mut lua_parser::LuaParser) -> Option<Box<dyn Output>> {
        let _output: Option<Box<dyn Output>> = None;

        match Self::get_output_file(command) {
            OutputType::AppendVariable(n) => lua_parser.append_to_variable(&n),
            OutputType::NewVariable(n)    => lua_parser.output_to_variable(&n),       
            OutputType::AppendFile(n)     => Self::append_file(&n).ok(),
            OutputType::OverwriteFile(n)  => Self::overwrite_file(&n).ok(),
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

    fn spawn_commands(&mut self, commands: &Commands, lua_parser: &mut lua_parser::LuaParser) -> Vec<std::process::Command> {
        let mut spawned_commands: Vec<std::process::Command> = Vec::new();

        for cmd in commands {
            if self.check_builtin_command(cmd) == true || lua_parser.parse(&cmd[0]) {
                continue;
            }
            spawned_commands.push(Self::spawn_command(cmd));
        }

        spawned_commands
    }

    fn spawn_command(command: &Vec<String>) -> std::process::Command {
        let mut process = std::process::Command::new(command[0].clone());
        
        if command.len() > 1 {
            process.args(&command[1..]);
        }

        process
    }

    fn pipe_to_output(last_command: &mut std::process::Command, outfile: &mut Option<Box<dyn Output>>) -> Result<(), Errors> {
        
        let file_stdio = outfile.as_mut().unwrap().to_stdio();
        last_command.stdout(file_stdio);
        
        Ok(())
    }

    fn execute_commands(commands: &mut Vec<std::process::Command>, outfile: &mut Option<Box<dyn Output>>) -> Result<Job, std::io::Error> {
        let mut retval: Result<Vec<std::process::Child>, std::io::Error> = Err(std::io::Error::new(std::io::ErrorKind::Other, "No Children"));
        
        if commands.len() > 0 {
            if let Ok((mut children, prev_stdout)) = Self::pipe_children(commands) {
                let last_cmd: &mut std::process::Command = commands.last_mut().unwrap();
                last_cmd.stdin(prev_stdout);
                
                if outfile.is_some() {
                    if let Err(e) = Self::pipe_to_output(last_cmd, outfile) {
                        write!(stdout(), "{:?}", e)?;
                        return retval;
                    }
                } else {
                    last_cmd.stdout(std::process::Stdio::inherit());
                }

                match Self::execute_command(last_cmd) {
                    Ok(last_child) => children.push(last_child),
                    Err(e) => return Err(e)
                };
                retval = Ok(children);
            }
        } 

        retval
    }


    fn pipe_children(commands: &mut Vec<std::process::Command>) -> Result<(Vec<std::process::Child>, std::process::Stdio), std::io::Error> {
        let mut children: Vec<std::process::Child> = Vec::new();
        let mut prev_stdout: std::process::Stdio = std::process::Stdio::inherit();

        for i in 0..(commands.len() - 1) {
            let cmd: &mut std::process::Command = &mut commands[i];
            cmd.stdin(prev_stdout);
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::inherit());

            match Self::execute_command(cmd) {
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
        } else {
            dir = dir.replace("~", &home::home_dir().unwrap().display().to_string());
        }
        if let Err(e) = std::env::set_current_dir(dir) {
            println!("{}\r\n", e);
        }
    }

    fn get_job_index(&mut self, pid: Option<u32>) -> Option<usize> {
        if pid.is_none() {
            return Some(0);
        }
        let pid = pid.unwrap();
        let mut index = 0;
        for children in &self.jobs {
            if children.iter().fold(0, |acc, child| { if child.id() == pid { acc + 1 } else { acc } }) > 0 {
                return Some(index);
            } else {
                index += 1;
            }
        }
        None
    }

    fn fg(&mut self, command: &Command) {
        let pid = if command.len() == 1 { None } else { command[1].parse().ok() };
        if let Some(job_index) = self.get_job_index(pid) {
            let children_to_wait_for = self.jobs.remove(job_index);
            Self::wait_for_children_to_finish(children_to_wait_for);
        }
    }


    fn is_builtin(command: &Command) -> bool {
        command[0].as_str() == "cd" || command[0].as_str() == "fg"
    }


    fn check_builtin_command(&mut self, command: &Command) -> bool {
        let mut is_builtin = true;
        match command[0].as_str() {
            "cd" => Self::cd(command),
            "fg" => self.fg(command),
            _ => {
                is_builtin = false;
            }
        }
        
        is_builtin
    }

    fn execute_command(command: &mut std::process::Command) -> Result<std::process::Child, std::io::Error>{
        log!(LogLevel::Debug, "Executing: {:?}", command);
        
        command.spawn()
    }

    fn wait_for_children_to_finish(children: Vec<std::process::Child>) {
        for cmd in children {
            if let Err(e) = cmd.wait_with_output() {
                println!("{}", e);
            }
        }
    }

    fn command_is_valid(dir: &str, path: &str) -> bool {
        let path_to_check = std::path::Path::new(&format!("{}/{}", dir, path)).to_owned();
        match std::path::Path::try_exists(&path_to_check) {
            Ok(b) => if b { true } else { false },
            Err(e) => {
                println!("{}", e);
                false
            }
        }   
    }

    fn check_validity_of_program(command: &Command) -> bool {

        if Self::is_builtin(command) {
            true
        } else if Self::is_lua_command(&command[0]) {
            true
        } else if Self::command_is_valid(".", &command[0]) {
            true
        } else if Self::command_is_valid("", &command[0]) {
            true
        } else if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(":") {
                if Self::command_is_valid(dir, &command[0]) {
                    return true;
                }
            }

            false
        } else {
            false
        }
    }

    fn check_for_possible_correction_in_dir(dir: &str, inp: &str) -> Option<(String, String)> {

        let dir_path = std::path::Path::new(dir);

        if let Err(e) = std::fs::read_dir(dir_path) {
            println!("{:?}", e);
            return None;
        }

        for f in std::fs::read_dir(dir_path).unwrap().sorted_by(|a, b| a.as_ref().unwrap().file_name().cmp(&b.as_ref().unwrap().file_name())) {
            if let Ok(entry) = f {
                let option = entry.file_name();

                if !inp.starts_with(".") && option.to_string_lossy().starts_with(".") {
                    continue;
                }

                log!(LogLevel::Debug, "Comparing {} to {}", inp, option.to_string_lossy());

                if strsim::jaro_winkler(inp, &option.to_string_lossy()) > STR_SIM_THRESHOLD {
                    return Some((option.to_string_lossy().to_string(), dir.to_string()));
                }
            }
        }

        None
    }

    fn has_possible_correction_in_same_dir(inp: &str) -> Option<(String, String)> {

        if let Ok(cur_dir) = env::current_dir() {
            Self::check_for_possible_correction_in_dir(&cur_dir.to_string_lossy(), inp)
        } else {
            None
        }
    }

    #[allow(non_snake_case)]
    fn has_possible_correction_in_PATH(inp: &str) -> Option<(String, String)> {
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(":") {
                if let Some(opt) = Self::check_for_possible_correction_in_dir(dir, inp) {
                    return Some(opt);
                }
            }
        }

        None
    }


    pub fn get_possible_correction(inp: &str) -> (String, String) {

        if let Some(correction) = Self::has_possible_correction_in_same_dir(inp) {
            correction
        } else if let Some(correction) = Self::has_possible_correction_in_PATH(inp) {
            correction
        } else {
            ("No solution found".to_string(), "file system".to_string())
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
