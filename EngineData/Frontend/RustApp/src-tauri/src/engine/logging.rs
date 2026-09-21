use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_AREA_CHARS: usize = 48;
const MAX_LOG_MESSAGE_CHARS: usize = 360;
const MAX_RUNTIME_LOG_FILE_BYTES: u64 = 1_000_000;
const REDACTED_PATH: &str = "[redacted-path]";
const REDACTED_EMAIL: &str = "[redacted-email]";
const REDACTED_SECRET: &str = "[redacted-secret]";

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeLogEvent {
    pub timestamp_unix_ms: u128,
    pub level: String,
    pub area: String,
    pub message: String,
}

impl RuntimeLogEvent {
    pub fn info(area: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp_unix_ms: current_unix_ms(),
            level: "info".to_string(),
            area: compact_log_field(area, MAX_LOG_AREA_CHARS),
            message: compact_log_field(message, MAX_LOG_MESSAGE_CHARS),
        }
    }

    pub fn warning(area: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp_unix_ms: current_unix_ms(),
            level: "warning".to_string(),
            area: compact_log_field(area, MAX_LOG_AREA_CHARS),
            message: compact_log_field(message, MAX_LOG_MESSAGE_CHARS),
        }
    }
}

pub fn write_jsonl_event(
    log_dir: &Path,
    file_name: &str,
    event: &RuntimeLogEvent,
) -> io::Result<()> {
    if !is_safe_log_file_name(file_name) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Unsafe runtime log file name.",
        ));
    }
    fs::create_dir_all(log_dir)?;
    let path = log_dir.join(file_name);
    rotate_if_too_large(&path, file_name)?;
    let line = serde_json::to_string(event)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    append_line(&path, &line)
}

fn rotate_if_too_large(path: &Path, file_name: &str) -> io::Result<()> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(());
    };
    if metadata.len() <= MAX_RUNTIME_LOG_FILE_BYTES {
        return Ok(());
    }
    let rotated_path = rotated_log_path(path, file_name);
    if rotated_path.exists() {
        fs::remove_file(&rotated_path)?;
    }
    match fs::rename(path, &rotated_path) {
        Ok(()) => Ok(()),
        Err(_) => {
            fs::remove_file(path)?;
            Ok(())
        }
    }
}

fn rotated_log_path(path: &Path, file_name: &str) -> PathBuf {
    let rotated_name = file_name
        .strip_suffix(".jsonl")
        .map(|stem| format!("{stem}.previous.jsonl"))
        .unwrap_or_else(|| "runtime.previous.jsonl".to_string());
    path.with_file_name(rotated_name)
}

fn append_line(path: &Path, line: &str) -> io::Result<()> {
    use std::io::Write;

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(file, "{line}")
}

fn is_unsafe_log_character(character: char) -> bool {
    character == '\0'
        || ('\u{0001}'..='\u{0008}').contains(&character)
        || ('\u{000b}'..='\u{001f}').contains(&character)
        || character == '\u{007f}'
        || ('\u{202a}'..='\u{202e}').contains(&character)
        || ('\u{2066}'..='\u{2069}').contains(&character)
}

pub fn sanitize_diagnostic_text(value: impl Into<String>) -> String {
    compact_log_field(value, MAX_LOG_MESSAGE_CHARS)
}

fn compact_log_field(value: impl Into<String>, max_chars: usize) -> String {
    let input = redact_log_value(&value.into());
    let mut output = String::new();
    let mut previous_was_space = false;
    let mut written = 0usize;
    let mut truncated = false;

    for character in input.chars() {
        if is_unsafe_log_character(character) {
            continue;
        }
        let next_character = if character.is_whitespace() {
            if previous_was_space {
                continue;
            }
            previous_was_space = true;
            ' '
        } else {
            previous_was_space = false;
            character
        };

        if written >= max_chars {
            truncated = true;
            break;
        }

        output.push(next_character);
        written += 1;
    }

    let mut output = output.trim().to_string();
    if truncated {
        output.push('…');
    }
    output
}

fn trimmed_log_token(token: &str) -> &str {
    token.trim_matches(|character: char| {
        matches!(
            character,
            ',' | ';' | ')' | '(' | '[' | ']' | '{' | '}' | '"'
        ) || character == char::from(39)
    })
}

fn secret_span_len(tokens: &[&str], index: usize) -> Option<usize> {
    let current = trimmed_log_token(tokens[index]).to_ascii_lowercase();
    if current == "bearer" {
        return Some(if index + 1 < tokens.len() { 2 } else { 1 });
    }

    for key in [
        "token",
        "api_key",
        "api-key",
        "apikey",
        "secret",
        "password",
        "authorization",
    ] {
        if current == key {
            if index + 1 >= tokens.len() {
                return Some(1);
            }
            let next = trimmed_log_token(tokens[index + 1]).to_ascii_lowercase();
            if matches!(next.as_str(), ":" | "=") {
                return Some((tokens.len() - index).min(3));
            }
            if key == "authorization" && next == "bearer" {
                return Some((tokens.len() - index).min(3));
            }
            return Some(2);
        }

        for separator in [':', '='] {
            let prefix = format!("{key}{separator}");
            if let Some(value) = current.strip_prefix(&prefix) {
                if value.is_empty() {
                    if key == "authorization" && index + 1 < tokens.len() {
                        let next = trimmed_log_token(tokens[index + 1]).to_ascii_lowercase();
                        if next == "bearer" {
                            return Some((tokens.len() - index).min(3));
                        }
                    }
                    return Some(if index + 1 < tokens.len() { 2 } else { 1 });
                }
                if key == "authorization" && value == "bearer" {
                    return Some(if index + 1 < tokens.len() { 2 } else { 1 });
                }
                return Some(1);
            }
        }
    }
    None
}

fn redact_log_value(value: &str) -> String {
    let tokens = value.split_whitespace().collect::<Vec<_>>();
    let mut output = Vec::with_capacity(tokens.len());
    let mut index = 0usize;
    while index < tokens.len() {
        if let Some(span) = secret_span_len(&tokens, index) {
            output.push(REDACTED_SECRET.to_string());
            index = index.saturating_add(span.max(1));
            continue;
        }
        output.push(redact_log_token(tokens[index]));
        index += 1;
    }
    output.join(" ")
}

fn redact_log_token(token: &str) -> String {
    let trimmed = trimmed_log_token(token);
    if looks_like_local_path(trimmed) {
        return REDACTED_PATH.to_string();
    }
    if looks_like_email(trimmed) {
        return REDACTED_EMAIL.to_string();
    }
    token.to_string()
}

fn looks_like_local_path(value: &str) -> bool {
    let candidate = value
        .split_once('=')
        .map(|(_, right)| right)
        .unwrap_or(value)
        .trim_start_matches("file:///");
    let normalized = candidate.replace(char::from(92), "/");
    let bytes = normalized.as_bytes();
    let drive_path =
        bytes.len() >= 3 && bytes[1] == b':' && bytes[2] == b'/' && bytes[0].is_ascii_alphabetic();
    drive_path
        || normalized.starts_with("//")
        || normalized.starts_with("/Users/")
        || normalized.starts_with("/home/")
        || normalized.starts_with("/mnt/")
        || normalized.contains("/UserData/")
        || normalized.contains("/EngineData/")
        || normalized.contains("/DevelopingData/")
}

fn looks_like_email(value: &str) -> bool {
    if value.len() > 254 || value.contains('/') || value.contains(char::from(92)) {
        return false;
    }
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.starts_with('.') && !domain.ends_with('.')
}

fn is_safe_log_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.ends_with(".jsonl")
        && !value.contains('/')
        && !value.contains(char::from(92))
        && !value.contains("..")
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '.')
        })
}

fn current_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_log_fields_redact_paths_email_and_inline_secrets() {
        let value = compact_log_field(
            r"failed C:\Users\alice\runtime.log alice@example.com token=abc123 password=hunter2",
            MAX_LOG_MESSAGE_CHARS,
        );
        assert!(value.contains(REDACTED_PATH));
        assert!(value.contains(REDACTED_EMAIL));
        assert!(value.contains(REDACTED_SECRET));
        assert!(!value.contains("alice@example.com"));
        assert!(!value.contains("abc123"));
        assert!(!value.contains("hunter2"));
    }

    #[test]
    fn bearer_credentials_are_redacted_as_a_span() {
        for raw in [
            "Bearer abc.def.123",
            "Authorization: Bearer abc.def.123",
            "authorization=Bearer abc.def.123",
        ] {
            let value = compact_log_field(raw, MAX_LOG_MESSAGE_CHARS);
            assert!(value.contains(REDACTED_SECRET));
            assert!(!value.contains("abc.def.123"));
        }
    }

    #[test]
    fn path_like_assignment_and_unc_paths_are_redacted() {
        for raw in [
            r"stderr=C:\Users\alice\trace.log",
            r"\\server\private\trace.log",
            "/home/alice/trace.log",
        ] {
            assert_eq!(compact_log_field(raw, MAX_LOG_MESSAGE_CHARS), REDACTED_PATH);
        }
    }

    #[test]
    fn unsafe_controls_are_removed_and_log_file_names_stay_bounded() {
        assert_eq!(compact_log_field("safe\u{202e} status", 64), "safe status");
        assert!(is_safe_log_file_name("rust_runtime_latest.jsonl"));
        assert!(!is_safe_log_file_name("../runtime.jsonl"));
        assert!(!is_safe_log_file_name("nested/runtime.jsonl"));
    }
}
