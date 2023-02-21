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

fn main() -> Result<(), Box<dyn Error>> {

<<<<<<< HEAD
    println!("{}", WELCOME_MSG);
=======
//    println!("{}", WELCOME_MSG);
>>>>>>> refs/remotes/origin/main
   
    parse_args();

    let home_dir = home::home_dir().unwrap().display().to_string();
<<<<<<< HEAD
    let mut lua_parser = lua_parser::LuaParser::init(&home_dir);
    let mut cmd_history = Vec::new();
=======
    println!("Home dir: {}", home_dir);
    let lua_parser = lua_parser::LuaParser::init(&home_dir);

    gui::Gui::start(lua_parser);
>>>>>>> refs/remotes/origin/main

    /*
    loop {
        display_prompt();

        let command = input_parser::get_input();

        if command.is_empty() {
            continue;
        }

        cmd_history.push(command.clone());

        log!(LogLevel::Debug, "Input received: {}", command);

        match input_parser::check_quit(&command) {
            Err(e) => {
                println!("{:?}", e);
                break;
            },
            Ok(()) => {

            }
        };

        parser::parse_inputs(&command, &mut lua_parser);
    }
*/
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
<<<<<<< HEAD

fn display_prompt() {
    const USERNAME_KEY: &str = "USER";

    if let Ok(cur_dir) = std::env::current_dir() {
        if let Ok(prompt) = hostname::get() {
            print!("[{}@{}] {} >> ", std::env::var(USERNAME_KEY).unwrap(), prompt.into_string().unwrap(), cur_dir.display());
        } else {
            print!("[{}] {} >> ", PROMPT, cur_dir.display());
        }  
        io::stdout().flush().expect("");
    } else {
        print!("[{}] ??? >> ", PROMPT);
        io::stdout().flush().expect("");
    }
}

=======
>>>>>>> refs/remotes/origin/main
