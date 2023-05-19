
use std::env::current_dir;


pub enum PosibilityType {
    Executable,
    File,
    ProgramSpecific
}

const CMD_SIM_THRESHOLD: f64 = 0.9;


pub fn get_possibilities<'a>(string: &'a str, cursor_pos: u16) -> (&'a str, String, Vec<String>) {
    match get_possibility_type(string, cursor_pos) {
        PosibilityType::Executable => {
            get_similar_commands(string) 
        },
        PosibilityType::File => {
            get_files(string, cursor_pos)
        },
        PosibilityType::ProgramSpecific => {
            get_command_specific_options(string, cursor_pos)
        }
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

            if strsim::jaro_winkler(command, &option) > CMD_SIM_THRESHOLD {
                similar_commands.push(option);
            }
        }
    }

    similar_commands
}

fn get_similar_builtin_commands(command: &str) -> Vec<String> {
const BUILTIN_COMMANDS: [&str; 3] = [
        "exit",
        "cd",
        "fg"
];

    let mut similar_commands = Vec::new();

    for option in BUILTIN_COMMANDS {
        if strsim::jaro_winkler(command, &option) > CMD_SIM_THRESHOLD {
            similar_commands.push(option.to_string());
        }
    }

    similar_commands
}

fn get_similar_commands<'a>(command: &'a str) -> (&'a str, String, Vec<String>) {
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

    (command, String::new(), similar_commands)
}


fn get_command_specific_options<'a>(string: &'a str, cursor_pos: u16) -> (&'a str, String, Vec<String>) {
    
    let to_complete = get_string_at(string, cursor_pos);

    #[cfg(target_os = "windows")]
    return (to_complete, String::new(), Vec::new());

    // First we check if bash completions exist...
    if let Ok((command, completions)) = get_bash_completions(to_complete, string, cursor_pos) {
        (to_complete, command, completions)
    } else if let Ok((command, completions)) = get_zsh_completions(to_complete, string, cursor_pos) {
        (to_complete, command, completions)
    } else {
        (to_complete, String::new(), Vec::new())
    }
}

fn get_bash_completions(to_complete: &str, string: &str, cursor_pos: u16) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {

    for Ok(entry) in std::fs::read_dir("/etc/bash_completion.d")? {
        if entry.file_name() == 
    }
}


fn get_zsh_completions(to_complete: &str, string: &str, cursor_pos: u16) -> Result<(String, Vec<String>), Box<dyn std::error::Error>> {
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

fn get_files<'a>(string: &'a str, cursor_pos: u16) -> (&'a str, String, Vec<String>) {

    let to_complete = get_string_at(string, cursor_pos);
    if let Ok(options) = get_files_in_dir(to_complete) {
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
            return string.split_whitespace().last().unwrap();
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
    let original_query = path;

    let path = if path.starts_with("~"){
        path.replace("~", &home::home_dir().unwrap().to_string_lossy())
    } else if path == " " {
        ".".to_string()
    } else {
        path.to_string()
    };

    let mut dir = path.clone();
    //let mut file = String::new();
    
    if let Some(n) = dir.rfind("/") {
        _ = dir.split_off(n+1);
    }
    
    //_ = std::fs::write(".files_log", format!("{}", dir));

    for f in std::fs::read_dir(&dir)? {
        if let Ok(f) = f {
            if f.path().to_string_lossy().starts_with(original_query) {
                files.push(f.file_name().to_string_lossy().to_string());
            }
        }
    }

    Ok((dir, files))
}


fn is_command_completion(string: &str, cursor_pos: u16) -> bool {
    string.len() == 0 || string.split_whitespace().nth(0).unwrap().len() >= cursor_pos as usize
}


fn is_file_completion(string: &str, cursor_pos: u16) -> bool {
    let cursor_pos = cursor_pos as usize;
    //std::fs::write("extra_log.txt", string.as_bytes());
    string.ends_with(" ") || string.split_whitespace().nth(0).unwrap().len() < cursor_pos
}
