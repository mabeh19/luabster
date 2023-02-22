use std::{io::{stdout, Write}, collections::VecDeque};

pub use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute, queue, style,
    terminal::{self, ClearType},
    Command, Result,
};


pub fn prompt_for_input(prompt: &str, retain: bool) -> Result<String> {
    print!("{}", prompt);
    std::io::stdout().flush()?;

    let res = get_line(&mut VecDeque::new(), retain);

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

pub fn get_line(history: &mut VecDeque<String>, retain: bool) -> Result<String> {
    crossterm::terminal::enable_raw_mode()?;

    let mut string = String::new();
    let screen_size = terminal::size().unwrap();
    let start_position = cursor::position().unwrap(); 
    let start_offset = start_position.0;
    let mut cursor_pos: u16 = string.len() as u16;
    let mut history_index = 0;
    
    history.push_front("".to_string());

    loop {
        queue!(
            stdout(),
            style::ResetColor,
            cursor::MoveTo(start_position.0, start_position.1),
            terminal::Clear(ClearType::FromCursorDown)
        )?;

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
            }

            _ => None
        };
        
       
        if let Some(inp) = inp {
            string.insert(cursor_pos.into(), inp);
            cursor_pos += 1;
        }

        let cursor_col = (start_offset + cursor_pos) % screen_size.0;
        let cursor_row = start_position.1 + (start_offset + cursor_pos) / screen_size.0;

        queue!(
            stdout(),
            style::Print(&string),
            cursor::MoveTo(cursor_col, cursor_row)
        )?;

        stdout().flush()?;

        if history_index == 0 {
            if let Some(front) = history.get_mut(0) {
                *front = string.clone();
            }
        }
    }
    
    if retain {
        queue!(stdout(), style::Print(format!("{}\r\n", string)))?;
    } else {
        queue!(stdout(), cursor::MoveTo(start_position.0, start_position.1), terminal::Clear(ClearType::FromCursorDown))?;
    }

    stdout().flush()?;

    crossterm::terminal::disable_raw_mode()?;

    history.pop_front();
    history.push_front(string.clone());

    Ok(string)
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


