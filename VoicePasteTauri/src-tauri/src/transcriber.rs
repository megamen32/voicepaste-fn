use crate::config::AppConfig;
use crate::models::Language;
use reqwest::blocking::Client;
use reqwest::header;
use std::path::Path;
use std::time::Duration;

/// Identifier we send as `User-Agent`. Some Whisper-compatible frontends
/// reject the default `reqwest/<version>` UA with HTTP 500 because of
/// User-Agent-based bot/abuse filtering. Pinning to a real product UA fixes it.
const APP_USER_AGENT: &str = concat!("VoicePaste/", env!("CARGO_PKG_VERSION"));

fn build_client(timeout_secs: u64) -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .user_agent(APP_USER_AGENT)
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))
}

/// Server-side Whisper API transcription.
pub struct Transcriber;

impl Transcriber {
    pub fn new() -> Self {
        Self
    }

    /// Transcribe a WAV file using the Whisper API.
    pub fn transcribe(
        &self,
        file_path: &Path,
        language: Language,
        model: Option<&str>,
        config: &AppConfig,
    ) -> Result<String, String> {
        let base_url = config.effective_base_url();
        let base_url = base_url.trim_end_matches('/');
        let url = format!("{}/audio/transcriptions", base_url);

        let client = build_client(60)?;

        let file_data =
            std::fs::read(file_path).map_err(|e| format!("Cannot read audio file: {}", e))?;

        let mut form = reqwest::blocking::multipart::Form::new().text("response_format", "json");

        if let Some(m) = model {
            if !m.is_empty() && m != "auto" {
                form = form.text("model", m.to_string());
            }
        }

        if let Some(lang) = language.api_value() {
            form = form.text("language", lang.to_string());
        }

        let part = reqwest::blocking::multipart::Part::bytes(file_data)
            .file_name("audio.wav")
            .mime_str("audio/wav")
            .map_err(|e| format!("Multipart error: {}", e))?;
        form = form.part("file", part);

        let mut request = client.post(&url).multipart(form);

        let api_key = config.effective_api_key();
        if !api_key.is_empty() {
            request = request.header(header::AUTHORIZATION, format!("Bearer {}", api_key));
        }

        let response = request
            .send()
            .map_err(|e| format!("Request failed: {}", e))?;

        let status = response.status();
        let body = response
            .text()
            .map_err(|e| format!("Cannot read response: {}", e))?;

        if !status.is_success() {
            return Err(format!("HTTP {}: {}", status.as_u16(), body));
        }

        let json: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON parse error: {}", e))?;

        let text = json["text"].as_str().unwrap_or("").trim().to_string();

        if text.is_empty() {
            return Err("Empty transcription result".to_string());
        }

        Ok(text)
    }

    /// Fetch available models from the API.
    pub fn fetch_models(&self, config: &AppConfig) -> Vec<String> {
        let base_url = config.effective_base_url();
        let base_url = base_url.trim_end_matches('/');
        let url = format!("{}/models", base_url);

        let client = build_client(10).unwrap_or_else(|_| Client::new());

        let mut request = client.get(&url);
        let api_key = config.effective_api_key();
        if !api_key.is_empty() {
            request = request.header(header::AUTHORIZATION, format!("Bearer {}", api_key));
        }

        let response = match request.send() {
            Ok(r) => r,
            Err(_) => return vec![],
        };

        let body = match response.text() {
            Ok(b) => b,
            Err(_) => return vec![],
        };

        let json: serde_json::Value = match serde_json::from_str(&body) {
            Ok(j) => j,
            Err(_) => return vec![],
        };

        json["data"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|m| m["id"].as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }
}
