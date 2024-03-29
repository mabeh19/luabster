
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

            if option.starts_with(command) {//strsim::jaro_winkler(command, &option) > CMD_SIM_THRESHOLD {
                similar_commands.push(option);
            }
        }
    }

    similar_commands
}

fn get_similar_builtin_commands(command: &str) -> Vec<String> {
    let mut similar_commands = Vec::new();

    for option in parser::CliParser::get_builtin_commands() {
        if option.starts_with(command) { //strsim::jaro_winkler(command, &option) > CMD_SIM_THRESHOLD {
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
    let to_complete = shellexpand::tilde(&shellexpand::full(string).unwrap_or(string.into())).to_string();
    if let Ok(options) = get_files_in_dir(&to_complete) {
        (to_complete, options.0, options.1)
    } else {
        (to_complete, String::new(), Vec::new())
    }
}

fn get_string_at<'a>(string: &'a str, cursor_pos: u16) -> &'a str {
    let mut retval: Option<&'a str> = None;
    let mut len = 0;

    if cursor_pos == string.len() as u16 {
        if string.ends_with(" ") {
            return " ";
        } else {
            // if no spaces are in the name, return easy solution
            let spaces = string.matches("\\ ").count();
            if spaces == 0 {
                return string.split_whitespace().last().unwrap();
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

    for s in string.split_whitespace() {
        if retval.is_none() && cursor_pos <= (len + s.len()) as u16 {
            retval = Some(s);
        }
        len += s.len();
    }

    match retval {
        Some(s) => s,
        None    => " "
    }
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
    //std::fs::write("extra_log.txt", string.as_bytes());
    string.ends_with(" ") || string.split_whitespace().nth(0).unwrap().len() < cursor_pos
}

fn get_common_prefix(replacements: &mut Vec<String>) -> Option<String> {
    replacements.sort();
    let first = replacements.first().unwrap();
    let last = replacements.last().unwrap();

    Some(first.chars().zip(last.chars()).take_while(|(a, b)| a == b).map(|(a,_)| a).collect::<String>())
}
