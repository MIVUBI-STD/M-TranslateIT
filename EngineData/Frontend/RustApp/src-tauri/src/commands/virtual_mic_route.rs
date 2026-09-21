use serde::Serialize;
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::latest_runtime_session_state;

use super::audio::{list_audio_devices, AudioDeviceSummary};
use super::helper_bridge_runtime::unix_ms;

const APPLICATION_MEETING_OWNER_ID: &str = "translateit_application_meeting";

#[derive(Debug, Clone, Serialize)]
pub struct VirtualMicRouteContractStatus {
    pub ok: bool,
    pub route_ready: bool,
    pub route_pair_id: Option<String>,
    pub selected_output_device: Option<String>,
    pub selected_input_device: Option<String>,
    pub output_device_found: bool,
    pub input_device_found: bool,
    pub evidence_path: Option<String>,
    pub route_output_contract_json: String,
    pub available_output_devices: Vec<String>,
    pub available_input_devices: Vec<String>,
    pub blocker: String,
    pub next_action: String,
    pub runtime_claim: String,
    pub updated_unix_ms: u128,
}

#[derive(Debug, Clone)]
struct MeetingVirtualMicRouteSelection {
    generation: Option<u64>,
    pair_id: String,
    output_device: String,
    input_device: String,
}

#[derive(Debug, Clone)]
struct RoutePairCandidate {
    pair_id: String,
    output_device: String,
    input_device: String,
}

#[derive(Debug, Clone)]
struct RouteSelection {
    route_pair_id: Option<String>,
    selected_output_device: Option<String>,
    selected_input_device: Option<String>,
    output_device_found: bool,
    input_device_found: bool,
    blocker: String,
}

static MEETING_VIRTUAL_MIC_ROUTE_SELECTION: OnceLock<
    Mutex<Option<MeetingVirtualMicRouteSelection>>,
> = OnceLock::new();

fn selection_runtime() -> &'static Mutex<Option<MeetingVirtualMicRouteSelection>> {
    MEETING_VIRTUAL_MIC_ROUTE_SELECTION.get_or_init(|| Mutex::new(None))
}

fn normalized_path_label(path: &std::path::Path) -> String {
    path.to_string_lossy().replace(char::from(92), "/")
}

fn route_evidence_path() -> PathBuf {
    let paths = ProjectPaths::discover();
    PathBuf::from(paths.user_log_dir)
        .join("RustAppValidation")
        .join("latest_virtual_mic_route_evidence.json")
}

fn device_names(devices: &[AudioDeviceSummary]) -> Vec<String> {
    devices.iter().map(|device| device.name.clone()).collect()
}

fn named_device_exists(devices: &[AudioDeviceSummary], name: &str) -> bool {
    devices.iter().any(|device| device.name == name)
}

fn supported_virtual_pair_provider(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    normalized.contains("vb-audio")
        || normalized.contains("voicemeeter")
        || normalized.contains("virtual cable")
}

fn endpoint_pair_identity(name: &str, expected_role: &str) -> Option<String> {
    if !supported_virtual_pair_provider(name) {
        return None;
    }
    let normalized = name.to_ascii_lowercase();
    let tokens = normalized
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>();
    if !tokens.contains(&expected_role) {
        return None;
    }
    let identity = tokens
        .into_iter()
        .filter(|token| !matches!(*token, "input" | "output"))
        .collect::<Vec<_>>()
        .join("-");
    (!identity.is_empty()).then_some(identity)
}

fn matched_pair_identity(output_device: &str, input_device: &str) -> Option<String> {
    // Virtual-cable naming is role-inverted from TranslateIT's point of view:
    // TTS is rendered to the cable playback-side "Input" endpoint and the meeting
    // application selects the paired recording-side "Output" endpoint as microphone.
    let output_identity = endpoint_pair_identity(output_device, "input")?;
    let input_identity = endpoint_pair_identity(input_device, "output")?;
    (output_identity == input_identity).then(|| format!("windows_virtual_pair:{output_identity}"))
}

fn matched_pair_candidates(
    output_devices: &[AudioDeviceSummary],
    input_devices: &[AudioDeviceSummary],
) -> Vec<RoutePairCandidate> {
    let mut candidates = Vec::new();
    for output in output_devices {
        for input in input_devices {
            let Some(pair_id) = matched_pair_identity(&output.name, &input.name) else {
                continue;
            };
            if candidates
                .iter()
                .any(|candidate: &RoutePairCandidate| candidate.pair_id == pair_id)
            {
                continue;
            }
            candidates.push(RoutePairCandidate {
                pair_id,
                output_device: output.name.clone(),
                input_device: input.name.clone(),
            });
        }
    }
    candidates.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    candidates
}

fn primary_vb_cable_pair(candidate: &RoutePairCandidate) -> bool {
    let output = candidate.output_device.to_ascii_lowercase();
    let input = candidate.input_device.to_ascii_lowercase();
    output.starts_with("cable input")
        && input.starts_with("cable output")
        && output.contains("vb-audio virtual cable")
        && input.contains("vb-audio virtual cable")
}

fn select_route_pair(
    output_devices: &[AudioDeviceSummary],
    input_devices: &[AudioDeviceSummary],
) -> RouteSelection {
    let candidates = matched_pair_candidates(output_devices, input_devices);
    let primary = candidates
        .iter()
        .filter(|candidate| primary_vb_cable_pair(candidate))
        .collect::<Vec<_>>();
    let selected = if primary.len() == 1 {
        primary.first().copied()
    } else if candidates.len() == 1 {
        candidates.first()
    } else {
        None
    };

    if let Some(candidate) = selected {
        return RouteSelection {
            route_pair_id: Some(candidate.pair_id.clone()),
            selected_output_device: Some(candidate.output_device.clone()),
            selected_input_device: Some(candidate.input_device.clone()),
            output_device_found: true,
            input_device_found: true,
            blocker: String::new(),
        };
    }

    RouteSelection {
        route_pair_id: None,
        selected_output_device: None,
        selected_input_device: None,
        output_device_found: false,
        input_device_found: false,
        blocker: if candidates.is_empty() {
            "virtual_mic:matched_route_pair_missing".to_string()
        } else {
            "virtual_mic:matched_route_pair_ambiguous".to_string()
        },
    }
}

fn active_application_meeting_generation() -> Option<u64> {
    latest_runtime_session_state()
        .snapshot
        .filter(|snapshot| snapshot.owner_id == APPLICATION_MEETING_OWNER_ID)
        .map(|snapshot| snapshot.generation)
}

fn current_meeting_selection() -> Option<MeetingVirtualMicRouteSelection> {
    selection_runtime()
        .lock()
        .ok()
        .and_then(|value| value.clone())
}

fn route_output_contract_json(status: &VirtualMicRouteContractStatus) -> String {
    serde_json::to_string_pretty(&json!({
        "schema": "translateit.virtual_route.output_contract.v3",
        "source_audio_owner": "realtime_local_worker.synthesize",
        "delivery_owner": "rust_windows_audio",
        "route_pair_id": &status.route_pair_id,
        "selected_output_device": &status.selected_output_device,
        "meeting_application_microphone_device": &status.selected_input_device,
        "route_ready": status.route_ready,
        "blocker": &status.blocker,
        "runtime_claim": "matched_virtual_route_pair_source_side_not_audio_delivery_proof"
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

fn write_route_evidence(status: &VirtualMicRouteContractStatus) -> Option<String> {
    let path = route_evidence_path();
    fs::create_dir_all(path.parent()?).ok()?;
    let body = serde_json::to_string_pretty(&json!({
        "schema": "translateit.virtual_route.evidence.v3",
        "status": status,
        "runtime_claim": "matched_virtual_route_evidence_source_side_not_audio_delivery_proof",
        "written_unix_ms": unix_ms()
    }))
    .ok()?;
    fs::write(&path, body).ok()?;
    Some(normalized_path_label(&path))
}

fn next_action_for_blocker(blocker: &str) -> String {
    if blocker.is_empty() {
        "use_prepared_route_for_native_meeting_output".to_string()
    } else if blocker.contains("ambiguous") {
        "leave_only_one_supported_virtual_audio_cable_pair_enabled".to_string()
    } else if blocker.contains("selected") || blocker.contains("generation") {
        "restore_prepared_virtual_audio_route_pair".to_string()
    } else {
        "install_or_enable_one_supported_virtual_audio_cable_pair".to_string()
    }
}

fn status_from_selection(
    available_output_devices: Vec<String>,
    available_input_devices: Vec<String>,
    selection: RouteSelection,
    runtime_claim: &str,
) -> VirtualMicRouteContractStatus {
    let route_ready = selection.blocker.is_empty()
        && selection.route_pair_id.is_some()
        && selection.output_device_found
        && selection.input_device_found;
    let blocker = selection.blocker;
    let mut status = VirtualMicRouteContractStatus {
        ok: route_ready,
        route_ready,
        route_pair_id: selection.route_pair_id,
        selected_output_device: selection.selected_output_device,
        selected_input_device: selection.selected_input_device,
        output_device_found: selection.output_device_found,
        input_device_found: selection.input_device_found,
        evidence_path: None,
        route_output_contract_json: "{}".to_string(),
        available_output_devices,
        available_input_devices,
        next_action: next_action_for_blocker(&blocker),
        blocker,
        runtime_claim: runtime_claim.to_string(),
        updated_unix_ms: unix_ms(),
    };
    status.route_output_contract_json = route_output_contract_json(&status);
    status
}

fn with_route_evidence(mut status: VirtualMicRouteContractStatus) -> VirtualMicRouteContractStatus {
    status.evidence_path = write_route_evidence(&status);
    status
}

fn build_dynamic_status() -> VirtualMicRouteContractStatus {
    let devices = list_audio_devices();
    let selection = select_route_pair(&devices.output_devices, &devices.input_devices);
    status_from_selection(
        device_names(&devices.output_devices),
        device_names(&devices.input_devices),
        selection,
        "virtual_mic_matched_route_pair_selection_source_side_not_audio_delivery_proof",
    )
}

fn build_fixed_status(
    fixed: &MeetingVirtualMicRouteSelection,
    runtime_claim: &str,
) -> VirtualMicRouteContractStatus {
    let devices = list_audio_devices();
    let output_device_found = named_device_exists(&devices.output_devices, &fixed.output_device);
    let input_device_found = named_device_exists(&devices.input_devices, &fixed.input_device);
    let pair_still_matches = matched_pair_identity(&fixed.output_device, &fixed.input_device)
        .as_deref()
        == Some(fixed.pair_id.as_str());
    let blocker = if !output_device_found {
        "virtual_mic:selected_output_device_missing".to_string()
    } else if !input_device_found {
        "virtual_mic:selected_input_device_missing".to_string()
    } else if !pair_still_matches {
        "virtual_mic:selected_route_pair_mismatch".to_string()
    } else {
        String::new()
    };
    status_from_selection(
        device_names(&devices.output_devices),
        device_names(&devices.input_devices),
        RouteSelection {
            route_pair_id: Some(fixed.pair_id.clone()),
            selected_output_device: Some(fixed.output_device.clone()),
            selected_input_device: Some(fixed.input_device.clone()),
            output_device_found,
            input_device_found,
            blocker,
        },
        runtime_claim,
    )
}

fn unbound_active_meeting_status(generation: u64) -> VirtualMicRouteContractStatus {
    let devices = list_audio_devices();
    status_from_selection(
        device_names(&devices.output_devices),
        device_names(&devices.input_devices),
        RouteSelection {
            route_pair_id: None,
            selected_output_device: None,
            selected_input_device: None,
            output_device_found: false,
            input_device_found: false,
            blocker: format!("virtual_mic:meeting_route_generation_not_bound:{generation}"),
        },
        "virtual_mic_active_meeting_route_not_bound_fail_closed",
    )
}

pub fn clear_prepared_virtual_mic_route_selection() {
    if let Ok(mut selection) = selection_runtime().lock() {
        *selection = None;
    }
}

pub fn prepare_current_virtual_mic_route_for_meeting() -> Result<(), String> {
    let route = build_dynamic_status();
    if !route.route_ready {
        return Err(if route.blocker.is_empty() {
            "virtual_mic:matched_route_pair_not_ready".to_string()
        } else {
            route.blocker
        });
    }
    let pair_id = route
        .route_pair_id
        .ok_or_else(|| "virtual_mic:matched_route_pair_identity_missing".to_string())?;
    let output_device = route
        .selected_output_device
        .ok_or_else(|| "virtual_mic:matched_route_output_missing".to_string())?;
    let input_device = route
        .selected_input_device
        .ok_or_else(|| "virtual_mic:matched_route_input_missing".to_string())?;
    let mut selection = selection_runtime()
        .lock()
        .map_err(|_| "virtual_mic:meeting_route_selection_lock_failed".to_string())?;
    *selection = Some(MeetingVirtualMicRouteSelection {
        generation: None,
        pair_id,
        output_device,
        input_device,
    });
    Ok(())
}

pub fn bind_prepared_virtual_mic_route_to_generation(generation: u64) -> Result<(), String> {
    if active_application_meeting_generation() != Some(generation) {
        return Err("virtual_mic:meeting_route_bind_generation_not_active".to_string());
    }
    let mut selection = selection_runtime()
        .lock()
        .map_err(|_| "virtual_mic:meeting_route_selection_lock_failed".to_string())?;
    let Some(prepared) = selection.as_mut() else {
        return Err("virtual_mic:prepared_route_pair_missing".to_string());
    };
    match prepared.generation {
        Some(existing) if existing != generation => {
            Err("virtual_mic:prepared_route_pair_bound_to_other_generation".to_string())
        }
        _ => {
            prepared.generation = Some(generation);
            Ok(())
        }
    }
}

pub fn get_bound_virtual_mic_output_device(generation: u64) -> Result<String, String> {
    if active_application_meeting_generation() != Some(generation) {
        return Err("virtual_mic:meeting_route_generation_not_active".to_string());
    }
    let mut selection = selection_runtime()
        .lock()
        .map_err(|_| "virtual_mic:meeting_route_selection_lock_failed".to_string())?;
    let Some(prepared) = selection.as_mut() else {
        return Err("virtual_mic:prepared_route_pair_missing".to_string());
    };
    match prepared.generation {
        Some(existing) if existing != generation => {
            Err("virtual_mic:prepared_route_pair_bound_to_other_generation".to_string())
        }
        _ => {
            prepared.generation = Some(generation);
            Ok(prepared.output_device.clone())
        }
    }
}

pub fn get_virtual_mic_route_selection() -> VirtualMicRouteContractStatus {
    if let Some(generation) = active_application_meeting_generation() {
        if let Ok(mut selection) = selection_runtime().lock() {
            if let Some(prepared) = selection.as_mut() {
                if prepared.generation.is_none() {
                    prepared.generation = Some(generation);
                }
                if prepared.generation == Some(generation) {
                    let fixed = prepared.clone();
                    drop(selection);
                    return build_fixed_status(
                        &fixed,
                        "virtual_mic_generation_bound_route_pair_source_side_not_audio_delivery_proof",
                    );
                }
            }
        }
        return unbound_active_meeting_status(generation);
    }

    if let Some(prepared) = current_meeting_selection() {
        if prepared.generation.is_none() {
            return build_fixed_status(
                &prepared,
                "virtual_mic_prepared_route_pair_source_side_not_audio_delivery_proof",
            );
        }
    }
    build_dynamic_status()
}

#[tauri::command]
pub fn get_virtual_mic_route_contract_status() -> VirtualMicRouteContractStatus {
    with_route_evidence(get_virtual_mic_route_selection())
}

#[cfg(test)]
mod b3_route_hot_path_tests {
    use super::{status_from_selection, RouteSelection};

    #[test]
    fn internal_route_status_does_not_write_evidence() {
        let status = status_from_selection(
            Vec::new(),
            Vec::new(),
            RouteSelection {
                route_pair_id: None,
                selected_output_device: None,
                selected_input_device: None,
                output_device_found: false,
                input_device_found: false,
                blocker: "virtual_mic:matched_route_pair_missing".to_string(),
            },
            "b3_internal_route_status",
        );

        assert!(status.evidence_path.is_none());
        assert!(!status.route_ready);
    }
}
