//! Small black-box entry point for testing clipboard paste in a focused field.

use std::env;
use voicepaste_lib::pasteboard_typer::PasteboardTyper;

fn main() {
    let mut args = env::args().skip(1);
    let mut text = None;
    let mut target_pid = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--text" => text = args.next(),
            "--target-pid" => {
                target_pid = args.next().and_then(|value| value.parse::<i32>().ok());
            }
            _ => {
                eprintln!("usage: voicepaste-paste-probe --text TEXT [--target-pid PID]");
                std::process::exit(2);
            }
        }
    }
    let text = text.unwrap_or_default();

    if let Err(error) = PasteboardTyper::new().paste_to_pid(&text, target_pid) {
        eprintln!("paste failed: {}", error);
        std::process::exit(1);
    }
}
