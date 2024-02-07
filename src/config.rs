use std::collections::HashMap;
use crate::log::*;

pub type Parser = fn (&str) -> Option<ConfigType>;
pub type Configs<'a> = HashMap<&'a str, ConfigType>;
pub type ConfigParam<'a> = (&'a str, &'a dyn Configuration);

#[derive(Debug, Clone)]
pub enum ConfigType {
    Color((u8, u8, u8)),
    String(String),
    Number(u32),
    Toggle(bool),
}

pub trait ConfigurationLoader<'a, 'b> {
    fn load_config(&self, params: &[&'a str]) -> HashMap<&'b str, String>;
}

pub fn configure<'a, 'b, T: ConfigurationLoader<'a, 'b>>(configurables: &'a mut [&'a mut dyn Configurable], loader: &T) {    
    let config_params: Vec<ConfigParam> = configurables.iter().map(|p| p.get_configs()).flatten().map(|(p, d)| (*p, *d)).collect();
    let param_names: Vec<&str> = config_params.iter().map(|(p, _)| *p).collect();
    let configs = loader.load_config(&param_names);
    let configs = build_configs(configs, &config_params);

    /*
     * Load configurations
     */
    configurables.iter_mut().for_each(|c| c.with_config(&configs));

    log!(LogLevel::Debug, "Configs: {:?}", configs);
}

pub trait Configurable<'a> {
    fn get_configs(&self) -> &'a [ConfigParam<'a>];
    fn with_config(&mut self, configs: &Configs);
}

pub fn build_configs<'a>(configs: HashMap<&str, String>, config_params: &[ConfigParam<'a>]) -> Configs<'a> {
    config_params.iter().map(|(p, conf)| {
        if let Some(e) = configs.get(p) {
            (*p, conf.from_str(e))
        } else {
            (*p, conf.convert())
        }
    }).collect()

}

fn parse_color(s: &str) -> Option<ConfigType> {
    let s = &s[1..];
    log!(LogLevel::Debug, "Parsing {}", s);
    if i32::from_str_radix(s, 16).is_err() {
        None
    } else {
        Some(ConfigType::Color((
            u8::from_str_radix(&s[0..2], 16).unwrap(),
            u8::from_str_radix(&s[2..4], 16).unwrap(),
            u8::from_str_radix(&s[4..6], 16).unwrap()
        )))
    }
}

fn parse_string(s: &str) -> Option<ConfigType> {
    Some(ConfigType::String(s.to_string()))
}

fn parse_number(s: &str) -> Option<ConfigType> {
    if let Ok(n) = s.parse::<u32>() {
        Some(ConfigType::Number(n))
    } else {
        None
    }
}

fn parse_toggle(s: &str) -> Option<ConfigType> {
    let s = s.to_lowercase();
    if s == "false" {
        Some(ConfigType::Toggle(false))
    } else if s == "true" {
        Some(ConfigType::Toggle(true))
    } else {
        None
    }
}

pub trait Configuration {
    fn convert(&self) -> ConfigType;
    fn from_str(&self, s: &str) -> ConfigType;
}

impl Configuration for &str {
    fn convert(&self) -> ConfigType {
        ConfigType::String(self.to_string())
    }

    fn from_str(&self, s: &str) -> ConfigType {
        ConfigType::String(s.to_string())
    }
}

impl Configuration for u32 {
    fn convert(&self) -> ConfigType {
        ConfigType::Number(*self)
    }

    fn from_str(&self, s: &str) -> ConfigType {
        parse_number(s).unwrap_or(self.convert())
    }
}

impl Configuration for (u8, u8, u8) {
    fn convert(&self) -> ConfigType {
        ConfigType::Color(*self)
    }

    fn from_str(&self, s: &str) -> ConfigType {
        parse_color(s).unwrap_or(self.convert())   
    }
}

impl Configuration for bool {
    fn convert(&self) -> ConfigType {
        ConfigType::Toggle(*self)
    }

    fn from_str(&self, s: &str) -> ConfigType {
        parse_toggle(s).unwrap_or(self.convert())
    }
}

