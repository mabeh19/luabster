#![allow(dead_code)]
#![feature(round_char_boundary)]

use std::{
    io::{self, Write},
    error::Error,
};
use colored::Colorize;

pub mod parser;
pub mod lua_parser;
pub mod log;
pub mod termio;
pub mod input_parser;
pub mod completions;

use crate::{
    parser::*,
    log::*,
};

const WELCOME_MSG: &str = "";

const PROMPT: &str = "LUABSTER ";

const REPLACE_BASH_COMMAND: usize = 0;
const REPLACE_LUA_COMMAND: usize = 1;
const EDIT_COMMAND: usize = 2;
const ABORT_COMMAND: usize = 3;

extern "C" {
    fn signal_setup(p: *mut CliParser);
}

fn main() -> Result<(), Box<dyn Error>> {

    print!("{}", WELCOME_MSG);
   
    parse_args();

    let home_dir = home::home_dir().unwrap().display().to_string();
    let mut cli_parser = CliParser::new();
    let max_history_len = 1000;
    let mut input_parser = input_parser::InputParser::new(&home_dir, max_history_len);

    _ = cli_parser.parse_inputs(&format!("source {}/.luabster/luabster.conf", home_dir));

    unsafe {
        signal_setup(&mut cli_parser as *mut CliParser);
    }
    
    loop {
        let prompt = get_prompt(&home_dir);
        display_prompt(&prompt);

        let mut command = input_parser.get_input();

        log!(LogLevel::Debug, "Input received: {}", command);

        match input_parser.check_quit(&command) {
            Err(_) => {
                //println!("{:?}", e);
                break;
            },
            Ok(_) => ()
        };

        if let Err(e) = cli_parser.parse_inputs(&command) {
            match e {
                Errors::NoProgramFound(p) => {
                    //println!("Did you mean...");
                    let (b_corr, b_corr_p) = CliParser::get_possible_correction(&p);

                    //let l_corr = lua_parser.get_possible_correction(&p);

                    let options = [
                        &format!("{} in {}", b_corr, b_corr_p),
                        &format!("{} in lua", "None"/*l_corr*/),
                        "Edit",
                        "Abort"
                    ];

                    match termio::get_choice(&options, false) {
                        Ok(c) => {
                            let retry = match c {
                                REPLACE_BASH_COMMAND => { if b_corr != "No solution found" { replace_command(&mut command, &p, &b_corr); true } else { false }  },
                                REPLACE_LUA_COMMAND => false,//replace_command(&mut command, &p, &l_corr)
                                EDIT_COMMAND => { termio::edit_command(&mut command)?; true },
                                ABORT_COMMAND => false,
                                _ => false,
                            };

                            if retry {
                                if let Err(e) = cli_parser.parse_inputs(&command) {
                                    println!("{:?}", e);
                                }
                            }
                        },
                        Err(_) => {
                            //println!("{:?}", e);
                        }
                    }
                },
                _ => {
                    println!("{:?}", e);
                }
            }
        }
    }

    Ok(())
}



fn parse_args() {
    let argv = std::env::args().collect::<Vec<String>>();
    for i in 0..argv.len() {
        let arg = &argv[i];
        if arg == "-d" {
            let level = argv[i + 1].clone();
            set_loglevel(level.parse().unwrap());
        }
    }
}

fn get_git_branch(p: std::path::PathBuf) -> Option<String> {
    match std::fs::read_to_string(format!("{}/.git/HEAD", p.display())) {
        Ok(s) => {
            let b = if s.starts_with("ref:") {
                s.replace("ref: refs/heads/", "").replace("\n", "")
            } else {
                s[..7].to_string()
            };
            Some(format!(" \u{F09B} {}", b))
        },
        Err(_) => None
    }
}

fn get_prompt(home_dir: &str) -> String {
    const USERNAME_KEY: &str = "USER";
    if let Ok(cur_dir) = std::env::current_dir() {
        if let Ok(hn) = hostname::get() {
            let current_dir = cur_dir.to_string_lossy().replace(home_dir, "~");
            let cur_branch = get_git_branch(cur_dir).unwrap_or("".to_string());
            format!("[{}@{}] {}{} \n>> ", std::env::var(USERNAME_KEY).unwrap().blue(), hn.into_string().unwrap().green(), current_dir.cyan(), cur_branch.red())
        } else {
            format!("[{}] {} >> ", PROMPT, cur_dir.display())
        }  
    } else {
        format!("[{}] ??? >> ", PROMPT)
    }
}

fn display_prompt(prompt: &str) {
    print!("{}", prompt);
    _ = io::stdout().flush();
}

fn replace_command(command: &mut String, erroneous_command: &str, fixed_command: &str) {
    *command = command.replace(erroneous_command, fixed_command);
}
