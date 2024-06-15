
pub fn expand_all(s: &str) -> String {
    shellexpand::full(s).unwrap_or(s.into()).to_string()
}


pub fn expand_bash(s: &str) -> Result<String, Box<dyn std::error::Error>> {
    let mut s = s.to_string();
    while let Some(to_expand) = find_expansion(&s) {
        let cmd = &to_expand[2..to_expand.len()-1];
        let repl = std::process::Command::new("bash").args(["-c", cmd]).output()?.stdout;
        let repl = String::from_utf8(repl)?;

        s = s.replace(to_expand, &repl);
    }

    Ok(s)
}

fn find_expansion(s: &str) -> Option<&str> {
    if let Some(idx) = s.find("$(") {
        let start_slice = &s[idx..];
        if let Some(end_idx) = start_slice.find(")") {
            return Some(&start_slice[..end_idx + 1]);
        }
    }
    None
}

