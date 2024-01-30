#![allow(dead_code)]
#![feature(round_char_boundary)]

use std::{
    io::{self, Write},
    error::Error,
};

pub mod parser;
pub mod lua_parser;
pub mod log;
pub mod termio;
//pub mod gui;
pub mod input_parser;
pub mod completions;

use crate::{
    parser::*,
    log::*,
};

const WELCOME_MSG: &str = r#"
    
"#;

const PROMPT: &str = "LAUBSTER ";

const REPLACE_BASH_COMMAND: usize = 0;
const REPLACE_LUA_COMMAND: usize = 1;
const EDIT_COMMAND: usize = 2;
const ABORT_COMMAND: usize = 3;

fn main() -> Result<(), Box<dyn Error>> {

    println!("{}", WELCOME_MSG);
   
    parse_args();

    let home_dir = home::home_dir().unwrap().display().to_string();
    let mut cli_parser = CliParser::new();
    let mut lua_parser = lua_parser::LuaParser::init(&home_dir);
    let max_history_len = 1000;
    let mut input_parser = input_parser::InputParser::new(&home_dir, max_history_len);
    
    loop {
        let prompt = get_prompt(&home_dir);
        display_prompt(&prompt);

        let mut command = input_parser.get_input();

        if command.is_empty() {
            continue;
        }

        log!(LogLevel::Debug, "Input received: {}", command);

        match input_parser.check_quit(&command) {
            Err(e) => {
                println!("{:?}", e);
                break;
            },
            Ok(()) => {

            }
        };

        if let Err(e) = cli_parser.parse_inputs(&command, &mut lua_parser) {
            match e {
                Errors::NoProgramFound(p) => {
                    println!("🦞`{}` not found 🦞", p);
                    println!("Did you mean...");
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
                                if let Err(e) = cli_parser.parse_inputs(&command, &mut lua_parser) {
                                    println!("{:?}", e);
                                }
                            }
                        },
                        Err(e) => {
                            println!("{:?}", e);
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

fn get_prompt(home_dir: &str) -> String {
    const USERNAME_KEY: &str = "USER";
    if let Ok(cur_dir) = std::env::current_dir() {
        if let Ok(prompt) = hostname::get() {
            let current_dir = cur_dir.to_string_lossy().replace(home_dir, "~");
            format!("[{}@{}] {} >> ", std::env::var(USERNAME_KEY).unwrap(), prompt.into_string().unwrap(), current_dir)
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
