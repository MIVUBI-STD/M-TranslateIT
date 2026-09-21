use std::sync::atomic::{AtomicU64, Ordering};

use super::capabilities::resolve_capabilities;
use super::contract::ApplicationSnapshot;
use super::lifecycle::derive_lifecycle;
use super::problems::collect_problems;
use super::resources::resolve_resources;
use super::summaries::build_subsystem_summaries;
use super::super::{audio, helper_bridge, meeting_session, settings};

static APPLICATION_REVISION: AtomicU64 = AtomicU64::new(0);

pub fn current_application_snapshot() -> ApplicationSnapshot {
    let settings = settings::load_runtime_settings();
    let meeting = meeting_session::get_meeting_session_status();
    let helper = helper_bridge::get_helper_bridge_status();
    let input = audio::get_input_status();
    let worker = if helper.state == "ready" {
        Some(helper_bridge::helper_bridge_worker_status())
    } else {
        None
    };

    let summaries = build_subsystem_summaries(&meeting, &helper, &input);
    let resources = resolve_resources(&summaries);
    let lifecycle = derive_lifecycle(&summaries);
    let capabilities = resolve_capabilities(&summaries, &resources);
    let problems = collect_problems(&summaries);

    ApplicationSnapshot {
        revision: APPLICATION_REVISION.fetch_add(1, Ordering::AcqRel) + 1,
        lifecycle,
        active_owner: summaries.meeting.owner_id.clone(),
        settings,
        meeting,
        helper,
        worker,
        input,
        summaries,
        resources,
        capabilities,
        problems,
    }
}
