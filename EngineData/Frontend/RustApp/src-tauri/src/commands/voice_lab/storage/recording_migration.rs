//! Explicit copy-only protection for old accepted and interrupted-backup WAVs.
//! Active take paths and the pinned GPT-SoVITS worker remain unchanged.
use serde::Serialize;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::recording_archive::{archive_guided_take, guided_take_is_archived};
use super::VoiceLabStoragePaths;

#[derive(Debug, Clone, Serialize)]
pub struct LegacyProtectionStatus {
    pub total_sources: usize,
    pub protected_sources: usize,
    pub unprotected_sources: usize,
    pub attention_sources: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LegacyMigrationResult {
    pub ok: bool,
    pub newly_archived: usize,
    pub report: LegacyProtectionStatus,
    pub message: String,
}

struct Source<'a> {
    id: u32,
    text: &'a str,
    path: PathBuf,
    native: Option<PathBuf>,
}

fn present(path: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(_) => Err("voice_lab:legacy_source_metadata_unknown".into()),
    }
}

fn legacy_candidates<'a>(
    storage: &VoiceLabStoragePaths, prompts: &'a [(u32, &'a str)],
) -> (Vec<Source<'a>>, usize) {
    let mut sources = Vec::new();
    let mut issues = 0;
    // A symlinked storage root could redirect private voice files into an
    // unexpected location. Refuse to import it, including in status reads.
    for root in [&storage.takes_dir, &storage.takes_dir.join("NativeRate")] {
        match fs::symlink_metadata(root) {
            Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => {
                return (Vec::new(), 1);
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return (Vec::new(), 1),
        }
    }
    for &(id, text) in prompts {
        let active = storage.takes_dir.join(format!("take_{id:04}.wav"));
        let native = storage.takes_dir.join("NativeRate").join(format!("take_{id:04}.wav"));
        for (canonical, sidecar) in [
            (active.clone(), native.clone()),
            (active.with_extension("wav.previous"), native.with_extension("wav.previous")),
        ] {
            match (present(&canonical), present(&sidecar)) {
                (Ok(true), Ok(native_present)) => sources.push(Source {
                    id, text, path: canonical, native: native_present.then_some(sidecar),
                }),
                (Ok(false), Ok(true)) => issues += 1, // orphan native, never delete
                (Ok(false), Ok(false)) => {},
                _ => issues += 1, // inaccessible source metadata is unknown
            }
        }
    }
    // Do not silently ignore unknown historical WAVs. Those require an
    // explicit investigation before the library can claim full protection.
    for (dir, is_native) in [
        (storage.takes_dir.clone(), false),
        (storage.takes_dir.join("NativeRate"), true),
    ] {
        let meta = match fs::symlink_metadata(&dir) {
            Ok(meta) => meta,
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(_) => { issues += 1; continue; }
        };
        if meta.file_type().is_symlink() || !meta.is_dir() {
            issues += 1; continue;
        }
        let iter = match fs::read_dir(&dir) {
            Ok(iter) => iter,
            Err(_) => { issues += 1; continue; }
        };
        for (index, entry) in iter.enumerate() {
            if index >= 2048 { issues += 1; break; }
            let entry = match entry { Ok(e) => e, Err(_) => { issues += 1; continue } };
            let name = entry.file_name().to_string_lossy().to_string();
            if !is_native && name == "NativeRate" { continue; }
            // Both active and prior native names may be reported as orphan
            // above, and are not double-counted here.
            if is_native && prompts.iter().any(|(id,_)| {
                let prefix = format!("take_{id:04}.wav");
                name == prefix || name == format!("{prefix}.previous")
            }) { continue; }
            let known = sources.iter().any(|s|
                if is_native { s.native.as_ref() == Some(&entry.path()) }
                else { s.path == entry.path() }
            );
            if !known { issues += 1; }
        }
    }
    (sources, issues)
}

pub(crate) fn legacy_protection_status(
    storage: &VoiceLabStoragePaths, prompts: &[(u32, &str)],
) -> LegacyProtectionStatus {
    let (sources, issues) = legacy_candidates(storage, prompts);
    let mut report = LegacyProtectionStatus {
        total_sources: sources.len(), protected_sources: 0,
        unprotected_sources: 0, attention_sources: issues,
    };
    for item in sources {
        match guided_take_is_archived(storage, item.id, item.text, &item.path, item.native.as_deref()) {
            Ok(true) => report.protected_sources += 1,
            Ok(false) => report.unprotected_sources += 1,
            Err(_) => report.attention_sources += 1,
        }
    }
    report
}

/// A failed item remains at its original location, with attention reflected
/// in the post-migration read-only status. No source deletion or path mutation.
pub(crate) fn migrate_legacy_recordings(
    storage: &VoiceLabStoragePaths, prompts: &[(u32, &str)],
) -> LegacyMigrationResult {
    let (sources, _) = legacy_candidates(storage, prompts);
    let mut newly_archived = 0;
    for item in sources {
        if matches!(
            guided_take_is_archived(storage, item.id, item.text, &item.path, item.native.as_deref()),
            Ok(true)
        ) {
            continue;
        }
        if archive_guided_take(storage, item.id, item.text, &item.path, item.native.as_deref()).is_ok() {
            newly_archived += 1;
        }
    }
    let report = legacy_protection_status(storage, prompts);
    let ok = report.unprotected_sources == 0 && report.attention_sources == 0;
    let message = if ok {
        "Existing recordings are verified in local saved storage. Original files were kept."
    } else {
        "Some older recording files still need attention. No source files were removed."
    };
    LegacyMigrationResult { ok, newly_archived, report, message: message.into() }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};
    const PROMPTS: &[(u32, &str)] = &[(1, "Good morning, everyone."), (2, "Thanks for joining today.")];
    fn wav(seed: u8) -> Vec<u8> {
        let payload = vec![seed; 64_000];
        let mut v=Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36_u32 + payload.len() as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16_u32.to_le_bytes());
        v.extend_from_slice(&1_u16.to_le_bytes());
        v.extend_from_slice(&1_u16.to_le_bytes());
        v.extend_from_slice(&32_000_u32.to_le_bytes());
        v.extend_from_slice(&64_000_u32.to_le_bytes());
        v.extend_from_slice(&2_u16.to_le_bytes());
        v.extend_from_slice(&16_u16.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(&payload);
        v
    }
    #[test]
    fn migration_is_idempotent_and_preserves_both_legacy_versions() {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-legacy-migration-{}-{t}", std::process::id()));
        let store = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(&store.takes_dir).unwrap();
        let active = store.takes_dir.join("take_0001.wav");
        let old = active.with_extension("wav.previous");
        fs::write(&active, wav(11)).unwrap();
        fs::write(&old, wav(22)).unwrap();
        let first = migrate_legacy_recordings(&store, PROMPTS);
        assert!(first.ok);
        assert_eq!(first.newly_archived, 2);
        let second = migrate_legacy_recordings(&store, PROMPTS);
        assert!(second.ok);
        assert_eq!(second.newly_archived, 0);
        assert_eq!(second.report.protected_sources, 2);
        assert_eq!(fs::read(active).unwrap(), wav(11));
        assert_eq!(fs::read(old).unwrap(), wav(22));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn damaged_source_or_orphan_sidecar_stays_visible_as_attention() {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("translateit-legacy-migration-bad-{}-{t}", std::process::id()));
        let store = VoiceLabStoragePaths::from_roots(&root.join("cache"), &root.join("saved"));
        fs::create_dir_all(store.takes_dir.join("NativeRate")).unwrap();
        fs::write(store.takes_dir.join("take_0001.wav"), b"not wave").unwrap();
        fs::write(store.takes_dir.join("NativeRate/take_0002.wav"), wav(33)).unwrap();
        let result = migrate_legacy_recordings(&store, PROMPTS);
        assert!(!result.ok);
        assert!(result.report.attention_sources > 0);
        assert!(store.takes_dir.join("take_0001.wav").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
