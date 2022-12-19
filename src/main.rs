use core::mem;
use std::{
    process,
    io::{self, Write},
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
};

pub mod parser;
pub mod lua_parser;
pub mod log;

use crate::{
    parser::*,
    log::*
};

use home;
use hostname;

const WELCOME_MSG: &str = "
    Hello, and welcome to 🦞 LAUBSTER 🦞
";


const PROMPT: &str = "🦞 LAUBSTER 🦞";

fn main() -> Result<(), Box<dyn Error>> {
    println!("{}", WELCOME_MSG);
   
    parse_args();

    let home_dir = home::home_dir().unwrap().display().to_string();
    println!("Home dir: {}", home_dir);
    let mut lua_parser = lua_parser::LuaParser::init(&home_dir);

    loop {
        display_prompt();

        let command = get_input();

        log!(LogLevel::Debug, "Input received: {}", command);

        match check_quit(&command) {
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
            unsafe {
                set_loglevel(arg.parse().unwrap());
            }
        }
    }
}

fn display_prompt() {
    const username_key: &str = "USER";

    if let Ok(cur_dir) = std::env::current_dir() {
        if let Ok(prompt) = hostname::get() {
            print!("[{}@{}]: {} >> ", std::env::var(username_key).unwrap(), prompt.into_string().unwrap(), cur_dir.display());
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

