//! Cross-platform post-transcription actions.
//!
//! An action is intentionally either a direct executable invocation or a file
//! write. We never pass a user-configured command line to a shell: each line
//! in `arguments` is one real process argument and the recognized text is
//! written to stdin. This makes `curl`, scripts and local tooling work the
//! same way on macOS, Windows and Linux without creating a shell-injection
//! surface.

use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const ACTION_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationTrigger {
    Keyword,
    FnControl,
}

impl Default for AutomationTrigger {
    fn default() -> Self {
        Self::Keyword
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeywordPosition {
    Start,
    Anywhere,
    End,
}

impl Default for KeywordPosition {
    fn default() -> Self {
        Self::Start
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutomationActionKind {
    Command,
    File,
}

impl Default for AutomationActionKind {
    fn default() -> Self {
        Self::Command
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileWriteMode {
    Append,
    Overwrite,
}

impl Default for FileWriteMode {
    fn default() -> Self {
        Self::Append
    }
}

/// Persisted configuration. `secret` is deliberately omitted from public
/// settings responses; it is exposed to a child only through a dedicated
/// environment variable and `{secret}` placeholder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub trigger: AutomationTrigger,
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub keyword_position: KeywordPosition,
    #[serde(default)]
    pub action_kind: AutomationActionKind,
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub arguments: Vec<String>,
    #[serde(default)]
    pub file_path: String,
    #[serde(default)]
    pub file_mode: FileWriteMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
}

impl Default for AutomationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            trigger: AutomationTrigger::Keyword,
            keyword: String::new(),
            keyword_position: KeywordPosition::Start,
            action_kind: AutomationActionKind::Command,
            command: String::new(),
            arguments: Vec::new(),
            file_path: String::new(),
            file_mode: FileWriteMode::Append,
            secret: None,
        }
    }
}

impl AutomationConfig {
    pub fn requires_fn_control_monitor(&self) -> bool {
        self.enabled && self.trigger == AutomationTrigger::FnControl
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.enabled {
            return Ok(());
        }
        if self.trigger == AutomationTrigger::Keyword && self.keyword.trim().is_empty() {
            return Err("Enter a keyword for the automation trigger.".to_string());
        }
        #[cfg(not(target_os = "macos"))]
        if self.trigger == AutomationTrigger::FnControl {
            return Err(
                "Fn + Control automation is available on macOS only. Use a keyword trigger on this platform."
                    .to_string(),
            );
        }
        match self.action_kind {
            AutomationActionKind::Command if self.command.trim().is_empty() => {
                Err("Enter the executable to run for this automation.".to_string())
            }
            AutomationActionKind::File if self.file_path.trim().is_empty() => {
                Err("Enter a file path for this automation.".to_string())
            }
            _ => Ok(()),
        }
    }

    pub fn masked_secret(&self) -> String {
        self.secret
            .as_deref()
            .map(mask_secret)
            .unwrap_or_else(|| "(not set)".to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AutomationConfigView {
    pub enabled: bool,
    pub trigger: AutomationTrigger,
    pub keyword: String,
    pub keyword_position: KeywordPosition,
    pub action_kind: AutomationActionKind,
    pub command: String,
    pub arguments: Vec<String>,
    pub file_path: String,
    pub file_mode: FileWriteMode,
    pub secret_set: bool,
    pub secret_masked: String,
}

impl From<&AutomationConfig> for AutomationConfigView {
    fn from(config: &AutomationConfig) -> Self {
        Self {
            enabled: config.enabled,
            trigger: config.trigger,
            keyword: config.keyword.clone(),
            keyword_position: config.keyword_position,
            action_kind: config.action_kind,
            command: config.command.clone(),
            arguments: config.arguments.clone(),
            file_path: config.file_path.clone(),
            file_mode: config.file_mode,
            secret_set: config
                .secret
                .as_deref()
                .is_some_and(|value| !value.is_empty()),
            secret_masked: config.masked_secret(),
        }
    }
}

/// A route selected while a recording is in progress. Keyword automation is
/// evaluated after every normal recording; `Fn+Control` selects the explicit
/// automation route and sends the whole transcription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptDelivery {
    Paste,
    AutomationHotkey,
}

impl Default for TranscriptDelivery {
    fn default() -> Self {
        Self::Paste
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationPayload {
    NoMatch,
    Text(String),
    EmptyAfterKeyword,
}

pub fn payload_for_transcript(
    config: &AutomationConfig,
    delivery: TranscriptDelivery,
    transcript: &str,
) -> AutomationPayload {
    if !config.enabled {
        return AutomationPayload::NoMatch;
    }

    match (delivery, config.trigger) {
        (TranscriptDelivery::AutomationHotkey, AutomationTrigger::FnControl) => {
            let text = transcript.trim();
            if text.is_empty() {
                AutomationPayload::EmptyAfterKeyword
            } else {
                AutomationPayload::Text(text.to_string())
            }
        }
        (TranscriptDelivery::Paste, AutomationTrigger::Keyword) => {
            keyword_payload(transcript, &config.keyword, config.keyword_position)
        }
        _ => AutomationPayload::NoMatch,
    }
}

/// Extract the actual payload without the command phrase. For `End`, the
/// natural spoken form is "buy milk, task", so the preceding text becomes the
/// payload; Start and Anywhere take the text following the keyword.
pub fn keyword_payload(
    transcript: &str,
    keyword: &str,
    position: KeywordPosition,
) -> AutomationPayload {
    let keyword = keyword.trim();
    if keyword.is_empty() {
        return AutomationPayload::NoMatch;
    }

    let payload = match position {
        KeywordPosition::Start => {
            let leading = transcript.len() - transcript.trim_start().len();
            find_keyword_at(transcript, keyword, leading).map(|(_, end)| &transcript[end..])
        }
        KeywordPosition::Anywhere => {
            find_keyword(transcript, keyword).map(|(_, end)| &transcript[end..])
        }
        KeywordPosition::End => {
            let end = trim_command_padding_end(transcript);
            find_keyword_ending_at(transcript, keyword, end).map(|(start, _)| &transcript[..start])
        }
    };

    let Some(payload) = payload else {
        return AutomationPayload::NoMatch;
    };
    let payload = trim_command_padding(payload);
    if payload.is_empty() {
        AutomationPayload::EmptyAfterKeyword
    } else {
        AutomationPayload::Text(payload.to_string())
    }
}

/// Execute one configured action. This method intentionally logs neither the
/// transcript nor the secret; the caller can show a short user-safe error.
pub fn run(config: &AutomationConfig, text: &str) -> Result<(), String> {
    config.validate()?;
    match config.action_kind {
        AutomationActionKind::Command => run_command(config, text),
        AutomationActionKind::File => write_file(config, text),
    }
}

fn run_command(config: &AutomationConfig, text: &str) -> Result<(), String> {
    let command = config.command.trim();
    let args: Vec<String> = config
        .arguments
        .iter()
        .map(|argument| replace_placeholders(argument, text, config.secret.as_deref()))
        .collect();

    let mut child = Command::new(command)
        .args(&args)
        .env(
            "VOICEPASTE_ACTION_SECRET",
            config.secret.as_deref().unwrap_or(""),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not start automation '{}': {}", command, error))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|error| format!("Could not send text to automation: {}", error))?;
    }

    let deadline = Instant::now() + ACTION_TIMEOUT;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("Could not wait for automation: {}", error))?
        {
            return if status.success() {
                Ok(())
            } else {
                Err(format!("Automation '{}' exited with {}.", command, status))
            };
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "Automation '{}' did not finish within {} seconds.",
                command,
                ACTION_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn write_file(config: &AutomationConfig, text: &str) -> Result<(), String> {
    let path = Path::new(config.file_path.trim());
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create automation folder: {}", error))?;
    }

    let mut options = OpenOptions::new();
    options.write(true).create(true);
    match config.file_mode {
        FileWriteMode::Append => {
            options.append(true);
        }
        FileWriteMode::Overwrite => {
            options.truncate(true);
        }
    }
    let mut file = options
        .open(path)
        .map_err(|error| format!("Could not open automation file: {}", error))?;
    file.write_all(text.as_bytes())
        .and_then(|_| file.write_all(b"\n"))
        .map_err(|error| format!("Could not write automation file: {}", error))
}

fn replace_placeholders(template: &str, text: &str, secret: Option<&str>) -> String {
    template
        .replace(
            "{text_json}",
            &serde_json::to_string(text).unwrap_or_default(),
        )
        .replace("{text_url}", &percent_encode(text))
        .replace("{text}", text)
        .replace("{secret}", secret.unwrap_or(""))
}

fn percent_encode(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(byte as char);
        } else {
            encoded.push('%');
            encoded.push(hex((byte >> 4) & 0x0f));
            encoded.push(hex(byte & 0x0f));
        }
    }
    encoded
}

fn hex(value: u8) -> char {
    match value {
        0..=9 => (b'0' + value) as char,
        _ => (b'A' + value - 10) as char,
    }
}

fn mask_secret(secret: &str) -> String {
    let characters: Vec<char> = secret.chars().collect();
    if characters.len() <= 8 {
        "****".to_string()
    } else {
        let start: String = characters[..4].iter().collect();
        let end: String = characters[characters.len() - 4..].iter().collect();
        format!("{}...{}", start, end)
    }
}

fn trim_command_padding(value: &str) -> &str {
    value
        .trim()
        .trim_start_matches(|character: char| matches!(character, ':' | ',' | ';' | '-' | '—'))
        .trim_end_matches(|character: char| matches!(character, ':' | ',' | ';' | '-' | '—'))
        .trim()
}

fn trim_command_padding_end(value: &str) -> usize {
    value
        .trim_end_matches(|character: char| {
            character.is_whitespace()
                || matches!(character, '.' | ',' | '!' | '?' | ':' | ';' | '-' | '—')
        })
        .len()
}

fn find_keyword(text: &str, keyword: &str) -> Option<(usize, usize)> {
    text.char_indices()
        .map(|(start, _)| start)
        .find_map(|start| find_keyword_at(text, keyword, start))
}

fn find_keyword_ending_at(
    text: &str,
    keyword: &str,
    required_end: usize,
) -> Option<(usize, usize)> {
    text.char_indices()
        .map(|(start, _)| start)
        .filter_map(|start| find_keyword_at(text, keyword, start))
        .find(|(_, end)| *end == required_end)
}

fn find_keyword_at(text: &str, keyword: &str, start: usize) -> Option<(usize, usize)> {
    if !text.is_char_boundary(start) {
        return None;
    }
    let mut end = start;
    let mut actual = text[start..].chars();
    for expected in keyword.chars() {
        let character = actual.next()?;
        if character.to_lowercase().to_string() != expected.to_lowercase().to_string() {
            return None;
        }
        end += character.len_utf8();
    }
    let before = text[..start].chars().next_back();
    let after = text[end..].chars().next();
    if before.is_some_and(char::is_alphanumeric) || after.is_some_and(char::is_alphanumeric) {
        return None;
    }
    Some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_at_start_sends_only_the_following_text() {
        assert_eq!(
            keyword_payload("Дело: купить молоко", "дело", KeywordPosition::Start),
            AutomationPayload::Text("купить молоко".to_string())
        );
    }

    #[test]
    fn keyword_anywhere_is_case_insensitive_and_respects_words() {
        assert_eq!(
            keyword_payload(
                "пожалуйста ДЕЛО отправить отчёт",
                "дело",
                KeywordPosition::Anywhere
            ),
            AutomationPayload::Text("отправить отчёт".to_string())
        );
        assert_eq!(
            keyword_payload("переделом не команда", "дело", KeywordPosition::Anywhere),
            AutomationPayload::NoMatch
        );
    }

    #[test]
    fn keyword_at_end_sends_text_before_the_command_word() {
        assert_eq!(
            keyword_payload("купить молоко, дело.", "дело", KeywordPosition::End),
            AutomationPayload::Text("купить молоко".to_string())
        );
    }

    #[test]
    fn matched_keyword_without_text_is_not_sent_or_pasted() {
        assert_eq!(
            keyword_payload("дело", "дело", KeywordPosition::Start),
            AutomationPayload::EmptyAfterKeyword
        );
    }

    #[test]
    fn fn_control_uses_the_complete_transcript() {
        let config = AutomationConfig {
            enabled: true,
            trigger: AutomationTrigger::FnControl,
            ..AutomationConfig::default()
        };
        assert_eq!(
            payload_for_transcript(
                &config,
                TranscriptDelivery::AutomationHotkey,
                "новая заметка"
            ),
            AutomationPayload::Text("новая заметка".to_string())
        );
    }

    #[test]
    fn placeholders_are_safe_for_json_and_urls() {
        assert_eq!(
            replace_placeholders(
                "{text_json}|{text_url}|{secret}",
                "тест & ok",
                Some("token")
            ),
            "\"тест & ok\"|%D1%82%D0%B5%D1%81%D1%82%20%26%20ok|token"
        );
    }

    #[test]
    fn file_action_appends_a_complete_transcript() {
        let path =
            std::env::temp_dir().join(format!("voicepaste-automation-{}.txt", std::process::id()));
        let config = AutomationConfig {
            enabled: true,
            keyword: "дело".to_string(),
            action_kind: AutomationActionKind::File,
            file_path: path.display().to_string(),
            file_mode: FileWriteMode::Overwrite,
            ..AutomationConfig::default()
        };
        run(&config, "new note").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new note\n");
        let _ = fs::remove_file(path);
    }

    #[cfg(unix)]
    #[test]
    fn command_action_passes_text_to_standard_input_without_a_shell() {
        let path = std::env::temp_dir().join(format!(
            "voicepaste-automation-stdin-{}.txt",
            std::process::id()
        ));
        let config = AutomationConfig {
            enabled: true,
            keyword: "дело".to_string(),
            command: "sh".to_string(),
            arguments: vec![
                "-c".to_string(),
                "cat > \"$1\"".to_string(),
                "voicepaste-test".to_string(),
                path.display().to_string(),
            ],
            ..AutomationConfig::default()
        };
        run(&config, "new note").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new note");
        let _ = fs::remove_file(path);
    }
}
