use std::fs;
use std::path::Path;

pub(super) fn discard_review_take(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path)
        .map_err(|error| format!("VoiceLab could not remove the review take safely: {error}"))
}

pub(super) fn accept_review_take(draft: &Path, target: &Path) -> Result<(), String> {
    let previous = target.with_extension("wav.previous");

    if previous.exists() && !target.exists() {
        fs::rename(&previous, target).map_err(|error| {
            format!(
                "VoiceLab found an interrupted previous-take backup but could not restore it safely: {error}"
            )
        })?;
    }

    if previous.exists() && target.exists() {
        fs::remove_file(&previous).map_err(|error| {
            format!("VoiceLab could not clear the stale previous-take backup safely: {error}")
        })?;
    }

    if target.exists() {
        fs::rename(target, &previous).map_err(|error| {
            format!("VoiceLab could not preserve the previous accepted take: {error}")
        })?;
    }

    if let Err(error) = fs::rename(draft, target) {
        if previous.exists() && !target.exists() {
            if let Err(rollback_error) = fs::rename(&previous, target) {
                return Err(format!(
                    "VoiceLab could not accept this take: {error}; previous accepted take rollback also failed: {rollback_error}"
                ));
            }
        }
        return Err(format!("VoiceLab could not accept this take: {error}"));
    }

    if previous.exists() {
        let _ = fs::remove_file(previous);
    }
    Ok(())
}


/// Preserve a device-native-rate mono sidecar alongside the canonical 32 kHz
/// take. GPT-SoVITS continues to consume ONLY the canonical take. This helper
/// rolls back ordinary file-operation failures; interrupted power-loss pairs
/// must be integrity-checked before any future enhancement reads the sidecar.
pub(super) fn accept_review_take_with_native_rate(
    draft: &Path,
    target: &Path,
    native_draft: &Path,
    native_target: &Path,
) -> Result<(), String> {
    if !draft.is_file() || !native_draft.is_file() {
        return Err("VoiceLab cannot accept the take without both review and native-rate drafts.".to_string());
    }
    let parent = native_target.parent().ok_or("VoiceLab native-rate take directory is invalid.")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("VoiceLab could not prepare native-rate take storage: {error}"))?;

    let previous = native_target.with_extension("wav.previous");
    if previous.exists() && !native_target.exists() {
        fs::rename(&previous, native_target).map_err(|error| {
            format!("VoiceLab could not recover an interrupted native-rate backup: {error}")
        })?;
    }
    // Never delete an ambiguous backup: it may be the only remaining source
    // after an interrupted accept. Canonical training remains unaffected.
    if previous.exists() {
        return Err("VoiceLab native-rate backup needs recovery before replacing this take.".to_string());
    }
    if native_target.exists() && !native_target.is_file() {
        return Err("VoiceLab native-rate destination is not a file.".to_string());
    }

    let had_previous = native_target.is_file();
    if had_previous {
        fs::rename(native_target, &previous).map_err(|error| {
            format!("VoiceLab could not preserve the previous native-rate take: {error}")
        })?;
    }
    if let Err(error) = fs::rename(native_draft, native_target) {
        if had_previous {
            fs::rename(&previous, native_target).map_err(|restore| {
                format!(
                    "VoiceLab could not save the native-rate take: {error}; previous native-rate rollback failed: {restore}"
                )
            })?;
        }
        return Err(format!("VoiceLab could not save the native-rate take: {error}"));
    }
    if let Err(error) = accept_review_take(draft, target) {
        let draft_restored = fs::rename(native_target, native_draft);
        let previous_restored = if had_previous {
            fs::rename(&previous, native_target)
        } else {
            Ok(())
        };
        if draft_restored.is_err() || previous_restored.is_err() {
            return Err(format!(
                "{error}; native-rate rollback incomplete (draft={}, previous={}).",
                draft_restored.is_ok(),
                previous_restored.is_ok(),
            ));
        }
        return Err(error);
    }
    if had_previous {
        // Same best-effort stale-backup cleanup as the canonical take owner.
        // The new canonical/native pair has already been accepted.
        let _ = fs::remove_file(previous);
    }
    Ok(())
}

#[cfg(test)]
mod native_rate_take_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static RUN: AtomicU64 = AtomicU64::new(0);

    fn fixture(label: &str) -> std::path::PathBuf {
        let token = RUN.fetch_add(1, Ordering::Relaxed);
        let folder = std::env::temp_dir().join(format!(
            "translateit_native_take_{}_{}_{}",
            label,
            std::process::id(),
            token
        ));
        fs::create_dir_all(&folder).expect("create fixture");
        folder
    }

    #[test]
    fn accepts_both_sources_and_replaces_a_previous_pair() {
        let root = fixture("accept");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_target, b"old native").unwrap();
        accept_review_take_with_native_rate(&draft, &target, &native_draft, &native_target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"new native");
        assert!(!native_target.with_extension("wav.previous").exists());
        assert!(!draft.exists());
        assert!(!native_draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_missing_native_draft_without_changing_accepted_take() {
        let root = fixture("missing");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        assert!(accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target
        ).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert!(draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn failure_of_canonical_accept_rolls_back_both_native_files() {
        let root = fixture("rollback");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("NativeRate").join("take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::create_dir_all(native_target.parent().unwrap()).unwrap();
        fs::write(&native_target, b"old native").unwrap();
        // A directory at the canonical rollback-file path makes the existing
        // atomic canonical accept fail after staging the native replacement.
        fs::create_dir(target.with_extension("wav.previous")).unwrap();
        assert!(accept_review_take_with_native_rate(
            &draft, &target, &native_draft, &native_target
        ).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"old native");
        assert_eq!(fs::read(&native_draft).unwrap(), b"new native");
        assert!(draft.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn discarding_unaccepted_pair_does_not_touch_existing_accepted_pair() {
        let root = fixture("discard");
        let draft = root.join("draft.wav");
        let target = root.join("take.wav");
        let native_draft = root.join("native_draft.wav");
        let native_target = root.join("native_take.wav");
        fs::write(&draft, b"new canonical").unwrap();
        fs::write(&native_draft, b"new native").unwrap();
        fs::write(&target, b"old canonical").unwrap();
        fs::write(&native_target, b"old native").unwrap();
        discard_review_take(&native_draft).unwrap();
        discard_review_take(&draft).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"old canonical");
        assert_eq!(fs::read(&native_target).unwrap(), b"old native");
        fs::remove_dir_all(root).unwrap();
    }
}
