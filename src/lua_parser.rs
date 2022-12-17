use std::{
    collections::HashMap,
    io::Write,
};

use tempfile;
use crate::Output;
use crate::log;
use crate::log::*;

const LUA_PREFIX: &str = "!";

static mut VAR_DIRECTORY_PATH: Option<String> = None; 


pub struct LuaParser {
    vars: HashMap<String, LuaVar>,
    var_dir: String,
    lua: rlua::Lua
}

impl LuaParser {
    pub fn init(home_dir: &str) -> Self 
    {      
        unsafe {
            VAR_DIRECTORY_PATH = Some(format!("{}/.luabster/var/", home_dir));
            std::fs::create_dir_all(VAR_DIRECTORY_PATH.as_ref().unwrap());
        }

        Self {
            vars: HashMap::new(),
            var_dir: home_dir.to_owned(),
            lua: rlua::Lua::new()
        }
    }

    pub fn parse(&self, command: &str) -> bool 
    {
        let is_lua_command = command.starts_with(LUA_PREFIX);

        if is_lua_command {
            let command = strip_prefix(command);
            log!(LogLevel::Debug, "Parsing Lua command: {}", command);
            self.lua.context(|lua_ctx| {
                lua_ctx.load(&command).exec();
            });
        }

        is_lua_command
    }

    fn load_var_from_memory(&mut self, var_name: &str) 
    {

    }

    fn save_vars_to_memory(&self) 
    {
        for v in &self.vars {
            self.save_var_to_memory(v);
        }
    }

    fn save_var_to_memory(&self, var: (&String, &LuaVar)) 
    {
        let mut path = self.var_dir.clone();
        path.push_str(var.0);
        let mut file = std::fs::File::create(path).unwrap();

        self.lua.context(|lua_ctx| {
            let var_val: rlua::Table = lua_ctx.globals().get(var.1.name.clone()).unwrap();
        });
    }
}

pub fn append_to_variable(command: &str) -> Option<Box<dyn Output>> 
{
    append_to_var(command)
}

pub fn output_to_variable(command: &str) -> Option<Box<dyn Output>> 
{
    new_var(command)
}

fn new_var(command: &str) -> Option<Box<dyn Output>> 
{
    let var_name = strip_prefix(command);
    log!(LogLevel::Debug, "Outputting to new variable: {}", var_name);
    Some(Box::new(LuaVar::new(&var_name)))
}

fn append_to_var(command: &str) -> Option<Box<dyn Output>> 
{
    let var_name = strip_prefix(command); 
    log!(LogLevel::Debug, "Appending to variable: {}", var_name);
    Some(Box::new(LuaVar::new(&var_name)))
}

fn strip_prefix(command: &str) -> String 
{
    command.replace(LUA_PREFIX, "")
}

struct LuaVar {
    name: String,
    file: std::fs::File
}

impl LuaVar {
    fn new(name: &str) -> Self 
    {
        unsafe {
            Self {
                name: name.to_string(),
                file: std::fs::File::create(format!("{}{}", VAR_DIRECTORY_PATH.as_ref().unwrap(), name)).unwrap()
            }
        }
    }
}

impl Output for LuaVar {
    fn to_stdio(&mut self) -> std::process::Stdio 
    {
        std::process::Stdio::from(self.file.try_clone().unwrap())
    }

    fn close(self) {
        
    }
}
