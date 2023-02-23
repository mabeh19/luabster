use std::{io::{stdout, Write}, collections::VecDeque, env::current_dir};

use itertools::Itertools;
pub use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue, style,
    terminal::{self, ClearType},
    Command, Result,
};

const CMD_SIM_THRESHOLD: f64 = 0.8;


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

    
    history.push_front(string.clone());

    loop {
        show_string(&string, start_position, cursor_pos)?;

        stdout().flush()?;

        let inp = match get_input()? {
            KeyCode::Char(c) => Some(c),
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
                let possibilities = get_possibilities(&string);

                if possibilities.len() == 1 {
                    string = possibilities[0].clone();
                    cursor_pos = string.len() as u16;

                } else if possibilities.len() > 0 {
                    show_possibilities(&possibilities, calc_cursor_screen_pos(start_position, cursor_pos));
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

fn show_string(string: &str, start_position: (u16, u16), cursor_pos: u16) -> Result<()> {
    
    let (cursor_col, cursor_row) = calc_cursor_screen_pos(start_position, cursor_pos);

    queue!(
        stdout(),
        style::ResetColor,
        cursor::MoveTo(start_position.0, start_position.1),
        terminal::Clear(ClearType::FromCursorDown),
        style::Print(&string),
        cursor::MoveTo(cursor_col, cursor_row)
    )?;

    Ok(())
}

fn get_possibilities(string: &str) -> Vec<String> {
    get_similar_commands(string)   
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

fn get_similar_commands(command: &str) -> Vec<String> {
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

    similar_commands
}


