//! Persistent transcription history with a small, dependency-free JSONL store.
//!
//! JSONL keeps the feature portable across macOS, Windows and Ubuntu and makes
//! recovery from a partially written last record straightforward: malformed
//! lines are ignored instead of making the whole history unreadable.

use chrono::Utc;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// `0` means keep history forever.
pub const RETENTION_FOREVER: u32 = 0;
pub const DEFAULT_RETENTION_DAYS: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEntry {
    pub id: String,
    pub created_at: i64,
    pub text: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub duration_ms: u64,
}

fn io_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub fn history_path() -> PathBuf {
    if let Ok(path) = std::env::var("VOICEPASTE_HISTORY") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }

    let dir = directories::ProjectDirs::from("com", "bezrabotnyi", "voicepaste")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| std::env::temp_dir().join("voicepaste-data"));
    dir.join("transcription-history.jsonl")
}

fn read_entries(path: &PathBuf) -> Vec<HistoryEntry> {
    let Ok(data) = fs::read_to_string(path) else {
        return Vec::new();
    };

    data.lines()
        .filter_map(|line| serde_json::from_str::<HistoryEntry>(line).ok())
        .collect()
}

fn write_entries(path: &PathBuf, entries: &[HistoryEntry]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "History path has no parent directory".to_string())?;
    fs::create_dir_all(parent).map_err(|e| format!("Cannot create history directory: {}", e))?;

    let mut data = String::new();
    for entry in entries {
        let line = serde_json::to_string(entry)
            .map_err(|e| format!("Cannot encode history entry: {}", e))?;
        data.push_str(&line);
        data.push('\n');
    }

    let temp = path.with_extension("jsonl.tmp");
    fs::write(&temp, data).map_err(|e| format!("Cannot write history: {}", e))?;
    #[cfg(windows)]
    if path.exists() {
        let _ = fs::remove_file(path);
    }
    fs::rename(&temp, path).map_err(|e| format!("Cannot commit history: {}", e))
}

fn apply_retention(entries: &mut Vec<HistoryEntry>, retention_days: u32) {
    if retention_days == RETENTION_FOREVER {
        return;
    }

    let cutoff = Utc::now().timestamp() - i64::from(retention_days) * 86_400;
    entries.retain(|entry| entry.created_at >= cutoff);
}

/// Append a completed transcription and prune entries outside the retention window.
pub fn append(
    text: String,
    engine: impl Into<String>,
    language: impl Into<String>,
    duration_ms: u64,
    retention_days: u32,
) -> Result<HistoryEntry, String> {
    let _guard = io_lock().lock();
    let path = history_path();
    let mut entries = read_entries(&path);
    apply_retention(&mut entries, retention_days);

    let now = Utc::now();
    let entry = HistoryEntry {
        id: format!("{}-{}", now.timestamp_millis(), entries.len()),
        created_at: now.timestamp(),
        text,
        engine: engine.into(),
        language: language.into(),
        duration_ms,
    };
    entries.push(entry.clone());
    write_entries(&path, &entries)?;
    Ok(entry)
}

/// Return newest entries first and prune old records while reading.
pub fn list(retention_days: u32) -> Result<Vec<HistoryEntry>, String> {
    let _guard = io_lock().lock();
    let path = history_path();
    let before = read_entries(&path);
    let mut entries = before.clone();
    apply_retention(&mut entries, retention_days);
    if entries.len() != before.len() {
        write_entries(&path, &entries)?;
    }
    entries.reverse();
    Ok(entries)
}

pub fn clear() -> Result<(), String> {
    let _guard = io_lock().lock();
    let path = history_path();
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Cannot clear history: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(created_at: i64) -> HistoryEntry {
        HistoryEntry {
            id: created_at.to_string(),
            created_at,
            text: "hello".to_string(),
            engine: "local".to_string(),
            language: "en".to_string(),
            duration_ms: 100,
        }
    }

    #[test]
    fn forever_retention_keeps_old_entries() {
        let mut entries = vec![entry(1)];
        apply_retention(&mut entries, RETENTION_FOREVER);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn retention_removes_entries_before_cutoff() {
        let now = Utc::now().timestamp();
        let mut entries = vec![entry(now - 2 * 86_400), entry(now)];
        apply_retention(&mut entries, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].created_at, now);
    }

    #[test]
    fn history_entry_round_trips_as_json() {
        let original = entry(42);
        let encoded = serde_json::to_string(&original).expect("encode");
        let decoded: HistoryEntry = serde_json::from_str(&encoded).expect("decode");
        assert_eq!(decoded, original);
    }
}
