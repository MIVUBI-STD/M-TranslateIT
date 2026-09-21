use crate::engine::runtime_state::{
    latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID,
};

pub fn prepare_for_app_exit() -> bool {
    if super::super::voice_lab_recording::voice_lab_recording_blocks_app_exit()
        || super::super::voice_lab::current_voice_lab_build_snapshot().active
    {
        return false;
    }

    let runtime = latest_runtime_session_state();
    if runtime.has_active_session && runtime.snapshot.is_none() {
        return false;
    }

    if let Some(owner) = runtime
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.owner_id.as_str())
    {
        if owner != APPLICATION_MEETING_OWNER_ID {
            return false;
        }

        let result = super::super::meeting_session::stop_meeting_translation();
        let meeting_still_owned = result.status.has_session
            && result.status.owner_id.as_deref() == Some(APPLICATION_MEETING_OWNER_ID);
        if meeting_still_owned {
            return false;
        }
    }

    if !super::super::helper_bridge::shutdown_helper_bridge_for_app_exit() {
        return false;
    }

    super::super::startup_recovery::mark_clean_shutdown()
}

#[cfg(test)]
mod tests {
    use crate::engine::runtime_state::{
        APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID, VOICE_RECORDING_OWNER_ID,
    };

    #[test]
    fn shared_resource_owner_ids_remain_distinct_for_shutdown_routing() {
        assert_ne!(APPLICATION_MEETING_OWNER_ID, MIC_TEST_OWNER_ID);
        assert_ne!(APPLICATION_MEETING_OWNER_ID, VOICE_RECORDING_OWNER_ID);
        assert_ne!(MIC_TEST_OWNER_ID, VOICE_RECORDING_OWNER_ID);
    }
}
