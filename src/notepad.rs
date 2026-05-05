use crate::get_notepad;
use std::process::Command;

const NOTEPAD_BASH: &str = include_str!(r#"./notepad.sh"#);

pub fn run() {
    let _ = Command::new("sh")
        .arg("-c")
        .arg(NOTEPAD_BASH)
        .env("NOTE_FILE", get_notepad().to_str().unwrap())
        .spawn()
        .unwrap()
        .wait();
}