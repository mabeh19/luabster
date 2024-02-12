use std::collections::VecDeque;
use std::fs;
use crate::{
    parser::Errors,
    tag,
};

#[cfg(debug_assertions)]
use crate::log::*;
use crate::termio;
use crate::config;

use itertools::Itertools;

const HISTORY_FILE: &str = ".luabster/.history";
const KEYWORDS_SCOPE_INCREASE: [&'static str; 8] = [
    "function",
    "if",
    "case",
    "for",
    "select",
    "while",
    "until",
    "{",
];
const KEYWORDS_SCOPE_DECREASE: [&'static str; 5] = [
    "fi",
    "esac",
    "done",
    "}",
    "end",
];
const KEYWORDS: [&'static str; 6] = [
    "do",
    " in ",
    "time",
    "[[",
    "]]",
    "coproc "
];



impl<'a> config::Configurable<'a> for InputParser {
    fn get_configs(&self) -> &'a [config::ConfigParam<'a>] {
        &tag!{
            "input",
            ,{"history",
                "length" => 1000,
            },
        }
    }

    fn with_config(&mut self, configs: &config::Configs) {
        if let Some(c) = configs.get("input.history.length") {
            match c {
                config::ConfigType::Number(n) => self.load_history(*n as usize),
                _ => (),
            }
        }
    }
}

#[derive(Clone)]
pub struct InputParser {
    history: VecDeque<String>,
    history_path: String
}

impl InputParser {
    
    pub fn new(home_dir: &str) -> Self {
        Self {
            history: VecDeque::new(),
            history_path: format!("{}/{}", home_dir, HISTORY_FILE)
        }
    }

    fn load_history(&mut self, max_history_len: usize) {
        if let Ok(content) = fs::read_to_string(&self.history_path) {
            self.history = content.split("\n").map(|substr| substr.to_owned()).take(max_history_len).collect();
        }
    }

    fn save_history(&self) -> Result<(), Box<dyn std::error::Error>> {
        fs::write(
            &self.history_path,
            self.history.iter().join("\n").as_bytes()
        )?;

        Ok(())
    }

    pub fn get_input(&mut self) -> String {
        let mut full_input = String::new();
        let mut scope = 0;
        
        loop {
            let mut input = self.get_line();
            let new_line_expected = new_line_expected(&mut input, &mut scope);

            full_input.push_str(&input);

            if new_line_expected == false {
                break;
            }

            input.clear();
        }

        return full_input;
    }

    pub fn check_quit(&self, input: &str) -> Result<(), Errors> {
        if input == "exit" {
            match self.save_history() {
                Err(e) => println!("Unable to save history: {:?}", e),
                Ok(_) => {
                    log!(LogLevel::Debug, "Saved history to {}!", self.history_path);
                }
            };
            Err(Errors::Exit) 
        } else {
            Ok(())
        }
    }

    pub fn replace_last(&mut self, rep: &str) {
        self.history.pop_front();
        self.history.push_front(rep.to_string());
    }

    fn get_line(&mut self) -> String {

        let input = termio::get_line(None, &mut self.history, true).unwrap();

        return input.trim().to_string();
    }

}

fn contains_isolated(input: &str, pattern: &str) -> bool {
    if let Some(start_index) = input.find(pattern) {
        if start_index == 0 {
            if input.len() == pattern.len() ||
               input.chars().nth(pattern.len()) == Some(' ') {
                true
            } else {
                false
            }
        } else {
            input.contains(&format!(" {} ", pattern))
        }
    } else {
        false
    }
}

fn contains_keyword(input: &str, scope_level: &mut usize) -> bool {
    for k in KEYWORDS_SCOPE_INCREASE {
        if input.split_whitespace().contains(&k) {
            *scope_level = scope_level.saturating_add(1);
        }
    }

    for k in KEYWORDS_SCOPE_DECREASE {
        if input.split_whitespace().contains(&k) {
            *scope_level = scope_level.saturating_sub(1);
        }
    }

    for k in KEYWORDS {
        if contains_isolated(input, k) {
            return true;
        }
    }

    return false;
}

fn new_line_expected(input: &mut String, scope_level: &mut usize) -> bool {
    log!(LogLevel::Debug, "Checking line: {}", input);
    
    if input.ends_with('\\') {
        input.pop();
        return true;
    }

    contains_keyword(input, scope_level) || *scope_level > 0
}



#[test]
fn sudo_command() {
    let mut sudo_command = "sudo su".to_string();
    let mut scope_level = 0;
    assert!(!contains_keyword(&sudo_command, &mut scope_level));
    assert!(!new_line_expected(&mut sudo_command, &mut scope_level));
    assert_eq!(scope_level, 0);
}

#[test]
fn multiline_command() {
    let mut multiline_command = "sudo su\\".to_string();
    let mut scope_level = 0;

    assert!(!contains_keyword(&multiline_command, &mut scope_level));
    assert!(new_line_expected(&mut multiline_command, &mut scope_level));
}
