
use std::env::current_dir;
use crate::parser;

#[derive(Debug)]
pub enum PosibilityType {
    Executable,
    File,
    ProgramSpecific
}

const CMD_SIM_THRESHOLD: f64 = 0.9;


pub fn get_possibilities<'a>(string: &'a str, cursor_pos: u16) -> (&'a str, String, Vec<String>) {
    let to_replace = get_string_at(string, cursor_pos);
    let (to_complete, cmd, mut replacements) = match get_possibility_type(string, cursor_pos) {
        PosibilityType::Executable => {
            get_similar_commands(string) 
        },
        PosibilityType::File => {
            get_files(to_replace)
        },
        PosibilityType::ProgramSpecific => {
            get_files(to_replace)
        }
    };

    if replacements.len() > 1 {
        if let Some(common_prefix) = get_common_prefix(&mut replacements) {
            if common_prefix != to_complete[cmd.len()..] {
                replacements.clear();
                replacements.push(common_prefix);
            }
        }
    }

    (to_replace, reverse_tilde(&cmd), replacements)
}

fn reverse_tilde(s: &str) -> String {
    if let Some(hd) = home::home_dir() {
        s.replace(&hd.display().to_string(), "~")
    } else {
        s.to_string()
    }
}

fn get_similar_commands_in_dir(dir: &str, command: &str) -> Vec<String> {
    
    let mut similar_commands = Vec::new();
    let dir_path = std::path::Path::new(dir);

    if let Err(_) = std::fs::read_dir(dir_path) {
        return similar_commands;
    }

    for f in std::fs::read_dir(dir_path).unwrap() {
        if let Ok(entry) = f {
            let option = entry.file_name().to_string_lossy().to_string();

            if option.starts_with(command) {
                similar_commands.push(option);
            }
        }
    }

    similar_commands
}

fn get_similar_builtin_commands(command: &str) -> Vec<String> {
    let mut similar_commands = Vec::new();

    for option in parser::CliParser::get_builtin_commands() {
        if option.starts_with(command) {
            similar_commands.push(option.to_string());
        }
    }

    similar_commands
}

fn get_similar_commands<'a>(command: &'a str) -> (String, String, Vec<String>) {
    let mut similar_commands = Vec::new();

    similar_commands.append(
        &mut get_similar_builtin_commands(command)
    );

    // Check directories in PATH
    if let Ok(env_path) = std::env::var("PATH") {
        env_path.split(":").for_each(|dir| {
            similar_commands.append(
                &mut get_similar_commands_in_dir(dir, command)
            );
        })
    }

    // Check current directory
    similar_commands.append(
        &mut get_similar_commands_in_dir(&current_dir().unwrap().to_string_lossy(), command)
    );

    // Sort alphabetically and remove duplicates
    similar_commands.sort();
    similar_commands.dedup();

    (command.to_string(), String::new(), similar_commands)
}


fn get_command_specific_options<'a>(string: &'a str, cursor_pos: u16) -> (&'a str, String, Vec<String>) {
    
    let to_complete = get_string_at(string, cursor_pos);

    // First we check if bash completions exist...
    if let Ok((command, completions)) = get_bash_completions(to_complete, string, cursor_pos) {
        (to_complete, command, completions)
    } 
    // Then we check if zsh completions exist...
    else if let Ok((command, completions)) = get_zsh_completions(to_complete, string, cursor_pos) {
        (to_complete, command, completions)
    } 
    // Then we give up :(
    else {
        (to_complete, String::new(), Vec::new())
    }
}

fn get_bash_completions(_to_complete: &str, _string: &str, _cursor_pos: u16) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {

    //for Ok(entry) in std::fs::read_dir("/etc/bash_completion.d")? {
    //    if entry.file_name() == 
    //}
    Ok((String::new(), Vec::new()))
}


fn get_zsh_completions(_to_complete: &str, _string: &str, _cursor_pos: u16) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    todo!();
    
}


fn get_possibility_type(string: &str, cursor_pos: u16) -> PosibilityType {
    
    if is_command_completion(string, cursor_pos) {
        return PosibilityType::Executable;
    } else if is_file_completion(string, cursor_pos) {
        return PosibilityType::File;
    } else {
        return PosibilityType::ProgramSpecific;
    }
}

fn get_files<'a>(string: &'a str) -> (String, String, Vec<String>) {
    let to_complete = crate::expand::expand_all(string);
    if let Ok(options) = get_files_in_dir(&to_complete) {
        (to_complete, options.0, options.1)
    } else {
        (to_complete, String::new(), Vec::new())
    }
}

fn get_string_at<'a>(string: &'a str, cursor_pos: u16) -> &'a str {
    if cursor_pos == string.len() as u16 {
        if string.ends_with(" ") {
            return "";
        } else {
            // if no spaces are in the name, return easy solution
            let spaces = string.matches("\\ ").count();
            if spaces == 0 {
                return string.split_whitespace().last().unwrap_or("");
            }

            let mut idx = cursor_pos as usize - 1;
            // backtrack search for space
            while idx > 1 {
                idx = string.floor_char_boundary(idx - 1);
                let prev = string.floor_char_boundary(idx - 1);

                if string.chars().nth(idx).unwrap() == ' ' && string.chars().nth(prev).unwrap() != '\\' {
                    break;
                }
            }
            return &string[idx + 1 .. cursor_pos as usize];
        }
    }

    let mut idx = 0;
    if cursor_pos > 0 {
        idx = cursor_pos as usize - 1;
        // backtrack search for space
        while idx > 1 {
            idx = string.floor_char_boundary(idx - 1);
            let prev = string.floor_char_boundary(idx - 1);

            if string.chars().nth(idx) == Some(' ') && string.chars().nth(prev) != Some('\\') {
                break;
            }
        }

        idx += 1;
    }

    // continue forward until we reach end of word
    let mut end_of_word = 0;
    let mut skip_next = false;
    for i in cursor_pos as usize.. string.len() {
        if skip_next {
            skip_next = false;
            continue;
        }

        match string.chars().nth(i) {
            Some('\\')          => skip_next = true,
            Some(' ') | None    => break,
            _                   => ()
        };

        end_of_word = i;
    }

    &string[idx .. end_of_word + 1]
}

fn get_files_in_dir(path: &str) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    let mut original_query = path.replace("\\ ", " ");
    let mut first_char_in_file = ' ';

    let path = if path.starts_with("~") {
        let path = path.replace("~", &home::home_dir().unwrap().to_string_lossy());
        original_query = path.to_string();
        path
    } else if path == " " {
        ".".to_string()
    } else {
        path.to_string()
    };

    let mut dir = path;

    if let Some(n) = dir.rfind("/") {
        let rest = dir.split_off(dir.ceil_char_boundary(n+1));
        if let Some(c) = rest.chars().nth(0) {
            first_char_in_file = c;
        }
    } else if let Some(c) = dir.chars().nth(0) {
        first_char_in_file = c;
    }

    let mut orig_dir = dir.clone();
    dir = dir.replace("\\ ", " ");

    //_ = std::fs::write(".files_log", format!("{}", dir));
    if !std::path::Path::new(&dir).exists() {
        // path doesn't exist, retry in current dir
        dir = "./".to_string(); //std::env::current_dir().unwrap().to_string_lossy().to_string();
        original_query.insert_str(0, &dir);
        orig_dir = "./".to_string();
    }

    for f in std::fs::read_dir(&dir)? {
        if let Ok(f) = f {
            let f_path = f.path().to_string_lossy().to_string();
            let mut f_name = f.file_name().to_string_lossy().to_string(); 
            if f_name.starts_with(".") && first_char_in_file != '.' {
                continue;
            }
            if original_query == " " || f_path.starts_with(&original_query) {
                if let Ok(t) = f.file_type() {

                    // Make us not have to add a `/` after every completion of dir
                    if t.is_dir() {
                        f_name.push('/');
                    }

                    f_name = f_name.replace(" ", "\\ ");

                    files.push(f_name);
                }
            }
        }
    }

    Ok((orig_dir, files))
}


fn is_command_completion(string: &str, cursor_pos: u16) -> bool {
    string.len() == 0 || string.split_whitespace().nth(0).unwrap().len() >= cursor_pos as usize
}


fn is_file_completion(string: &str, cursor_pos: u16) -> bool {
    let cursor_pos = cursor_pos as usize;
    let string = string.trim_start();

    if string.is_empty() { 
        false 
    } else {
        string.ends_with(" ") || string.split_whitespace().nth(0).unwrap().len() < cursor_pos
    }
}

fn get_common_prefix(replacements: &mut Vec<String>) -> Option<String> {
    replacements.sort();
    let first = replacements.first().unwrap();
    let last = replacements.last().unwrap();

    let prefix = first.chars().zip(last.chars()).take_while(|(a, b)| a == b).map(|(a,_)| a).collect::<String>();

    if prefix != "" { Some(prefix) } else { None }
}


#[test]
fn test_get_common_prefix() {
    let mut items = [
        "banana",
        "bandana",
        "bandolier",
        "banjo"
    ].into_iter().map(str::to_string).collect();

    let prefix = get_common_prefix(&mut items);

    assert!(prefix.is_some());
    assert_eq!(prefix.unwrap(), "ban");

    let mut items = [
        "banana",
        "apple",
        "cherry",
        "eggplant"
    ].into_iter().map(str::to_string).collect();

    let prefix = get_common_prefix(&mut items);

    assert!(prefix.is_none());
}

#[test]
fn test_is_file_completion() {
    assert!(!is_file_completion("", 0));
    assert!(!is_file_completion(" ", 0));
    assert!(is_file_completion("ls ./C", 5));
}

#[test]
fn test_get_string_at() {
    assert_eq!(get_string_at("ls", 0), "ls");
    assert_eq!(get_string_at("ls x", 4), "x");
    assert_eq!(get_string_at("ls Sub\\ directory", 9), "Sub\\ directory");
    assert_eq!(get_string_at("ls Sub\\ directory -r", 5), "Sub\\ directory");
    assert_eq!(get_string_at("", 0), "");
    assert_eq!(get_string_at("ls ", 3), "");
}



