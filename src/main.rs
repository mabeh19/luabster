#![allow(dead_code)]

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

use crate::{
    parser::*,
    log::*,
};

const WELCOME_MSG: &str = "
    Hello, and welcome to 🦞 LAUBSTER 🦞
";

const PROMPT: &str = "🦞 LAUBSTER 🦞";

fn main() -> Result<(), Box<dyn Error>> {

    println!("{}", WELCOME_MSG);
   
    parse_args();

    let home_dir = home::home_dir().unwrap().display().to_string();
    let mut lua_parser = lua_parser::LuaParser::init(&home_dir);
    let mut input_parser = input_parser::InputParser::new(&home_dir);
    
    loop {
        display_prompt(&home_dir);

        let command = input_parser.get_input();

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

        parser::parse_inputs(&command, &mut lua_parser);
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

fn display_prompt(home_dir: &str) {
    const USERNAME_KEY: &str = "USER";

    if let Ok(cur_dir) = std::env::current_dir() {
        if let Ok(prompt) = hostname::get() {
            let current_dir = cur_dir.to_string_lossy().replace(home_dir, "~");
            print!("[{}@{}] {} >> ", std::env::var(USERNAME_KEY).unwrap(), prompt.into_string().unwrap(), current_dir);
        } else {
            print!("[{}] {} >> ", PROMPT, cur_dir.display());
        }  
        io::stdout().flush().expect("");
    } else {
        print!("[{}] ??? >> ", PROMPT);
        io::stdout().flush().expect("");
    }
}
