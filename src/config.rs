use crate::log::*;

pub type Parser = fn (&str) -> Option<ConfigType>;

#[derive(Debug)]
pub enum ConfigType {
    Color((u8, u8, u8)),
    String(String),
    Number(u32)
}

pub fn parse_color(s: &str) -> Option<ConfigType> {
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

pub fn parse_string(s: &str) -> Option<ConfigType> {
    Some(ConfigType::String(s.to_string()))
}

pub fn parse_number(s: &str) -> Option<ConfigType> {
    if let Ok(n) = s.parse::<u32>() {
        Some(ConfigType::Number(n))
    } else {
        None
    }
}
