//! Small black-box entry point for testing clipboard paste in a focused field.

use std::env;
use voicepaste_lib::pasteboard_typer::PasteboardTyper;

fn main() {
    let mut args = env::args().skip(1);
    let text = match args.next().as_deref() {
        Some("--text") => args.next().unwrap_or_default(),
        _ => {
            eprintln!("usage: voicepaste-paste-probe --text TEXT");
            std::process::exit(2);
        }
    };

    if let Err(error) = PasteboardTyper::new().paste(&text) {
        eprintln!("paste failed: {}", error);
        std::process::exit(1);
    }
}
