//! Training status reconciliation. A Quick Preview never consumes a previous
//! full-training child status or changes its active resource phase.
use super::child_status;
use super::super::voice_lab::{
    current_voice_lab_build_snapshot, mark_voice_lab_build_evaluating,
    mark_voice_lab_build_training, VoiceLabStoragePaths,
};

pub(super) fn reconcile_phase(paths: &VoiceLabStoragePaths) {
    // The preview borrows the VoiceLab resource but is not a trained build.
    // Its phase must never be changed by a previous build's status.json.
    if super::super::voice_lab_preview::quick_voice_preview_active() {
        return;
    }
    let snapshot = current_voice_lab_build_snapshot();
    let Some(generation) = snapshot.generation else {
        return;
    };
    if !snapshot.active || snapshot.phase == "cancelling" {
        return;
    }
    let Some(status) = child_status(paths) else {
        return;
    };
    match (snapshot.phase.as_str(), status.phase.as_str()) {
        ("preparing", "training") => {
            let _ = mark_voice_lab_build_training(generation);
        }
        ("preparing", "evaluating") | ("preparing", "ready_for_review") => {
            if mark_voice_lab_build_training(generation).is_ok() {
                let _ = mark_voice_lab_build_evaluating(generation);
            }
        }
        ("training", "evaluating") | ("training", "ready_for_review") => {
            let _ = mark_voice_lab_build_evaluating(generation);
        }
        _ => {}
    }
}
