use core::mem;
use std::{
    process,
    thread,
    io::{self, Write},
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
};

pub mod parser;
pub mod lua_parser;
pub mod log;
pub mod gui;

use crate::{
    parser::*,
    log::*,
    gui::*,
};

fn main() -> Result<(), Box<dyn Error>> {

//    println!("{}", WELCOME_MSG);
   
    parse_args();

    let home_dir = home::home_dir().unwrap().display().to_string();
    println!("Home dir: {}", home_dir);
    let lua_parser = lua_parser::LuaParser::init(&home_dir);

    gui::Gui::start(lua_parser);

    /*
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
