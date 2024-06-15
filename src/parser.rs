use core::mem;
use std::{
    fmt::{Display, Formatter, Result as FmtResult},
    os::unix::io::*,
    env,
    collections::HashMap,
};


#[cfg(debug_assertions)]
use crate::log::*;

use crate::{
    log,
    lua_parser,
    config,
    input_parser,
    prompt,
    config::Configurable,
    expand,
};

use itertools::Itertools;
use strsim;


type Command = Vec<String>;
type Commands = Vec<Command>;
//type Job = Vec<ChildProcess>;
type Job = Vec<i32>;
pub type BuiltInFunctionHandler<'a> = fn(&mut CliParser<'a>, &Command);

#[derive(Clone, Debug)]
pub enum Errors {
    Exit,
    NoProgramFound(String),
    FileOverwriteError,
    FileAppendError,
    PipeFailure
}

#[derive(Debug)]
enum ChildCommand {
    Bash(std::process::Command),
    Lua(Child)
}

#[derive(Debug)]
enum ChildProcess {
    Bash(std::process::Child),
    Lua(Child),
}


const PIPE_READ: usize = 0;
const PIPE_WRITE: usize = 1;

#[repr(C)]
#[derive(Debug)]
pub struct Child {
    pub pid: i32,
    pub stdin: [i32; 2],
    pub stdout: [i32; 2],
    pub stderr: [i32; 2],
    pub cmd: *const std::ffi::c_uchar,
    pub is_first: i32,
    pub is_last: i32,
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

pub struct CliParser<'a> {
    jobs: Vec<Job>,
    builtin_handlers: HashMap<&'a str, BuiltInFunctionHandler<'a>>,
    aliases: HashMap<String, String>,
    cur_job: Option<usize>,
    lua_parser: lua_parser::LuaParser,
    should_wait: bool,
    pub input_parser: input_parser::InputParser,
    pub prompt: prompt::Prompt<'a>,
    children: std::collections::HashMap<i32, std::process::Child>,
}

extern "C" {
    fn sig_kill(pid: u32, sig: i32);
    fn signal_is_stopped(pids: *const u32, num_pids: u32) -> bool;
    fn lua_runner_run_command(l: *mut std::ffi::c_void, c: *mut Child) -> Child;
    fn try_wait_process(pid: u32) -> i32;
    fn enter_critical_section();
    fn exit_critical_section();
    static sig_CONT: i32;
    static PROCESS_EXITED: i32;
    static PROCESS_STOPPED: i32;
    static PROCESS_RUNNING: i32;
}

const LUA_PREFIX: &str = "!";
const STR_SIM_THRESHOLD: f64 = 0.95;


impl<'a: 'b, 'b, 'c> config::ConfigurationLoader<'a, 'b> for CliParser<'c> {
    fn load_config(&self, params: &[&'a str]) -> HashMap<&'b str, String> {
        self.lua_parser.load_config(params, &home::home_dir().unwrap().to_string_lossy())
    }
}


impl<'a> CliParser<'a> {
    const BUILTIN_COMMANDS: [(&'static str, BuiltInFunctionHandler<'a>); 9] = [
        ("exit", Self::exit),
        ("cd", Self::cd),
        ("fg", Self::fg),
        ("bg", Self::bg),
        ("alias", Self::alias),
        ("source", Self::source),
        ("export", Self::export),
        ("eval", Self::eval),
        ("luabster_update", Self::update_config),
    ];

    pub fn get_builtin_commands() -> Vec<&'static str> {
        Self::BUILTIN_COMMANDS.iter().map(|(n,_)| *n).collect()
    }

    pub fn new(home_dir: &str) -> Self {
        let mut parser = Self {
            jobs: Vec::new(),
            builtin_handlers: HashMap::new(),
            aliases: HashMap::new(),
            cur_job: None,
            lua_parser: lua_parser::LuaParser::init(home_dir),
            should_wait: false,
            input_parser: input_parser::InputParser::new(home_dir),
            prompt: prompt::Prompt::new(),
            children: HashMap::new(),
        };

        for (n, f) in Self::BUILTIN_COMMANDS {
            parser.bind_builtin_command(n, f);
        }

        parser.update_config(&Vec::new());

        parser
    }

    pub fn bind_builtin_command(&mut self, command: &'a str, handler: BuiltInFunctionHandler<'a>) {
        self.builtin_handlers.insert(command, handler);
    }

    pub fn read_config<'b>(&mut self, params: &[&'b str], home_dir: &str) -> HashMap<&'b str, String> {
        self.lua_parser.load_config(params, home_dir)
    }

    fn configure(&mut self) {
        let mut new_prompt = self.prompt.clone();
        let mut new_input_parser = self.input_parser.clone();
        let mut lua_scripts = self.lua_parser.scripts.clone();
        
        let mut configurables = [
            &mut new_prompt as &mut dyn Configurable,
            &mut new_input_parser as &mut dyn Configurable,
            &mut crate::termio::Termio as &mut dyn Configurable,
            &mut lua_scripts as &mut dyn Configurable,
        ];

        config::configure(&mut configurables, self);

        self.prompt = new_prompt;
        self.input_parser = new_input_parser;
        self.lua_parser.scripts = lua_scripts;
    }

    pub fn parse_inputs(&mut self, command: &str) -> Result<(), Errors> {
        if command.is_empty() {
            return Ok(());
        }
        let run_in_bg = Self::run_in_bg(command);
        let mut command = Self::expand_string(command);

        if run_in_bg {
            command.pop(); // Remove final '&' from command
        }
        self.should_wait = !run_in_bg;

        for block in command.split(";") {
            for cmd in block.split("&&") {

                let mut args: (Commands, Option<Box<dyn Output>>) = self.parse_input(&cmd);

                for arg in &args.0 {
                    if Self::check_validity_of_program(&arg) == false {
                        return Err(Errors::NoProgramFound(arg[0].clone()));
                    }
                }

                let mut commands = self.spawn_commands(&args.0);

                unsafe { enter_critical_section(); }
                match self.execute_commands(&mut commands, &mut args.1) {
                    Ok(children) => {
                        self.jobs.push(children);
                        if self.should_wait {
                            self.cur_job = Some(self.jobs.len() - 1);
                            unsafe { exit_critical_section(); }
                            self.wait_for_children_to_finish();
                        } else {
                            unsafe { exit_critical_section(); }
                        }
                    }
                    Err(_) => unsafe { exit_critical_section(); }
                };
            };
        }

        self.lua_parser.save_vars_to_memory();

        Ok(())
    }

    fn parse_input(&mut self, command: &str) -> (Commands, Option<Box<dyn Output>>) {
        let mut arguments: Commands = Vec::new();
        let mut output: Option<Box<dyn Output>> = None;
        let cmds = command.split("|").collect::<Vec<&str>>();
        let (_, last_cmd) = cmds.split_at(cmds.len()-1);
        let last_cmd = last_cmd[0];
        let mut out_file = None;

        for arg in cmds {
            let arg = arg.trim();
            if Self::is_lua_command(arg) {
                arguments.push(vec![arg.to_owned()]);
                continue;
            }
            match Self::parse_command(arg) {
                Ok(mut cmd) => {
                    // expand arguments
                    cmd = cmd.iter_mut().map(|a| crate::expand::expand_all(a)).collect();
                    if arg == last_cmd && cmd.len() > 1 {
                        match cmd[cmd.len() - 2].as_str() {
                            ">" | ">>" => {
                                out_file = Some(cmd.last().unwrap().to_owned());
                                _ = cmd.split_off(cmd.len() - 2);
                            },
                            _ => {}
                        }
                    }
                    if let Some(a) = self.aliases.get(&cmd[0]) {
                        if let Ok(mut exp_cmd) = Self::parse_command(a) {
                            exp_cmd.append(&mut cmd.into_iter().dropping(1).collect());
                            cmd = exp_cmd;
                        }
                    }
                    arguments.push(cmd);
                },
                Err(e) => {
                    println!("{:?}", e); 
                    break;
                }
            };
        }

        if let Some(file) = out_file {
            log!(LogLevel::Debug, "Creating output {}", file);
            output = Self::create_output(command, &mut self.lua_parser);
        }

        (arguments, output)
    }

    fn run_in_bg(command: &str) -> bool {
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
            OutputType::AppendVariable(n) => lua_parser.append_to_variable(&n.trim()),
            OutputType::NewVariable(n)    => lua_parser.output_to_variable(&n.trim()),       
            OutputType::AppendFile(n)     => Self::append_file(&n.trim()).ok(),
            OutputType::OverwriteFile(n)  => Self::overwrite_file(&n.trim()).ok(),
            OutputType::NoOutput          => None
        }
    }

    fn overwrite_file(file_name: &str) -> Result<Box<dyn Output>, Errors> {
        let file = OutFile::new(file_name);

        match file {
            Ok(f) => Ok(f),
            Err(_) => Err(Errors::FileOverwriteError)
        }
    }

    fn append_file(file_name: &str) -> Result<Box<dyn Output>, Errors> {
        let file = OutFile::open(file_name);

        match file {
            Ok(f) => Ok(f),
            Err(_) => Err(Errors::FileAppendError)
        }
    }

    fn spawn_commands(&mut self, commands: &Commands) -> Vec<ChildCommand> {
        let mut spawned_commands: Vec<ChildCommand> = Vec::new();

        for (i, cmd) in commands.iter().enumerate() {
            let first = i == 0;
            let last = i == commands.len() - 1;
            if self.check_builtin_command(cmd) == true {
                continue;
            }
            if let Some(c) = self.lua_parser.parse(&cmd[0], first, last) {
                spawned_commands.push(ChildCommand::Lua(c));
            }
            else {
                spawned_commands.push(ChildCommand::Bash(Self::spawn_command(cmd)));
            }
        }

        spawned_commands
    }

    fn spawn_command(command: &Vec<String>) -> std::process::Command {
        let mut process = std::process::Command::new(&command[0]);
        
        if command.len() > 1 {
            process.args(&command[1..]);
        }

        process
    }

    fn pipe_to_output(last_command: &mut ChildCommand, outfile: &mut Option<Box<dyn Output>>) -> Result<(), Errors> {
        
        match last_command {
            ChildCommand::Bash(last_command) => {    
                let file_stdio = outfile.as_mut().unwrap().to_stdio();
                last_command.stdout(file_stdio);
            },
            ChildCommand::Lua(last_command) => {
                last_command.stdout[PIPE_READ] = outfile.as_mut().unwrap().to_fd();
            }
        }

        Ok(())
    }

    fn execute_commands(&mut self, commands: &mut Vec<ChildCommand>, outfile: &mut Option<Box<dyn Output>>) -> Result<Job, std::io::Error> {
        let mut retval: Result<Job, std::io::Error> = Err(std::io::Error::new(std::io::ErrorKind::Other, "No Children"));

        if commands.len() > 0 {
            let (mut children, prev_stdout) = self.pipe_children(commands)?;
            let last_cmd: &mut ChildCommand = commands.last_mut().unwrap();

            match last_cmd {
                ChildCommand::Bash(last_cmd) => {
                    if let Some(stdout) = prev_stdout {
                        unsafe { last_cmd.stdin(std::process::Stdio::from_raw_fd(stdout)); }
                    } else {
                        last_cmd.stdin(std::process::Stdio::inherit());
                    }
                },
                ChildCommand::Lua(last_cmd)  => {
                    if let Some(stdout) = prev_stdout {
                        last_cmd.stdin[PIPE_READ] = stdout;
                    }
                }
            }

            if outfile.is_some() {
                if let Err(e) = Self::pipe_to_output(last_cmd, outfile) {
                    println!("{:?}", e);
                    return retval;
                }
            } else {
                match last_cmd {
                    ChildCommand::Bash(last_cmd) => {last_cmd.stdout(std::process::Stdio::inherit());},
                    ChildCommand::Lua(last_cmd)  => ()
                }
            }

            match self.execute_command(last_cmd) {
                Ok(last_child) => children.push(last_child.get_pid()),
                Err(e) => return Err(e)
            };
            retval = Ok(children); 
        } 

        retval
    }


    fn pipe_children(&mut self, commands: &mut Vec<ChildCommand>) -> Result<(Vec<i32>, Option<i32>), std::io::Error> {
        let mut children = Vec::new();
        let mut prev_stdout = None;

        for i in 0..(commands.len() - 1) {
            let cmd = &mut commands[i];
            match cmd {
                ChildCommand::Bash(cmd) => {
                    if let Some(prev_stdout) = prev_stdout {
                        unsafe {
                            cmd.stdin(std::process::Stdio::from_raw_fd(prev_stdout));
                        }
                    } else {
                        //cmd.stdin(std::process::Stdio::inherit());
                    }
                    cmd.stdout(std::process::Stdio::piped());
                    cmd.stderr(std::process::Stdio::inherit());
                },
                ChildCommand::Lua(c) => {
                    if let Some(prev_stdout) = prev_stdout {
                        c.stdin[PIPE_READ] = prev_stdout;
                    }
                }
            }

            match self.execute_command(cmd) {
                Ok(child) => {
                    // The repeat circumvents moving issues
                    match child {
                        ChildProcess::Bash(mut child) => {
                            // Extra steps are performed to avoid premature closing of pipes
                            let stdout = child.stdout.take().unwrap();
                            let stdout_fd = stdout.as_raw_fd();
                            child.stdout = Some(stdout);
                            children.push(child.id() as i32);
                            prev_stdout = Some(stdout_fd);
                            self.children.insert(child.id() as i32, child);
                        },
                        ChildProcess::Lua(child) => {
                            let stdout = child.stdout[PIPE_READ];
                            children.push(child.pid);
                            prev_stdout = Some(stdout);
                        }
                    }
                },
                Err(e) => {
                    return Err(e); 
                }
            }
        }

        Ok((children, prev_stdout))
    }

    fn cd(self: &mut Self, command: &Command) {
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

    fn exit(&mut self, _command: &Command) {

    }

    fn alias(&mut self, command: &Command) {
        if command.len() != 2 {
            return;
        }
        if let Some(idx) = command[1].find('=') {
            let (name, cmd) = command[1].split_at(idx);
            self.aliases.insert(name.to_string(), cmd[1..].to_string());
        }
    }

    fn source(&mut self, command: &Command) {
        for cmd in &command[1..] {
            match std::fs::read_to_string(cmd) {
                Ok(s) => {
                    for line in s.lines() {
                        _ = self.parse_inputs(line);
                    }
                },
                Err(_) => (),
            }
        }
    }

    fn export(&mut self, command: &Command) {
        if let Some(idx) = command[1].find('=') {
            let (var, val) = command[1].split_at(idx);
            std::env::set_var(var, &Self::expand_string(&val[1..]));
        }
    }

    fn expand_string(s: &str) -> String {
        if let Ok(s) = shellexpand::env(s) {
            if let Ok(s) = expand::expand_bash(&s) {
                s.to_string()
            } else {
                s.to_string()
            }
        } else {
            s.to_string()
        }
    }

    fn get_job_index(&mut self, pid: Option<u32>) -> Option<usize> {
        if pid.is_none() {
            if self.jobs.len() == 0 {
                return None;
            }
            return Some(self.jobs.len() - 1);
        }
        let pid = pid.unwrap();
        let mut index = 0;
        for children in &self.jobs {
            if children.iter().fold(0, |acc, child| { if *child as u32 == pid { acc + 1 } else { acc } }) > 0 {
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
            self.cur_job = Some(job_index);
            unsafe {
                self.kill(sig_CONT);
            }
            self.wait_for_children_to_finish();
        }
    }

    fn bg(&mut self, command: &Command) {
        let pid = if command.len() == 1 { None } else { command[1].parse().ok() };
        if let Some(job_index) = self.get_job_index(pid) {
            self.cur_job = None;
            unsafe {
                self.kill_job(job_index, sig_CONT);
            }
        }
    }

    fn eval(&mut self, command: &Command) {
        for cmd in &command[1..] {
            self.parse_input(cmd);
        }
    }

    fn update_config(&mut self, _: &Command) {
        self.configure();
        _ = self.lua_parser.load_scripts();
    }


    fn is_builtin(command: &Command) -> bool {
        Self::BUILTIN_COMMANDS.map(|(n,_)| n).contains(&command[0].as_str())
    }


    fn check_builtin_command(&mut self, command: &Command) -> bool {
        let is_builtin = Self::is_builtin(command);

        if is_builtin {
            (self.builtin_handlers.get(&command[0] as &str).unwrap())(self, command);
        }
         
        is_builtin
    }

    fn execute_command(&mut self, command: &mut ChildCommand) -> Result<ChildProcess, std::io::Error> {
        log!(LogLevel::Debug, "Executing: {:?}", command);
        
        match command {
            ChildCommand::Bash(command) => {
                match command.spawn() {
                    Ok(p) => Ok(ChildProcess::Bash(p)),
                    Err(e) => {
                        println!("{:?}", e);
                        Err(e)
                    }
                }
            },
            ChildCommand::Lua(command)  => {
                unsafe {
                    Ok(ChildProcess::Lua(lua_runner_run_command(&mut self.lua_parser as *mut lua_parser::LuaParser as *mut std::ffi::c_void, command as *mut Child)))
                }
            }
        }
    }

    fn wait_for_children_to_finish(&mut self) {
        while unsafe { std::ptr::read_volatile(&self.should_wait) } {
            if !self.get_current_job().is_some_and(|j| j != []) {
                break;
            }
        }
    }

    fn try_wait(pid: u32) -> Result<Option<i32>, ()> {
        unsafe {
            let status = try_wait_process(pid);
            if status == PROCESS_RUNNING {
                    Ok(None)
            } else if status == PROCESS_STOPPED {
                Ok(Some(PROCESS_STOPPED))
            } else {
                Err(())
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

        if let Err(_) = std::fs::read_dir(dir_path) {
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

    fn has_possible_correction_in_path(inp: &str) -> Option<(String, String)> {
        if let Ok(path) = std::env::var("PATH") {
            for dir in path.split(":") {
                if let Some(opt) = Self::check_for_possible_correction_in_dir(dir, inp) {
                    return Some(opt);
                }
            }
        }

        None
    }

    fn get_pid(child: &ChildProcess) -> u32 {
        match child {
            ChildProcess::Bash(p) => p.id(),
            ChildProcess::Lua(p)  => p.pid as u32,
        }
    }

    fn get_current_job(&self) -> Option<&[i32]> {
        if let Some(idx) = self.cur_job {
            Some(&self.jobs[idx])
        } else {
            None
        }
    }

    pub fn get_possible_correction(inp: &str) -> (String, String) {

        if let Some(correction) = Self::has_possible_correction_in_same_dir(inp) {
            correction
        } else if let Some(correction) = Self::has_possible_correction_in_path(inp) {
            correction
        } else {
            ("No solution found".to_string(), "file system".to_string())
        }
    }

    pub fn kill(&mut self, sig: i32) {
        if let Some(cmds) = self.get_current_job() {
            for cmd in cmds {
                unsafe {
                    sig_kill(*cmd as u32, sig);
                }
            }
        }
    }

    fn kill_job(&self, job_idx: usize, sig: i32) {
        if let Some(cmds) = self.jobs.get(job_idx) {
            cmds.iter().map(|cmd| *cmd as u32).for_each(|pid| unsafe { sig_kill(pid, sig) });
        }
    }

    pub fn stop(&mut self) {
        if let Some(children) = self.get_current_job() {
            let ids: Vec<_> = children.iter().map(|pid| *pid as u32).collect();
            unsafe {
                self.should_wait = !signal_is_stopped(ids.as_ptr(), ids.len() as u32);
            }
        }
    }
}

impl From<std::process::Child> for Child {
    fn from(mut value: std::process::Child) -> Self {
        let empty_string = std::ffi::CString::new("").unwrap();
        let empty_string = empty_string.as_ptr();
        let stdin = [0, if value.stdin.is_some() { value.stdin.take().unwrap().as_raw_fd() } else { 0 }];
        let stdout = [if value.stdout.is_some() { value.stdout.take().unwrap().as_raw_fd() } else { 1 }, 1];        
        let stderr = [if value.stderr.is_some() { value.stderr.take().unwrap().as_raw_fd() } else { 2 }, 2];
        Self {
            pid:    value.id() as i32,
            stdin,
            stdout,
            stderr,
            cmd: empty_string as *const u8,
            is_first: 0,
            is_last: 0
        }
    }
}


impl ChildProcess {
    fn get_pid(&self) -> i32 {
        match self {
            Self::Bash(c) => c.id() as i32,
            Self::Lua(c)  => c.pid
        }
    }
}


pub trait Output {
    fn to_stdio(&mut self) -> std::process::Stdio;
    fn to_fd(&mut self) -> RawFd;
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

    fn to_fd(&mut self) -> RawFd {
        self.file.as_raw_fd()
    }

    fn close(self) {
        
    }
}

#[no_mangle]
pub extern "C" fn parser_kill(parser: *mut std::ffi::c_void, sig: i32) {
    unsafe {
        let p: &mut CliParser = &mut *(parser as *mut CliParser);

        p.kill(sig);
    }
}

#[no_mangle]
pub extern "C" fn parser_stop(parser: *mut std::ffi::c_void, sig: i32) {
    unsafe {
        let p: &mut CliParser = &mut *(parser as *mut CliParser);

        p.kill(sig);
        p.stop();
    }
}

#[no_mangle]
pub extern "C" fn parser_child_reaped(parser: *mut std::ffi::c_void, pid: i32) {
    unsafe {
        let p: &mut CliParser = &mut *(parser as *mut CliParser);
        
        let job_idx = p.jobs.iter().enumerate().filter(|(_, j)| j.contains(&pid)).nth(0).unwrap().0;
        let pid_idx = p.jobs[job_idx].iter().find_position(|p| **p == pid).unwrap().0;
        p.jobs[job_idx].swap_remove(pid_idx);
        p.children.remove(&pid);

        if p.jobs[job_idx].is_empty() {
            p.jobs.swap_remove(job_idx);

            if p.cur_job.is_some_and(|idx| idx == job_idx) { 
                p.cur_job = None;

                std::ptr::write_volatile(&mut p.should_wait, false);
            }
        }
    }
}
