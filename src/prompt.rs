use std::collections::HashMap;
use crate::{
    config,
    log::*,
    tag,
};

use colored::Colorize;

type ColorHex = (u8,u8,u8);

#[derive(Clone)]
pub struct Prompt<'a> {
    colors: HashMap<&'a str, ColorHex>,
    custom_prompt: Option<String>,
    show_git: bool,
}

macro_rules! prompt_prefix {
    () => {
        "prompt."
    };
}
macro_rules! colors_prefix {
    () => {
        "colors."
    };
}

const DIR_CONFIG_NAME:  &str = concat!(prompt_prefix!(), colors_prefix!(), "dir");
const USER_CONFIG_NAME: &str = concat!(prompt_prefix!(), colors_prefix!(), "user");
const HOST_CONFIG_NAME: &str = concat!(prompt_prefix!(), colors_prefix!(), "host");
const GIT_COLOR_CONFIG_NAME:  &str = concat!(prompt_prefix!(), colors_prefix!(), "git");
const GIT_ENABLE_CONFIG_NAME: &str = concat!(prompt_prefix!(), "show_git");

const DIR_DEFAULT_COLOR:  (u8, u8, u8) = (0x00, 0xFF, 0xFF);
const NAME_DEFAULT_COLOR: (u8, u8, u8) = (0x00, 0x00, 0xFF);
const HOST_DEFAULT_COLOR: (u8, u8, u8) = (0x00, 0x80, 0x00);
const GIT_DEFAULT_COLOR:  (u8, u8, u8) = (0xFF, 0x00, 0x00);
const GIT_DEFAULT_ENABLE: bool = true;
const PROMPT: &str = "LUABSTER ";


const fn prompt_configs<'a>() -> &'a [config::ConfigParam<'a>] {
    & tag!{ "prompt",
        "show_git"      =>  GIT_DEFAULT_ENABLE,
        { "colors",
            "dir"       =>  DIR_DEFAULT_COLOR,
            "user"      =>  NAME_DEFAULT_COLOR,
            "host"      =>  HOST_DEFAULT_COLOR,
            "git"       =>  GIT_DEFAULT_COLOR,
        },
    }
}

impl<'a> config::Configurable<'a> for Prompt<'a> {
    fn get_configs(&self) -> &'a [config::ConfigParam<'a>] {
        let confs = prompt_configs();
        
        log!(LogLevel::Debug, "{:?}", confs.iter().map(|(p, _)| p).collect::<Vec<_>>());

        confs
    }

    fn with_config(&mut self, configs: &config::Configs) {
        if let Some(p) = configs.get("prompt.custom_prompt") {
            match p {
                config::ConfigType::String(s) => self.custom_prompt = Some(s.to_string()),
                _ => (),
            }
        }
        if let Some(b) = configs.get(GIT_ENABLE_CONFIG_NAME) {
            match b {
                config::ConfigType::Toggle(b) => self.show_git = *b,
                _ => (),
            }
        }
        for (p, default) in prompt_configs() {
            match default.convert() {
                config::ConfigType::Color(c) => _ = self.colors.insert(p, Self::get_config(p, c, configs)),
                _ => (),
            };
        }

        log!(LogLevel::Debug, "Colors: {:?}", self.colors);
    }
}

macro_rules! color {
    ( $color:expr ) => {
        colored::CustomColor::new($color.0, $color.1, $color.2)
    };
}

impl<'a> Prompt<'a> {

    pub fn new() -> Self {
        Self {
            colors: HashMap::new(),
            custom_prompt: None,
            show_git: false,
        }
    }

    fn get_git_branch(p: std::path::PathBuf) -> Option<String> {
        match std::fs::read_to_string(format!("{}/.git/HEAD", p.display())) {
            Ok(s) => {
                let b = if s.starts_with("ref:") {
                    s.replace("ref: refs/heads/", "").replace("\n", "")
                } else {
                    s[..7].to_string()
                };
                Some(format!(" \u{F09B} {}", b))
            },
            Err(_) => None
        }
    }

    pub fn get(&self, home_dir: &str) -> String {
        if let Some(custom_prompt) = &self.custom_prompt {
            return custom_prompt.to_string();
        }
        let dir_color = self.colors.get(DIR_CONFIG_NAME).unwrap();
        let git_color = self.colors.get(GIT_COLOR_CONFIG_NAME).unwrap();
        let user_color = self.colors.get(USER_CONFIG_NAME).unwrap();
        let host_color = self.colors.get(HOST_CONFIG_NAME).unwrap();

        const USERNAME_KEY: &str = "USER";
        if let Ok(cur_dir) = std::env::current_dir() {
            if let Ok(hn) = hostname::get() {
                let current_dir = cur_dir.to_string_lossy().replace(home_dir, "~").custom_color(color!(dir_color));
                let cur_branch = Self::get_git_branch(cur_dir).unwrap_or("".to_string()).custom_color(color!(git_color));
                let user = std::env::var(USERNAME_KEY).unwrap().custom_color(color!(user_color));
                let hn = hn.to_string_lossy().to_string().custom_color(color!(host_color));
                format!("[{}@{}] {}{} \n>> ", user, hn, current_dir, if self.show_git { &cur_branch } else { "" })
            } else {
                format!("[{}] {} >> ", PROMPT, cur_dir.display().to_string().custom_color(color!(dir_color)))
            }  
        } else {
            format!("[{}] ??? >> ", PROMPT)
        }
    }

    fn get_config(conf: &str, default: ColorHex, configs: &config::Configs) -> ColorHex {
        if let Some(conf) = configs.get(conf) {
            match conf {
                config::ConfigType::Color(c) => *c,
                _ => default
            }
        } else {
            default
        }
    }
}
