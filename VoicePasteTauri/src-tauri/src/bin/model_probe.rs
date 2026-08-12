use std::process::ExitCode;

use voicepaste_lib::config::AppConfig;
use voicepaste_lib::transcriber::Transcriber;

fn required_arg(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {}", name))
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut endpoint = None;
    let mut api_key = String::new();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--endpoint" => endpoint = Some(required_arg(&mut args, "--endpoint")?),
            "--api-key" => api_key = required_arg(&mut args, "--api-key")?,
            other => return Err(format!("unknown argument: {}", other)),
        }
    }

    let endpoint = endpoint.ok_or_else(|| "missing required --endpoint".to_string())?;
    let mut config = AppConfig::default();
    config.base_url = endpoint;
    config.api_key = Some(api_key);

    let models = Transcriber::new().fetch_models(&config);
    println!("{}", serde_json::json!({"models": models}));
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("model probe failed: {}", error);
            ExitCode::FAILURE
        }
    }
}
