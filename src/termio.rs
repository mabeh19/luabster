use std::{io::{stdout, Write}, collections::VecDeque};

pub use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue, style,
    terminal::{self, ClearType},
    Command, Result,
};

use crate::completions;


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

    let mut string = start_string.unwrap_or("").to_string();
    let mut start_position = cursor::position().unwrap(); 
    let mut internal_cursor_pos: u16 = string.len() as u16;
    let mut visual_cursor_pos: u16 = string.len() as u16;
    let mut history_index = 0;
    let mut clear_all = false;
    
    history.push_front(string.clone());

    loop {
        show_string(&string, start_position, visual_cursor_pos, clear_all)?;
        stdout().flush()?;

        clear_all = false;

        let inp = match get_input()? {
            KeyCode::Char(c) => {
                clear_all = true;
                Some(c)
            },
            KeyCode::Backspace => {
                if internal_cursor_pos > 0 {
                    let removed = string.remove(string.floor_char_boundary((internal_cursor_pos - 1) as usize) as usize);
                    internal_cursor_pos = internal_cursor_pos.saturating_sub(removed.len_utf8() as u16);
                    visual_cursor_pos = visual_cursor_pos.saturating_sub(1);
                }
                None
            },
            KeyCode::Left => {
                internal_cursor_pos = string.floor_char_boundary(internal_cursor_pos.saturating_sub(1) as usize) as u16;
                visual_cursor_pos = visual_cursor_pos.saturating_sub(1);
                None
            },
            KeyCode::Right => {
                sat_add(&mut internal_cursor_pos, 1, string.len() as u16);
                internal_cursor_pos = string.ceil_char_boundary(internal_cursor_pos as usize) as u16;
                sat_add(&mut visual_cursor_pos, 1, string.len() as u16);
                None
            },
            KeyCode::Enter => {
                break;
            },
            KeyCode::Delete => {
                if string.len() > 0 && internal_cursor_pos < (string.len() as u16) {
                    string.remove(internal_cursor_pos as usize);
                }
                None
            },
            KeyCode::Home => {
                visual_cursor_pos = 0;
                internal_cursor_pos = 0;
                None
            },
            KeyCode::End => {
                visual_cursor_pos = string.len() as u16;
                internal_cursor_pos = string.len() as u16;
                None
            },
            KeyCode::Up => {
                sat_add_usize(&mut history_index, 1, history.len() - 1);
                string = history.get(history_index).expect("Index error in history").to_string();
                visual_cursor_pos = string.chars().count() as u16;
                internal_cursor_pos = string.len() as u16;

                None
            },
            KeyCode::Down => {
                history_index = history_index.saturating_sub(1);
                string = history.get(history_index).expect("Index error in history").to_string();
                visual_cursor_pos = string.chars().count() as u16;
                internal_cursor_pos = string.len() as u16;
            
                None
            },
            KeyCode::Tab => {
                let possibilities = completions::get_possibilities(&string, visual_cursor_pos);

                if possibilities.2.len() == 1 {
                    let (to_replace, prefix, completion) = (possibilities.0, possibilities.1, possibilities.2[0].clone());
                    string = string.replace(to_replace, &format!("{}{}", prefix, completion));
                    visual_cursor_pos = string.len() as u16;
                    internal_cursor_pos = string.len() as u16;

                } else {
                    start_position.1 -= show_possibilities(&possibilities.2, calc_cursor_screen_pos(start_position, visual_cursor_pos));
                }

                None
            },
            _ => None
        };
        
        if let Some(inp) = inp {
            internal_cursor_pos = insert_byte_aligned(&mut string, inp, internal_cursor_pos);
            internal_cursor_pos += inp.len_utf8() as u16;
            visual_cursor_pos += 1;
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

fn show_possibilities(strings: &[String], cursor_position: (u16, u16)) -> u16 {
    if strings.len() == 0 {
        return 0;
    }

    let longest_option = strings.iter().fold(0, |max_str_len, s| {
        s.len().max(max_str_len)
    });

    let mut cursor_position = cursor_position;
    let mut lines_shifted = 0;

    let max_options_per_line = get_max_options_per_line(longest_option);
    let terminal_size = terminal::size().unwrap_or((100, 20));

    if cursor_position.1 == terminal_size.1 - 1 {
        // on last line, make room
        let num_lines = strings.chunks(max_options_per_line).count() + 1;
        
        cursor_position.1 -= num_lines as u16;
        _ = execute!(
            stdout(),
            style::Print("\n".repeat(num_lines)),
            cursor::MoveTo(cursor_position.0, cursor_position.1)
        );  

        lines_shifted += num_lines as u16;
        
    }

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
        
    lines_shifted
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


fn insert_byte_aligned(string: &mut String, c: char, internal_pos: u16) -> u16 {
    let mut internal_pos = internal_pos as usize;

    if !string.is_char_boundary(internal_pos) {
        internal_pos = string.floor_char_boundary(internal_pos);
    }

    string.insert(internal_pos, c);

    internal_pos as u16
}

