#![allow(unused, dead_code, unused_unsafe)]

use std::{
    collections::HashMap,
    io::Write,
    os::fd::{RawFd, AsRawFd},
};

use rlua::{
    prelude,
    ToLua
};

use itertools::Itertools;

use tempfile;
use crate::{
    Output,
    log,
    log::*,
    parser,
    config,
    tag,
};

const LUA_PREFIX: &str = "!";
static mut VAR_DIRECTORY_PATH: Option<String> = None; 


extern "C" {
    fn lua_runner_spawn_command(cmd: *const std::ffi::c_uchar, len: u32, is_first: i32, is_last: i32) -> parser::Child;
}

const SCRIPTS_DIR: &str = "${HOME}/.luabster/scripts";

impl<'a> config::Configurable<'a> for LuaScripts {
    
    fn get_configs(&self) -> &'a [config::ConfigParam<'a>] {
        & tag!( "lua",
            "scripts_dir" => SCRIPTS_DIR,
        )
    }

    fn with_config(&mut self, configs: &config::Configs) {
        if let Some(dir) = configs.get("lua.scripts_dir") {
            match dir {
                config::ConfigType::String(dir) => self.dir = dir.to_string(),
                _ => ()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct LuaScripts {
    dir: String
}


#[derive(Debug)]
pub struct LuaParser {
    vars: Vec<(String, bool)>,
    var_dir: String,
    pub lua: rlua::Lua,
    pub scripts: LuaScripts,
}

impl LuaParser {
    pub fn init(home_dir: &str) -> Self {
        
        unsafe {
            VAR_DIRECTORY_PATH = Some(format!("{}/.luabster/var/", home_dir));
            std::fs::create_dir_all(VAR_DIRECTORY_PATH.as_ref().unwrap());
        }

        if let Ok(p) = shellexpand::full(SCRIPTS_DIR) {
            std::fs::create_dir_all(p.to_string());
        }

        let mut this = Self {
            vars: Vec::new(),
            var_dir: home_dir.to_owned(),
            lua: rlua::Lua::new(),
            scripts: LuaScripts { dir: SCRIPTS_DIR.to_string() },
        };

        let _: Result<(), rlua::Error> = this.lua.context(|lua_ctx| {
            let globals = lua_ctx.globals();
            let lua_version = &globals.get::<&str, String>("_VERSION")?["Lua ".len()..];
            _ = lua_ctx.load(&format!(r#"
                package.path = package.path .. ";{0}/.luabster/?.lua;{0}/.luabster/packages/share/lua/{1}/?/init.lua;{0}/.luabster/packages/share/lua/{1}/?.lua"
                package.cpath = package.cpath .. ";{0}/.luabster/packages/lib/lua/{1}/?.so"

                function Add_Package(name)
                    os.execute("luarocks --tree {0}/.luabster/packages install " .. name)
                end

                function Remove_Package(name)
                    os.execute("luarocks --tree {0}/.luabster/packages remove " .. name)
                end
            "#, home_dir, lua_version)).exec()?;

            Ok(())
        });

        this
    }

    pub fn parse(&mut self, command: &str, first: bool, last: bool) -> Option<parser::Child> {
        let is_lua_command = command.starts_with(LUA_PREFIX);

        if is_lua_command {
            log!(LogLevel::Debug, "Running cmd {}", command);
            let command = strip_prefix(command);

            //let res: Result<(), rlua::Error> = self.lua.context(|lua_ctx| {
            //    lua_ctx.load(&command).exec()?;
            //    Ok(())
            //}); 
            
            unsafe {
                return Some(lua_runner_spawn_command(command.as_ptr(), command.len() as u32, first.into(), last.into()));
            }
        }

        None
    }

    pub fn load_config<'a>(&self, params: &[&'a str], home_dir: &str) -> HashMap<&'a str, String> {
        let mut map = HashMap::new();
        let res: Result<(), rlua::Error> = self.lua.context(|lua_ctx| {
            let globals = lua_ctx.globals();
            _ = lua_ctx.load(&format!("LuabsterConfig = dofile \"{}/.luabster/config.lua\"", home_dir)).exec()?;
            params.iter().for_each(|p| {
                let conf = globals.get("LuabsterConfig");
                if conf.is_err() { return }
                let conf: rlua::Table = conf.unwrap();
                let mut subtables = p.split(".").collect_vec();
                let key = subtables.pop().unwrap();
                if let Ok(subtable) = subtables.iter().try_fold(conf, |cur_table, subtable| cur_table.get(*subtable) ) {
                    match subtable.get::<&str, String>(key) {
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

        log!(LogLevel::Debug, "{:?}", res);
        
        map
    }

    fn run_command(&mut self, cmd: &str) {
    }

    pub fn get_possible_correction(&self, token: &str) -> Option<String> {
        let mut res: Option<String> = None;
        //let _: Result<(), rlua::Error> = self.lua.context(|lua_ctx| {
        //    let globals = lua_ctx.globals();

        //    for pair in globals.pairs::<rlua::Value, rlua::Value>() {
        //        let (p, v) = pair?;
        //        if strsim::jaro_winkler(p, token) > 0.92 {
        //            res =  Some(p);
        //            break;
        //        }
        //    }
        //    Ok(())
        //});

        res
    }

    pub fn append_to_variable(&mut self, command: &str) -> Option<Box<dyn Output>> {
        self.append_to_var(command)
    }

    pub fn output_to_variable(&mut self, command: &str) -> Option<Box<dyn Output>> {
        self.new_var(command)
    }

    pub fn load_scripts(&mut self) -> Result<(), std::io::Error> {
        if let Ok(p) = shellexpand::full(&self.scripts.dir) {
            let p = p.to_string();
            log!(LogLevel::Debug, "Creating dir {}", p);
            std::fs::create_dir_all(&p);

            let fs = std::fs::read_dir(&p)?;

            for f in fs {
                if let Ok(f) = f {
                    log!(LogLevel::Debug, "Loading {}", f.path().display());
                    let code = std::fs::read_to_string(f.path())?;
                    let _: Result<(), rlua::Error> = self.lua.context(|lua_ctx| {
                        lua_ctx.load(&code).exec()?;
                        Ok(())
                    });
                }
            }
        }

        Ok(())
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

#[no_mangle]
pub extern "C" fn run_lua(l: *mut std::ffi::c_void, cmd: *const std::ffi::c_uchar, cmdlen: i32) {
    unsafe {
        let l = &mut *(l as *mut LuaParser);
        let cmd = String::from_raw_parts(cmd as *mut u8, cmdlen as usize, cmdlen as usize);

        let res: Result<(), rlua::Error> = l.lua.context(|lua_ctx| {
            lua_ctx.load(&cmd).exec()?;
            Ok(())
        });
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

    fn to_fd(&mut self) -> RawFd {
        self.file.as_raw_fd()
    }

    fn close(self) {
        
    }
}
