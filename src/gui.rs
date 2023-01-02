use core::mem;
use std::{
    process,
    thread,
    io::{self, Write},
    error::Error,
    fmt::{Display, Formatter, Result as FmtResult},
};

use crate::log;
use crate::log::*;
use crate::lua_parser;
use crate::parser::*;

use gdk::{
    prelude::*,
    Key,
    ModifierType,
};
use gtk::{
    prelude::*,
    Application, 
    ApplicationWindow,
    EventControllerKey,
    Inhibit,
};
use home;
use hostname;

const WELCOME_MSG: &str = "
    Hello, and welcome to 🦞 LAUBSTER 🦞
";
const PROMPT: &str = "🦞 LAUBSTER 🦞";
const GUI_APP_ID: &str = "app.gtk_rs.luabster";

static mut GUI: Option<&'static mut Gui> = None;

pub struct Gui {
    app: Application,
    lua_parser: lua_parser::LuaParser
}

impl Gui {
    
    pub fn start(lua_parser: lua_parser::LuaParser)
    {
        let mut gui = Self {
            app: Self::create_app(),
            lua_parser
        };

        unsafe {
            GUI = Some(&mut gui);
        }

        gui.app.run();
    }

    fn create_app() -> Application
    {
        let app = Application::builder().application_id(GUI_APP_ID).build();

        app.connect_activate(Self::build_ui);

        app
    }

    fn enter_key_handler()
    {
        unsafe {
            if let Some(mut gui) = GUI {
                display_prompt();

                let command = get_input();

                log!(LogLevel::Debug, "Input received: {}", command);

                match check_quit(&command) {
                    Err(e) => {
                        println!("{:?}", e);
                    },
                    Ok(()) => {

                    }
                };

                parse_inputs(&command, &mut gui.lua_parser);
            }
        }
    }

    fn key_pressed_handler(event_controller: &EventControllerKey, key: Key, keycode: u32, state: ModifierType) -> Inhibit
    {
        let key_handled: bool;
        match key {
            Key::Return => {
                Self::enter_key_handler();

                key_handled = true;
            },
            _ => {
                
                key_handled = false;
            }
        };

        Inhibit(key_handled)
    }

    fn build_ui(app: &Application)
    {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("luabster")
            .build();

        let event_controller = EventControllerKey::new();
        
        event_controller.connect_key_pressed(Self::key_pressed_handler);

        window.add_controller(&event_controller);

        window.present();
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

