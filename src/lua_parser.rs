#![allow(unused, dead_code, unused_unsafe)]

use std::{
    collections::HashMap,
    io::Write,
};

use rlua::{
    prelude,
    ToLua
};

use itertools::Itertools;

use tempfile;
use crate::Output;
use crate::log;
use crate::log::*;

const LUA_PREFIX: &str = "!";
static mut VAR_DIRECTORY_PATH: Option<String> = None; 


pub struct LuaParser {
    vars: Vec<(String, bool)>,
    var_dir: String,
    lua: rlua::Lua,
}

impl LuaParser {
    pub fn init(home_dir: &str) -> Self {
        
        unsafe {
            VAR_DIRECTORY_PATH = Some(format!("{}/.luabster/var/", home_dir));
            std::fs::create_dir_all(VAR_DIRECTORY_PATH.as_ref().unwrap());
        }

        Self {
            vars: Vec::new(),
            var_dir: home_dir.to_owned(),
            lua: rlua::Lua::new(),
        }
    }

    pub fn parse(&mut self, command: &str) -> bool {
        let is_lua_command = command.starts_with(LUA_PREFIX);

        if is_lua_command {
            log!(LogLevel::Debug, "Running cmd {}", command);
            let command = strip_prefix(command);

            let res: Result<(), rlua::Error> = self.lua.context(|lua_ctx| {
                lua_ctx.load(&command).exec()?;
                Ok(())
            }); 

            match res {
                Ok(e) => (),
                Err(e) => println!("{}", e),
            };
        }

        is_lua_command
    }

    pub fn load_config<'a>(&mut self, params: &[&'a str], home_dir: &str) -> HashMap<&'a str, String> {
        let mut map = HashMap::new();
        let res: Result<(), rlua::Error> = self.lua.context(|lua_ctx| {
            let globals = lua_ctx.globals();
            let package = globals.get::<&str, rlua::Table>("package")?;
            let path: String = package.get("path")?;
            let new_path = format!("{};{}/.luabster/?.lua", path, home_dir);
            package.set("path", new_path)?;
            _ = lua_ctx.load("LuabsterConfig = require\"config\"").exec()?;
            params.iter().for_each(|p| {
                let conf = globals.get("LuabsterConfig");
                if conf.is_err() { return }
                let conf: rlua::Table = conf.unwrap();
                let mut subtables = p.split(".").collect_vec();
                let key = subtables.pop().unwrap();
                if let Ok(subtable) = subtables.iter().try_fold(conf, |acc, subtable| acc.get(*subtable) ) {
                    match subtable.get::<&str, String> (key) {
                        Ok(s) => {
                            map.insert(*p, s);
                        },
                        Err(_) => {
                            log!(LogLevel::Debug, "Config param not found: {}", p);
                        }
                    };
                }
            });

            Ok(())
        });
        
        map
    }

    pub fn append_to_variable(&mut self, command: &str) -> Option<Box<dyn Output>> {
        self.append_to_var(command)
    }

    pub fn output_to_variable(&mut self, command: &str) -> Option<Box<dyn Output>> {
        self.new_var(command)
    }

    fn new_var(&mut self, command: &str) -> Option<Box<dyn Output>> {
        let var_name = strip_prefix(command);
        log!(LogLevel::Debug, "Outputting to new variable: {}", var_name);
        let var = LuaVar::new(&var_name, false);
        self.vars.push((var_name, false));
        Some(Box::new(var))
    }

    fn append_to_var(&mut self, command: &str) -> Option<Box<dyn Output>> {
        let var_name = strip_prefix(command); 
        log!(LogLevel::Debug, "Appending to variable: {}", var_name);
        let var = LuaVar::new(&var_name, true);
        self.vars.push((var_name, true));
        Some(Box::new(var))
    }

    pub fn load_vars_from_memory(&mut self) {
        
    }

    fn load_var_from_memory(&mut self, var_name: &str) {
        var_name;
    }

    pub fn save_vars_to_memory(&mut self) {
        for v in &self.vars {
            self.save_var_to_memory(v);
        }

        self.vars.clear();
    }

    fn save_var_to_memory(&self, var: &(String, bool)) { 
        unsafe {
            let file_name = format!("{}{}", VAR_DIRECTORY_PATH.as_ref().unwrap(), var.0.clone());
            
            log!(LogLevel::Debug, "Saving variable {}", file_name);

            if let Ok(file_data) = std::fs::read_to_string(file_name) {
                log!(LogLevel::Debug, "File `{}` == {}", var.0, file_data);

                let res: Result<(), rlua::Error>  = self.lua.context(|lua_ctx| {
                    let file_data_as_lua = file_data.as_str().to_lua(lua_ctx).unwrap();
                    let var_name_as_lua = var.0.clone().to_lua(lua_ctx).unwrap();
                    if var.1 {
                        let mut var: String = lua_ctx.globals().get(var_name_as_lua.clone()).unwrap();
                        var.push_str(&file_data);
                        lua_ctx.globals().set(var_name_as_lua, var.to_lua(lua_ctx).unwrap())?;
                    } else {
                        lua_ctx.globals().set(var_name_as_lua, file_data_as_lua)?;
                    }

                    Ok(())
                });

                match res {
                    Ok(r) => (),
                    Err(e) => println!("{:?}", e),
                };
            }
        }

    }
}

fn strip_prefix(command: &str) -> String {
    command.trim_start_matches(LUA_PREFIX).to_string()
}

struct LuaVar {
    name: String,
    file: std::fs::File,
    append: bool
}

impl LuaVar {
    fn new(name: &str, should_append: bool) -> Self {
        unsafe {
            Self {
                name: name.to_string(),
                file: std::fs::File::create(format!("{}{}", VAR_DIRECTORY_PATH.as_ref().unwrap(), name)).unwrap(),
                append: should_append
            }
        }
    }
}

impl Output for LuaVar {
    fn to_stdio(&mut self) -> std::process::Stdio {
        std::process::Stdio::from(self.file.try_clone().unwrap())
    }

    fn close(self) {
        
    }
}
