use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

const CURRENT_SCHEMA_VERSION: u32 = 8;
const MAX_SETTING_TEXT_CHARS: usize = 160;
const MAX_TERMINOLOGY_ENTRIES: usize = 24;
const MAX_TERMINOLOGY_TERM_CHARS: usize = 80;
const MAX_SPOKEN_TERM_ENTRIES: usize = 24;
const MAX_SPOKEN_TERM_CHARS: usize = 80;
const MAX_SPOKEN_TERM_ALIASES: usize = 4;
const MAX_ASR_HOTWORDS_CHARS: usize = 1024;

fn default_source_language() -> String {
    "id".to_string()
}

fn default_target_language() -> String {
    "en".to_string()
}

fn default_meeting_setup_state() -> String {
    "new".to_string()
}

fn default_meeting_setup_checkpoint() -> u8 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioSettings {
    pub input_device_id: Option<String>,
    pub output_device_id: Option<String>,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            input_device_id: None,
            output_device_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminologyEntry {
    pub indonesian: String,
    pub english: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SpokenTermEntry {
    pub term: String,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeSettings {
    pub schema_version: u32,
    pub source_language: String,
    pub target_language: String,
    pub meeting_setup_state: String,
    pub meeting_setup_checkpoint: u8,
    pub terminology: Vec<TerminologyEntry>,
    pub spoken_terms: Vec<SpokenTermEntry>,
    pub audio: AudioSettings,
}

impl Default for RuntimeSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            source_language: default_source_language(),
            target_language: default_target_language(),
            meeting_setup_state: default_meeting_setup_state(),
            meeting_setup_checkpoint: default_meeting_setup_checkpoint(),
            terminology: Vec::new(),
            spoken_terms: Vec::new(),
            audio: AudioSettings::default(),
        }
    }
}

impl RuntimeSettings {
    pub fn load_or_default(path: &Path) -> Self {
        if let Some(settings) = read_settings_file(path) {
            // If the replacement already committed but the process stopped before
            // cleanup, the valid primary file is authoritative. Remove stale write
            // artifacts so an older backup cannot become a future recovery source.
            let _ = fs::remove_file(path.with_extension("json.bak"));
            let _ = fs::remove_file(path.with_extension("json.tmp"));
            return settings;
        }

        let backup_path = path.with_extension("json.bak");
        if let Some(settings) = read_settings_file(&backup_path) {
            // `write_atomic` keeps the previous committed file here until the new
            // settings rename succeeds. If startup lands in that crash window, use
            // the last committed settings instead of silently resetting to defaults.
            if restore_settings_backup(path, &backup_path).is_ok() {
                let _ = fs::remove_file(path.with_extension("json.tmp"));
            }
            return settings;
        }

        Self::default()
    }

    pub fn save_pretty(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(&self.clone().sanitized())
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        write_atomic(path, &body)
    }

    pub fn sanitized(mut self) -> Self {
        self.schema_version = CURRENT_SCHEMA_VERSION;
        self.source_language = sanitize_language(&self.source_language, "id");
        self.target_language = sanitize_language(&self.target_language, "en");
        self.meeting_setup_state = sanitize_meeting_setup_state(&self.meeting_setup_state);
        self.meeting_setup_checkpoint = self.meeting_setup_checkpoint.clamp(1, 5);
        self.terminology = sanitize_terminology(self.terminology);
        self.spoken_terms = sanitize_spoken_terms(self.spoken_terms);
        self.audio.input_device_id =
            sanitize_optional_runtime_text(self.audio.input_device_id.take());
        self.audio.output_device_id =
            sanitize_optional_runtime_text(self.audio.output_device_id.take());
        self
    }

    pub fn asr_hotwords(&self) -> String {
        let mut result = String::new();
        for value in self.spoken_terms.iter().flat_map(|entry| {
            std::iter::once(&entry.term).chain(entry.aliases.iter())
        }) {
            let separator = if result.is_empty() { "" } else { ", " };
            let next_len = result.chars().count() + separator.chars().count() + value.chars().count();
            if next_len > MAX_ASR_HOTWORDS_CHARS {
                break;
            }
            result.push_str(separator);
            result.push_str(value);
        }
        result
    }
}

fn read_settings_file(path: &Path) -> Option<RuntimeSettings> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str::<RuntimeSettings>(&raw)
        .ok()
        .map(RuntimeSettings::sanitized)
}

fn restore_settings_backup(path: &Path, backup_path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(backup_path, path)?;
    let _ = fs::remove_file(backup_path);
    Ok(())
}

fn write_atomic(path: &Path, body: &str) -> io::Result<()> {
    let temp_path = path.with_extension("json.tmp");
    fs::write(&temp_path, body)?;
    // Preserve the previous good file before any destructive step so a crash between
    // removing the destination and completing the retry rename cannot lose settings.
    let backup_path = path.with_extension("json.bak");
    if path.exists() {
        fs::copy(path, &backup_path)?;
    }
    match fs::rename(&temp_path, path) {
        Ok(()) => {
            let _ = fs::remove_file(&backup_path);
            Ok(())
        }
        Err(first_error) => {
            if !path.exists() {
                let _ = fs::remove_file(&temp_path);
                return Err(first_error);
            }
            fs::remove_file(path)?;
            if let Err(retry_error) = fs::rename(&temp_path, path) {
                let _ = fs::copy(&backup_path, path);
                let _ = fs::remove_file(&temp_path);
                return Err(retry_error);
            }
            let _ = fs::remove_file(&backup_path);
            Ok(())
        }
    }
}

fn is_unsafe_setting_text_character(character: char) -> bool {
    character.is_control()
        || ('\u{202a}'..='\u{202e}').contains(&character)
        || ('\u{2066}'..='\u{2069}').contains(&character)
}

fn clean_setting_text(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| !is_unsafe_setting_text_character(*character))
        .take(MAX_SETTING_TEXT_CHARS)
        .collect::<String>()
}

fn sanitize_language(value: &str, fallback: &str) -> String {
    let text = clean_setting_text(value).to_lowercase();
    if text.starts_with("ind") || text == "id" {
        "id".to_string()
    } else if text.starts_with("eng") || text == "en" {
        "en".to_string()
    } else {
        fallback.to_string()
    }
}

fn sanitize_meeting_setup_state(value: &str) -> String {
    match clean_setting_text(value).to_lowercase().as_str() {
        "deferred" => "deferred".to_string(),
        "completed" => "completed".to_string(),
        _ => "new".to_string(),
    }
}

fn sanitize_optional_runtime_text(value: Option<String>) -> Option<String> {
    let text = clean_setting_text(&value?);
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn clean_terminology_term(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|character| !is_unsafe_setting_text_character(*character))
        .take(MAX_TERMINOLOGY_TERM_CHARS)
        .collect::<String>()
}

fn sanitize_spoken_terms(entries: Vec<SpokenTermEntry>) -> Vec<SpokenTermEntry> {
    let mut clean: Vec<SpokenTermEntry> = Vec::new();
    let mut seen: Vec<String> = Vec::new();

    for entry in entries.into_iter().take(MAX_SPOKEN_TERM_ENTRIES) {
        let term = clean_terminology_term(&entry.term);
        if term.is_empty() || seen.iter().any(|value| value.eq_ignore_ascii_case(&term)) {
            continue;
        }
        seen.push(term.clone());

        let mut aliases = Vec::new();
        for raw_alias in entry.aliases.into_iter().take(MAX_SPOKEN_TERM_ALIASES) {
            let alias = clean_terminology_term(&raw_alias);
            if alias.is_empty() || seen.iter().any(|value| value.eq_ignore_ascii_case(&alias)) {
                continue;
            }
            seen.push(alias.clone());
            aliases.push(alias);
        }
        clean.push(SpokenTermEntry { term, aliases });
    }

    clean
}

fn sanitize_terminology(entries: Vec<TerminologyEntry>) -> Vec<TerminologyEntry> {
    let mut clean = Vec::new();
    for entry in entries.into_iter().take(MAX_TERMINOLOGY_ENTRIES) {
        let indonesian = clean_terminology_term(&entry.indonesian);
        let english = clean_terminology_term(&entry.english);
        if indonesian.is_empty() || english.is_empty() {
            continue;
        }
        let conflicts = clean.iter().any(|existing: &TerminologyEntry| {
            existing.indonesian.eq_ignore_ascii_case(&indonesian)
                || existing.english.eq_ignore_ascii_case(&english)
        });
        if !conflicts {
            clean.push(TerminologyEntry { indonesian, english });
        }
    }
    clean
}

#[cfg(test)]
mod tests {
    use super::{RuntimeSettings, CURRENT_SCHEMA_VERSION};
    use std::fs;

    #[test]
    fn missing_settings_file_falls_back_to_current_defaults() {
        let path = std::env::temp_dir().join("translateit_missing_settings_test.json");
        let _ = fs::remove_file(&path);
        let settings = RuntimeSettings::load_or_default(&path);
        assert_eq!(settings.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(settings.source_language, "id");
        assert_eq!(settings.target_language, "en");
        assert_eq!(settings.meeting_setup_state, "new");
        assert_eq!(settings.meeting_setup_checkpoint, 1);
        assert!(settings.terminology.is_empty());
        assert!(settings.spoken_terms.is_empty());
        assert!(settings.audio.input_device_id.is_none());
        assert!(settings.audio.output_device_id.is_none());
    }

    #[test]
    fn terminology_rejects_conflicting_directional_mappings() {
        use super::TerminologyEntry;

        let mut settings = RuntimeSettings::default();
        settings.terminology = vec![
            TerminologyEntry {
                indonesian: "pemugaran".to_string(),
                english: "restoration".to_string(),
            },
            TerminologyEntry {
                indonesian: "PEMUGARAN".to_string(),
                english: "renovation".to_string(),
            },
            TerminologyEntry {
                indonesian: "restorasi".to_string(),
                english: "RESTORATION".to_string(),
            },
            TerminologyEntry {
                indonesian: "arsip".to_string(),
                english: "archive".to_string(),
            },
        ];

        let sanitized = settings.sanitized();
        assert_eq!(sanitized.terminology.len(), 2);
        assert_eq!(sanitized.terminology[0].indonesian, "pemugaran");
        assert_eq!(sanitized.terminology[0].english, "restoration");
        assert_eq!(sanitized.terminology[1].indonesian, "arsip");
        assert_eq!(sanitized.terminology[1].english, "archive");
    }

    #[test]
    fn spoken_terms_are_sanitized_deduplicated_and_bounded_for_asr() {
        use super::SpokenTermEntry;

        let mut settings = RuntimeSettings::default();
        settings.spoken_terms = vec![
            SpokenTermEntry {
                term: "MIVUBI".to_string(),
                aliases: vec!["mi vu bi".to_string(), "MIVUBI".to_string()],
            },
            SpokenTermEntry {
                term: "mivubi".to_string(),
                aliases: vec!["duplicate".to_string()],
            },
            SpokenTermEntry {
                term: "Vredeburg".to_string(),
                aliases: vec!["vrede burg".to_string()],
            },
        ];

        let sanitized = settings.sanitized();
        assert_eq!(sanitized.spoken_terms.len(), 2);
        assert_eq!(sanitized.spoken_terms[0].term, "MIVUBI");
        assert_eq!(sanitized.spoken_terms[0].aliases, vec!["mi vu bi"]);
        assert_eq!(sanitized.spoken_terms[1].term, "Vredeburg");
        assert_eq!(sanitized.asr_hotwords(), "MIVUBI, mi vu bi, Vredeburg, vrede burg");
        assert!(sanitized.asr_hotwords().chars().count() <= super::MAX_ASR_HOTWORDS_CHARS);
    }

    #[test]
    fn valid_primary_clears_stale_atomic_write_artifacts() {
        let path = std::env::temp_dir().join(format!(
            "translateit_settings_stale_cleanup_{}.json",
            std::process::id()
        ));
        let backup = path.with_extension("json.bak");
        let temp = path.with_extension("json.tmp");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let _ = fs::remove_file(&temp);

        let mut current = RuntimeSettings::default();
        current.meeting_setup_state = "completed".to_string();
        current
            .save_pretty(&path)
            .expect("current settings should save");
        fs::write(&backup, r#"{"meeting_setup_state":"new"}"#)
            .expect("stale backup should be written");
        fs::write(&temp, r#"{"meeting_setup_state":"deferred"}"#)
            .expect("stale temp should be written");

        let loaded = RuntimeSettings::load_or_default(&path);
        assert_eq!(loaded.meeting_setup_state, "completed");
        assert!(!backup.exists(), "valid primary should retire stale backup");
        assert!(!temp.exists(), "valid primary should retire stale temp");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let _ = fs::remove_file(&temp);
    }

    #[test]
    fn crash_window_recovers_last_committed_settings_from_backup() {
        let path = std::env::temp_dir().join(format!(
            "translateit_settings_recovery_{}.json",
            std::process::id()
        ));
        let backup = path.with_extension("json.bak");
        let temp = path.with_extension("json.tmp");
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let _ = fs::remove_file(&temp);

        let mut committed = RuntimeSettings::default();
        committed.source_language = "en".to_string();
        committed.target_language = "id".to_string();
        committed.meeting_setup_state = "completed".to_string();
        committed.meeting_setup_checkpoint = 4;
        committed.audio.input_device_id = Some("Committed Microphone".to_string());
        committed
            .save_pretty(&path)
            .expect("committed settings should save");

        fs::copy(&path, &backup).expect("last committed settings backup should exist");
        fs::write(&temp, r#"{"source_language":"id","target_language":"en"}"#)
            .expect("new uncommitted temp settings should exist");
        fs::remove_file(&path).expect("simulate crash after destination removal");

        let recovered = RuntimeSettings::load_or_default(&path);
        assert_eq!(recovered.source_language, "en");
        assert_eq!(recovered.target_language, "id");
        assert_eq!(recovered.meeting_setup_state, "completed");
        assert_eq!(recovered.meeting_setup_checkpoint, 4);
        assert_eq!(
            recovered.audio.input_device_id.as_deref(),
            Some("Committed Microphone")
        );
        assert!(path.is_file(), "backup recovery should restore the main settings file");
        assert!(!backup.exists(), "restored backup should not remain stale");
        assert!(!temp.exists(), "uncommitted temp settings should be discarded");

        let reloaded = RuntimeSettings::load_or_default(&path);
        assert_eq!(reloaded.source_language, "en");
        assert_eq!(reloaded.target_language, "id");

        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(&backup);
        let _ = fs::remove_file(&temp);
    }

    #[test]
    fn legacy_settings_shape_is_read_without_persisting_retired_fields() {
        let path = std::env::temp_dir().join(format!(
            "translateit_legacy_settings_{}.json",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);
        let legacy = r#"{
  "schema_version": 5,
  "language_focus_mode": "id-en-focus",
  "runtime_profile": "retired-value",
  "source_language": "en",
  "target_language": "id",
  "history_enabled": true,
  "meeting_setup_state": "deferred",
  "meeting_setup_checkpoint": 3,
  "audio": {
    "input_device_id": "Legacy Microphone",
    "output_device_id": "Legacy Meeting Sound",
    "sensitivity": 1.8,
    "input_sensitivity": "retired-value",
    "show_advanced_devices": true,
    "allow_low_but_usable_input": true,
    "allow_cpu_degraded_mode": true,
    "auto_play_translation_voice": true,
    "auto_play_out_voice": true,
    "use_custom_voice_actor": true,
    "voice_actor_profiles_root": "EngineData/VoiceActorProfiles"
  },
  "voice_actor_profile_id": "marcel"
}"#;
        fs::write(&path, legacy).expect("legacy settings should be written");

        let settings = RuntimeSettings::load_or_default(&path);
        assert_eq!(settings.schema_version, CURRENT_SCHEMA_VERSION);
        assert_eq!(settings.source_language, "en");
        assert_eq!(settings.target_language, "id");
        assert_eq!(settings.meeting_setup_state, "deferred");
        assert_eq!(settings.meeting_setup_checkpoint, 3);
        assert_eq!(settings.audio.input_device_id.as_deref(), Some("Legacy Microphone"));
        assert_eq!(
            settings.audio.output_device_id.as_deref(),
            Some("Legacy Meeting Sound")
        );

        settings
            .save_pretty(&path)
            .expect("migrated settings should save in the current small shape");
        let saved = fs::read_to_string(&path).expect("saved settings should be readable");
        let _ = fs::remove_file(&path);

        for retired in [
            "language_focus_mode",
            "runtime_profile",
            "history_enabled",
            "sensitivity",
            "input_sensitivity",
            "show_advanced_devices",
            "allow_low_but_usable_input",
            "allow_cpu_degraded_mode",
            "auto_play_translation_voice",
            "auto_play_out_voice",
            "use_custom_voice_actor",
            "voice_actor_profiles_root",
            "voice_actor_profile_id",
        ] {
            assert!(!saved.contains(retired), "retired setting persisted: {retired}");
        }
    }
}
