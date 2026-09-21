use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use crate::engine::paths::ProjectPaths;

#[derive(Debug, Clone, Serialize)]
pub struct ModelInventoryItem {
    pub model_id: String,
    pub required: bool,
    pub expected_path: String,
    pub found: bool,
    pub file_count: usize,
    pub size_bytes: u64,
    pub gpu_capable: String,
    pub cpu_fallback: bool,
    pub download_url: Option<String>,
    pub status: String,
    pub blocker: Option<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInventoryReport {
    pub ok: bool,
    pub scope: String,
    pub status: String,
    pub created_at: String,
    pub items: Vec<ModelInventoryItem>,
    pub blockers: Vec<String>,
    pub note: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ModelManifest {
    schema: String,
    inventory_scope: String,
    required_field_semantics: String,
    meeting_readiness_owner: String,
    models: Vec<ModelManifestEntry>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct ModelManifestEntry {
    model_id: String,
    required: bool,
    expected_path: String,
    gpu_capable: Option<bool>,
    cpu_fallback: Option<bool>,
    download_url: Option<String>,
}

fn now_iso() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", now.as_secs())
}

fn validation_write_path(project_paths: &ProjectPaths, file_name: &str) -> PathBuf {
    PathBuf::from(&project_paths.user_cache_dir)
        .join("validation")
        .join(file_name)
}

fn manifest_contract_valid(manifest: &ModelManifest) -> bool {
    manifest.schema == "translateit.local_model_inventory.v2"
        && manifest.inventory_scope == "full_product_release_assets"
        && manifest.required_field_semantics == "required_for_full_product_release"
        && manifest.meeting_readiness_owner == "realtime_local_worker.status"
}

fn read_model_manifest(path: &Path) -> Option<ModelManifest> {
    let text = fs::read_to_string(path).ok()?;
    let manifest = serde_json::from_str::<ModelManifest>(&text).ok()?;
    manifest_contract_valid(&manifest).then_some(manifest)
}

fn count_files(root: &Path) -> (usize, u64) {
    let mut count = 0usize;
    let mut bytes = 0u64;
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return (0, 0),
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() {
            count += 1;
            bytes += path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        }
    }
    (count, bytes)
}

fn build_model_inventory(
    project_paths: &ProjectPaths,
) -> (Vec<ModelInventoryItem>, Vec<String>, String) {
    let runtime_root = PathBuf::from(&project_paths.runtime_root);
    let manifest_path =
        PathBuf::from(&project_paths.worker_runtime_dir).join("model_manifest.json");
    let manifest = read_model_manifest(&manifest_path);
    let mut blockers = Vec::new();
    if manifest.is_none() {
        blockers.push("model_inventory:release_manifest_missing_or_invalid".to_string());
    }

    let mut items = Vec::new();
    if let Some(manifest) = manifest {
        for entry in manifest.models {
            let expected_path = runtime_root.join(&entry.expected_path);
            let found = expected_path.is_dir() || expected_path.is_file();
            let (file_count, size_bytes) = if expected_path.is_dir() {
                count_files(&expected_path)
            } else if expected_path.is_file() {
                (1, expected_path.metadata().map(|m| m.len()).unwrap_or(0))
            } else {
                (0, 0)
            };
            let gpu_capable = entry
                .gpu_capable
                .map(|value| if value { "true" } else { "false" }.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let cpu_fallback = entry.cpu_fallback.unwrap_or(false);
            let metadata_incomplete = entry.download_url.is_none();
            let status = if found {
                "installed"
            } else if entry.required {
                "missing_release_required"
            } else if metadata_incomplete {
                "missing_optional_metadata_incomplete"
            } else {
                "missing_optional"
            };
            let blocker = if !found && entry.required {
                Some(format!("missing_release_model:{}", entry.model_id))
            } else {
                None
            };
            let next_action = if found {
                "Installed asset path detected. Runtime load/inference must be checked separately."
                    .to_string()
            } else {
                format!(
                    "Install {} at {} through the approved runtime/release asset flow.",
                    entry.model_id, entry.expected_path
                )
            };

            if entry.required && !found {
                blockers.push(format!("missing_release_model:{}", entry.model_id));
            }

            items.push(ModelInventoryItem {
                model_id: entry.model_id,
                required: entry.required,
                expected_path: entry.expected_path,
                found,
                file_count,
                size_bytes,
                gpu_capable,
                cpu_fallback,
                download_url: entry.download_url,
                status: status.to_string(),
                blocker,
                next_action,
            });
        }
    }

    let status = if blockers.is_empty() {
        "release_assets_present"
    } else if blockers
        .iter()
        .any(|blocker| blocker.starts_with("missing_release_model:"))
    {
        "missing_release_assets"
    } else {
        "release_inventory_unavailable"
    };
    (items, blockers, status.to_string())
}

fn build_model_inventory_report() -> ModelInventoryReport {
    let project_paths = ProjectPaths::discover();
    let (items, blockers, status) = build_model_inventory(&project_paths);
    ModelInventoryReport {
        ok: blockers.is_empty(),
        scope: "full_product_release_assets".to_string(),
        status,
        created_at: now_iso(),
        items,
        blockers: blockers.clone(),
        note: if blockers.is_empty() {
            "All assets required by the full-product-release model manifest are present. Meeting Start readiness is owned separately by current worker/provider capability; runtime load, inference, quality, latency, and device use still require separate proof."
                .to_string()
        } else {
            "One or more assets required for full product release are missing. This release inventory does not decide whether the current Meeting outbound runtime can start."
                .to_string()
        },
    }
}

fn write_validation_json(project_paths: &ProjectPaths, file_name: &str, value: &impl Serialize) {
    let path = validation_write_path(project_paths, file_name);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(value) {
        let _ = fs::write(path, text);
    }
}

pub fn verify_models() -> ModelInventoryReport {
    let report = build_model_inventory_report();
    let project_paths = ProjectPaths::discover();
    write_validation_json(&project_paths, "latest_model_inventory.json", &report);
    report
}
#[cfg(test)]
mod a7_release_inventory_tests {
    use super::*;

    #[test]
    fn manifest_contract_is_explicitly_release_scoped() {
        let manifest: ModelManifest = serde_json::from_str(
            r#"{
                "schema":"translateit.local_model_inventory.v2",
                "inventory_scope":"full_product_release_assets",
                "required_field_semantics":"required_for_full_product_release",
                "meeting_readiness_owner":"realtime_local_worker.status",
                "models":[]
            }"#,
        )
        .expect("A7 release manifest contract");
        assert!(manifest_contract_valid(&manifest));
    }

    #[test]
    fn legacy_or_meeting_scoped_manifest_is_not_canonical_release_inventory() {
        let legacy: ModelManifest = serde_json::from_str(
            r#"{
                "schema":"translateit.local_model_inventory.v1",
                "inventory_scope":"meeting_start",
                "required_field_semantics":"required_for_meeting",
                "meeting_readiness_owner":"model_manifest",
                "models":[]
            }"#,
        )
        .expect("legacy-shaped manifest fixture");
        assert!(!manifest_contract_valid(&legacy));
    }
}
