import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rust = (name: string) => readFileSync(resolve(root, "src-tauri/src/commands/" + name), "utf8");
const frontend = (name: string) => readFileSync(resolve(root, "src/" + name), "utf8");

test("version browsing performs only small-receipt discovery and warns that full verification is required", () => {
  const archive = rust("voice_lab/storage/actor_history.rs");
  const page = frontend("components/my-voice/VoiceVersionBrowser.svelte");
  const app = frontend("pages/MyVoice.svelte");
  assert.match(archive, /const PAGE_SIZE: usize = 12/);
  assert.match(archive, /verification_required: true/);
  assert.match(archive, /pub\(crate\) fn actor_version_page/);
  assert.match(page, /Full model integrity will be verified before use/);
  assert.match(page, /selectionUnknown = true/);
  assert.match(page, /No saved personal voice versions found yet/);
  assert.match(app, /<VoiceVersionBrowser/);
});

test("restoring a previous actor requires an explicit confirmation and native resource guards", () => {
  const build = rust("voice_lab_build.rs");
  const store = rust("voice_lab/storage.rs");
  const actor = rust("voice_lab/storage/actor_history.rs");
  const mutations = rust("application_runtime/mutations.rs");
  const registered = rust("registry.rs");
  const api = frontend("app/bridge/myVoiceBuildApi.ts");
  const browser = frontend("components/my-voice/VoiceVersionBrowser.svelte");
  assert.match(build, /pub fn select_archived_personal_voice_version/);
  assert.match(build, /meeting_blocks_voice_change\(\)/);
  assert.match(build, /voice_lab_build_blocks_meeting\(\)/);
  assert.match(build, /voice_lab_recording_active\(\)/);
  assert.match(build, /quick_voice_preview_active\(\)/);
  assert.match(build, /voice_selection_guard\(\)/);
  assert.match(build, /if !replace_confirmed/);
  assert.match(store, /pub\(crate\) fn activate_archived_personal_voice/);
  assert.match(store, /actor_history::preserve_actor_version\(paths, selected\)/);
  assert.match(store, /version_restore_recovery_required/);
  assert.match(actor, /prepare_version_for_activation/);
  assert.match(actor, /verify_actor_version\(storage, id\)/);
  assert.match(actor, /version_restore_integrity_failed/);
  assert.match(mutations, /emit_after\(&app, "select_voice_version"/);
  assert.match(registered, /get_voice_lab_actor_versions/);
  assert.match(registered, /select_product_voice_version/);
  assert.match(api, /select_product_voice_version/);
  assert.match(browser, /Confirm and use/);
  assert.match(browser, /approval_required/);
  assert.match(browser, /selectionUnknown = true/);
  assert.match(browser, /Retry voice status/);
  assert.match(browser, /onRecheck/);
});

test("stored versions are never deleted or silently promoted by the listing module", () => {
  const source = rust("voice_lab/storage/actor_history.rs").split("#[cfg(test)]")[0];
  assert.doesNotMatch(source, /remove_file\(|remove_dir_all\(/);
  assert.match(source, /create_dir\(&pending\)/);
  assert.match(source, /sha256/);
  assert.doesNotMatch(source, /get_application_snapshot/);
});

test("uncertain saved-version restore keeps selection locked and rechecks native voice without retrying the write", () => {
  const browser = frontend("components/my-voice/VoiceVersionBrowser.svelte");
  const page = frontend("pages/MyVoice.svelte");
  const reconcile = browser.slice(browser.indexOf("async function reconcileUncertainSelection("),
    browser.indexOf("async function selectVersion("));
  assert.match(reconcile, /pendingId = null/);
  assert.match(reconcile, /selectionUnknown = true/);
  assert.match(reconcile, /await onRecheck\(versionId\)/);
  assert.doesNotMatch(reconcile, /selectionUnknown = false|selectActorVersion\(/);
  assert.match(browser, /if \(!next\) \{\s*error = true;\s*pendingId = null/);
  assert.match(browser, /pendingId && !versions\.items\.some/);
  assert.match(browser, /if \(busy\) return;\s*opened = !opened/);
  assert.match(browser, /aria-expanded=\{opened\} disabled=\{busy\}/);
  assert.match(browser, /if \(busy \|\| error \|\| selectionUnknown/);
  assert.equal((browser.match(/disabled=\{busy \|\| error \|\| voiceChangesLocked \|\| selectionUnknown\}/g) ?? []).length, 2);
  const select = browser.slice(browser.indexOf("async function selectVersion("),
    browser.indexOf("\n  }", browser.indexOf("async function selectVersion(")));
  assert.match(select, /await reconcileUncertainSelection\(/);

  const refresh = page.slice(page.indexOf("async function refreshVoiceSelection("),
    page.indexOf("\n  }", page.indexOf("async function refreshVoiceSelection(")));
  assert.match(page, /let selectedVoiceReadVersion = 0/);
  assert.match(refresh, /const requestVersion = \+\+selectedVoiceReadVersion/);
  assert.match(refresh, /disposed \|\| requestVersion !== selectedVoiceReadVersion/);
  assert.match(refresh, /requestVersion === selectedVoiceReadVersion/);
  assert.match(refresh, /return true/);
  assert.match(page, /onRecheck=\{verifySelectedVersion\}/);
  assert.match(page, /if \(!await refreshVoiceSelection\(\) \|\| selectedVoiceKind !== "personal"\)/);
  assert.match(page, /onVoiceActivated=\{async \(versionId, message\)/);
  assert.match(page, /throw new Error\("selected_meeting_voice_unverified"\)/);
});

test("on-demand selected actor fingerprint proves exact version without hashing on every polling read", () => {
  const actor = rust("voice_lab/storage/actor_history.rs");
  const store = rust("voice_lab/storage.rs");
  const facade = rust("voice_lab.rs");
  const build = rust("voice_lab_build.rs");
  const registry = rust("registry.rs");
  const bridge = frontend("app/bridge/myVoiceBuildApi.ts");
  const browser = frontend("components/my-voice/VoiceVersionBrowser.svelte");
  const parent = frontend("pages/MyVoice.svelte");
  assert.match(actor, /fn selected_actor_matches_version\(selected: &Path, id: &str\)/);
  assert.match(actor, /let receipt = fingerprint\(selected\)\?/);
  assert.match(actor, /Ok\(hash_bytes\(&bytes\) == id\)/);
  assert.match(actor, /fn selected_actor_identity_requires_exact_model_bytes_not_just_personal_kind/);
  assert.match(store, /ActorKind::Personal => actor_history::selected_actor_matches_version/);
  assert.match(store, /ActorKind::Builtin\(_\) => Ok\(false\)/);
  assert.match(facade, /selected_personal_actor_matches_version/);
  assert.match(build, /pub fn get_voice_lab_selected_version_match\(version_id: String\) -> Result<bool, String>/);
  assert.match(build, /selected_personal_actor_matches_version\(&storage\(\), &version_id\)/);
  assert.match(registry, /voice_lab_build::get_voice_lab_selected_version_match/);
  assert.match(bridge, /get_voice_lab_selected_version_match", \{ versionId \}/);
  assert.match(parent, /myVoiceBuildApi\.verifySelectedVoiceVersion\(versionId\)/);
  assert.match(browser, /await onRecheck\(versionId\)/);
  assert.match(browser, /await onVoiceActivated\(versionId, result\.message\)/);
  assert.match(browser, /uncertainVersionId = versionId/);
  assert.match(browser, /selectionUnknown = !verified/);
  const statusOwner = build.slice(build.indexOf("fn current_status()"), build.indexOf("fn result("));
  assert.doesNotMatch(statusOwner, /selected_personal_actor_matches_version|selected_actor_matches_version/);
});
