use crate::parser::Errors;
use crate::log::*;
use crate::termio;

const KEYWORDS_SCOPE_INCREASE: [&'static str; 8] = [
    "function",
    "if",
    //"then",
    //"else",
    //"elif",
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
    "coproc"
];

fn contains_keyword(input: &str, scope_level: &mut usize) -> bool {
    for k in KEYWORDS_SCOPE_INCREASE {
        if input.contains(k) {
            *scope_level = scope_level.saturating_add(1);
        }
    }

    for k in KEYWORDS_SCOPE_DECREASE {
        if input.contains(k) {
            *scope_level = scope_level.saturating_sub(1);
        }
    }

    for k in KEYWORDS {
        if input.contains(k) {
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

pub fn get_input() -> String {
    let mut full_input = String::new();
    let mut scope = 0;
    
    loop {
        let mut input = get_line();
        let new_line_expected = new_line_expected(&mut input, &mut scope);

        full_input.push_str(&input);

        if new_line_expected == false {
            break;
        }

        input.clear();
    }

    return full_input;
}

pub fn check_quit(input: &str) -> Result<(), Errors> {
    if input == "exit" {
        Err(Errors::Exit) 
    } else {
        Ok(())
    }
}
/*
if (key == ) {
        process.stdout.write('up'); 
    }
    if (key == '\u001B\u005B\u0043') {
        process.stdout.write('right'); 
    }
    if (key == '\u001B\u005B\u0042') {
        process.stdout.write('down'); 
    }
    if (key == '\u001B\u005B\u0044') {
        process.stdout.write('left'); 
    }
*/
fn get_line() -> String {

    let input = termio::get_line(true).unwrap();
    //io::stdin().read_line(&mut input).expect("Failed to read input");

    return input.trim().to_string();
}


