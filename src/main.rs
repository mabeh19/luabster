#![allow(dead_code)]
#![feature(round_char_boundary)]
#![feature(map_try_insert)]

use std::{
    io::{self, Write},
    error::Error
};

pub mod parser;
pub mod lua_parser;
pub mod log;
pub mod termio;
pub mod input_parser;
pub mod completions;
pub mod config;
pub mod prompt;
pub mod expand;

use crate::{
    parser::*,
    log::*,
};

const WELCOME_MSG: &str = "";

const REPLACE_BASH_COMMAND: usize = 0;
const REPLACE_LUA_COMMAND: usize = 1;
const EDIT_COMMAND: usize = 2;
const ABORT_COMMAND: usize = 3;


extern "C" {
    fn signal_setup(p: *mut std::ffi::c_void);
}

fn main() -> Result<(), Box<dyn Error>> {   
    parse_args();

    let home_dir = home::home_dir().unwrap().display().to_string();
    let mut cli_parser = CliParser::new(&home_dir);

    _ = cli_parser.parse_inputs(&format!("source {}/.luabster/luabster.conf", home_dir));

    unsafe {
        signal_setup(&mut cli_parser as *mut CliParser as *mut std::ffi::c_void);
    }


    print!("{}", WELCOME_MSG);

    loop {
        let prompt = cli_parser.prompt.get(&home_dir);
        display_prompt(&prompt);

        let mut command = cli_parser.input_parser.get_input();

        log!(LogLevel::Debug, "Input received: {}", command);

        match cli_parser.input_parser.check_quit(&command) {
            Err(_) => {
                break;
            },
            Ok(_) => ()
        };

        if let Err(e) = cli_parser.parse_inputs(&command) {
            match e {
                Errors::NoProgramFound(p) => {
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

                                cli_parser.input_parser.replace_last(&command);
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

fn display_prompt(prompt: &str) {
    print!("{}", prompt);
    _ = io::stdout().flush();
}

fn replace_command(command: &mut String, erroneous_command: &str, fixed_command: &str) {
    *command = command.replace(erroneous_command, fixed_command);
}
