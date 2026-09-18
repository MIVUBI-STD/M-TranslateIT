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
