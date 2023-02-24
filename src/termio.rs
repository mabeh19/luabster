use std::{io::{stdout, Write}, collections::VecDeque, env::current_dir};

pub use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue, style,
    terminal::{self, ClearType},
    Command, Result,
};
use itertools::Itertools;

enum PosibilityType {
    Executable,
    File,
    ProgramSpecific
}

const CMD_SIM_THRESHOLD: f64 = 0.9;


pub fn prompt_for_input(prompt: &str, retain: bool) -> Result<String> {
    print!("{}", prompt);
    std::io::stdout().flush()?;

    let res = get_line(None, &mut VecDeque::new(), retain);

    res
}

fn get_input() -> Result<KeyCode> {
    
    loop {
        let byte = event::read();

        if let Ok(c) = byte {
            match c {
                Event::Key(c) => return Ok(c.code),
                _ => (),
            }
        } else {
            
        }
    }
}

pub fn get_line(start_string: Option<&str>, history: &mut VecDeque<String>, retain: bool) -> Result<String> {
    crossterm::terminal::enable_raw_mode()?;

    let mut string = if start_string.is_some() { start_string.unwrap().to_string() } else { String::new() };
    let start_position = cursor::position().unwrap(); 
    let mut cursor_pos: u16 = string.len() as u16;
    let mut history_index = 0;
    let mut clear_all = false;
    
    history.push_front(string.clone());

    loop {
        show_string(&string, start_position, cursor_pos, clear_all)?;
        stdout().flush()?;

        clear_all = false;

        let inp = match get_input()? {
            KeyCode::Char(c) => {
                clear_all = true;
                Some(c)
            },
            KeyCode::Backspace => {
                if cursor_pos > 0 {
                    string.remove((cursor_pos - 1) as usize);
                    cursor_pos = cursor_pos.saturating_sub(1);
                }
                None
            },
            KeyCode::Left => {
                cursor_pos = cursor_pos.saturating_sub(1);
                None
            },
            KeyCode::Right => {
                sat_add(&mut cursor_pos, 1, string.len() as u16);
                None
            },
            KeyCode::Enter => {
                break;
            },
            KeyCode::Delete => {
                if string.len() > 0 && cursor_pos < (string.len() as u16) {
                    string.remove(cursor_pos as usize);
                }
                None
            },
            KeyCode::Home => {
                cursor_pos = 0;
                None
            },
            KeyCode::End => {
                cursor_pos = string.len() as u16;
                None
            },
            KeyCode::Up => {
                sat_add_usize(&mut history_index, 1, history.len() - 1);
                string = history.get(history_index).expect("Index error in history").to_string();
                cursor_pos = string.len() as u16;

                None
            },
            KeyCode::Down => {
                history_index = history_index.saturating_sub(1);
                string = history.get(history_index).expect("Index error in history").to_string();
                cursor_pos = string.len() as u16;
            
                None
            },
            KeyCode::Tab => {
                let possibilities = get_possibilities(&string, cursor_pos);

                if possibilities.2.len() == 1 {
                    let (to_replace, prefix, completion) = (possibilities.0, possibilities.1, possibilities.2[0].clone());
                    string = string.replace(to_replace, &format!("{}{}", prefix, completion));
                    cursor_pos = string.len() as u16;

                } else {
                    show_possibilities(&possibilities.2, calc_cursor_screen_pos(start_position, cursor_pos));
                }

                None
            },
            _ => None
        };
        
       
        if let Some(inp) = inp {
            string.insert(cursor_pos.into(), inp);
            cursor_pos += 1;
        }

        if history_index == 0 {
            if let Some(front) = history.get_mut(0) {
                *front = string.clone();
            }
        }
    }
    
    if retain {
        execute!(stdout(), style::Print(format!("\r\n")))?;
    } else {
        execute!(stdout(), cursor::MoveTo(start_position.0, start_position.1), terminal::Clear(ClearType::FromCursorDown))?;
    }

    stdout().flush()?;

    crossterm::terminal::disable_raw_mode()?;

    history.pop_front();
    history.push_front(string.clone());

    Ok(string)
}

fn calc_cursor_screen_pos(start_position: (u16, u16), cursor_pos: u16) -> (u16, u16) {
    let screen_size = terminal::size().unwrap();
    let start_offset = start_position.0;
    let cursor_col = (start_offset + cursor_pos) % screen_size.0;
    let cursor_row = start_position.1 + (start_offset + cursor_pos) / screen_size.0;

    (cursor_col, cursor_row)
}

fn show_string(string: &str, start_position: (u16, u16), cursor_pos: u16, clear_all: bool) -> Result<()> {
    
    let (cursor_col, cursor_row) = calc_cursor_screen_pos(start_position, cursor_pos);

    queue!(
        stdout(),
        style::ResetColor,
        cursor::MoveTo(start_position.0, start_position.1),
        if clear_all { 
            terminal::Clear(ClearType::FromCursorDown)
        } else {
            terminal::Clear(ClearType::UntilNewLine)
        },
        style::Print(&string),
        cursor::MoveTo(cursor_col, cursor_row)
    )?;

    Ok(())
}

fn get_possibilities<'a>(string: &'a str, cursor_pos: u16) -> (&'a str, String, Vec<String>) {
    match get_possibility_type(string, cursor_pos) {
        PosibilityType::Executable => {
            get_similar_commands(string) 
        },
        PosibilityType::File => {
            get_files(string, cursor_pos)
        },
        PosibilityType::ProgramSpecific => {
            ("", String::new(), Vec::new())
        }
    }
}

fn get_max_options_per_line(max_str_len: usize) -> usize {
    if max_str_len == 0 {
        return 0;
    }
    let screen_size = terminal::size().unwrap();
    let max_options_per_line = screen_size.0 / (max_str_len as u16);
    let total_width = max_options_per_line + 2 * (max_options_per_line - 1);
    if total_width > screen_size.0 {
        (max_options_per_line - 1) as usize
    } else {
        max_options_per_line as usize
    }
}

fn show_possibilities(strings: &[String], cursor_position: (u16, u16)) {
    let longest_option = strings.iter().fold(0, |max_str_len, s| {
        s.len().max(max_str_len)
    });

    let max_options_per_line = get_max_options_per_line(longest_option);

    strings.chunks(max_options_per_line).for_each(|c| {
        let string = c.join("  ");
        
        _ = execute!(
            stdout(),
            cursor::MoveToNextLine(1),
            style::Print(string)
        );
    });

    _ = execute!(
        stdout(),
        cursor::MoveTo(cursor_position.0, cursor_position.1)
    );
}

fn sat_add(lhs: &mut u16, rhs: u16, upper_bound: u16) {
    *lhs += if (*lhs + rhs) > upper_bound { 0 } else { rhs } 
}

fn sat_add_usize(lhs: &mut usize, rhs: usize, upper_bound: usize) {
    *lhs += if (*lhs + rhs) > upper_bound { 0 } else { rhs } 
}




pub fn get_choice(options: &[&str], retain: bool) -> Result<usize> {

    crossterm::terminal::enable_raw_mode()?;
    let mut choice: usize = 0;

    let start_pos = cursor::position()?;

    loop {
        execute!(
            stdout(),
            cursor::MoveTo(start_pos.0, start_pos.1)
        )?;
        for opt in 0..options.len() {
            if choice == opt {
                queue!(
                    stdout(),
                    style::SetAttribute(style::Attribute::Bold),
                )?;
            } else {
                queue!(
                    stdout(),
                    style::SetAttribute(style::Attribute::NormalIntensity)
                )?;
            }
            queue!(
                stdout(),
                style::Print(format!("{}. {}\r\n", opt, options[opt]))
            )?;
        }

        stdout().flush()?;

        match get_input()? {
            KeyCode::Up => {
                choice = choice.saturating_sub(1);
            },
            KeyCode::Down => {
                sat_add_usize(&mut choice, 1, options.len() - 1);
            },
            KeyCode::Enter => {
                break;
            },
            KeyCode::Char(c) => {
                if c.is_digit(10) {
                    if let Some(d) = c.to_digit(10) {
                        if d < options.len() as u32 {
                            return Ok(d as usize);
                        }
                    }
                }
            }
            _ => (),
        };   
    }

    if !retain {
        queue!(
            stdout(),
            cursor::MoveUp(options.len() as u16),
            terminal::Clear(ClearType::FromCursorDown)
        )?;
    }

    execute!(
        stdout(),
        style::SetAttribute(style::Attribute::NormalIntensity)
    )?;

    crossterm::terminal::disable_raw_mode()?;

    Ok(choice)
}



pub fn edit_command(command: &mut String) -> Result<()> {
    *command = get_line(Some(command), &mut VecDeque::new(), true)?;

    Ok(())
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
const BUILTIN_COMMANDS: [&str; 2] = [
        "exit",
        "cd"
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
    let options = get_files_in_dir(to_complete).unwrap();
    (to_complete, options.0, options.1)
}

fn get_string_at<'a>(string: &'a str, cursor_pos: u16) -> &'a str {
    let mut retval: Option<&'a str> = None;
    let mut len = 0;

    if cursor_pos == string.len() as u16 {
        return string.split_whitespace().last().unwrap();
    }

    for s in string.split_whitespace() {
        if retval.is_none() && cursor_pos <= (len + s.len()) as u16 {
            retval = Some(s);
        }
        len += s.len();
    }

    match retval {
        Some(s) => s,
        None    => "."
    }
}

fn get_files_in_dir(path: &str) -> Result<(String, Vec<String>)> {
    let mut files = Vec::new();

    let path = path.replace("~", &home::home_dir().unwrap().to_string_lossy());
    let mut dir = path.clone();
    //let mut file = String::new();
    
    if let Some(n) = dir.rfind("/") {
        _ = dir.split_off(n+1);
    }
    
    _ = std::fs::write(".files_log", format!("{}", dir));

    for f in std::fs::read_dir(&dir)? {
        if let Ok(f) = f {
            if f.path().to_string_lossy().starts_with(&path) {
                files.push(f.file_name().to_string_lossy().to_string());
            }
        }
    }

    Ok((dir, files))
}

fn is_command_completion(string: &str, cursor_pos: u16) -> bool {
    let tokens = string.split_whitespace();
    string.len() == 0 || tokens.collect_vec()[0].len() >= cursor_pos as usize
}

fn is_file_completion(string: &str, cursor_pos: u16) -> bool {
    string.ends_with(" ") || string.split_whitespace().collect_vec()[0].len() < cursor_pos as usize
}
