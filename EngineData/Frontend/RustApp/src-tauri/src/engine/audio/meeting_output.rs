use std::fs;
use std::io::Read;

use super::duration_ms;

#[path = "meeting_output_runtime.rs"]
mod runtime;

pub(crate) use runtime::MAX_DELIVERY_DEADLINE_MS;

pub use runtime::{
    cancel_meeting_output_for_generation, clear_prepared_meeting_output_device,
    prepare_meeting_output_device, probe_prepared_meeting_output_device_functionally,
    MeetingOutputDeliveryReport,
};

const RIFF_HEADER_BYTES: usize = 12;

fn riff_boundary_blocker(header: &[u8], file_len: u64) -> Option<&'static str> {
    if header.len() < RIFF_HEADER_BYTES || &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return None;
    }

    let declared_size = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as u64;
    let Some(declared_end) = 8u64.checked_add(declared_size) else {
        return Some("meeting_output:wav_chunk_overflow");
    };
    if declared_end < RIFF_HEADER_BYTES as u64 {
        return Some("meeting_output:wav_riff_size_invalid");
    }
    if declared_end > file_len {
        return Some("meeting_output:wav_truncated");
    }
    if declared_end < file_len {
        return Some("meeting_output:wav_riff_size_mismatch");
    }
    None
}

fn validate_source_riff_boundary(source_audio_path: &str) -> Option<&'static str> {
    let metadata = fs::metadata(source_audio_path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    let mut file = fs::File::open(source_audio_path).ok()?;
    let mut header = [0u8; RIFF_HEADER_BYTES];
    if file.read_exact(&mut header).is_err() {
        return None;
    }
    riff_boundary_blocker(&header, metadata.len())
}

fn blocked_before_execution(blocker: &str) -> MeetingOutputDeliveryReport {
    MeetingOutputDeliveryReport {
        ok: false,
        execution_attempted: false,
        first_playback_at: None,
        first_playback_unix_ms: None,
        blocker: blocker.to_string(),
    }
}

pub fn deliver_meeting_output_wav(
    source_audio_path: &str,
    selected_output_device: Option<&str>,
    generation: u64,
) -> MeetingOutputDeliveryReport {
    if let Some(blocker) = validate_source_riff_boundary(source_audio_path) {
        return blocked_before_execution(blocker);
    }
    runtime::deliver_meeting_output_wav(source_audio_path, selected_output_device, generation)
}

#[cfg(test)]
mod tests {
    use super::{duration_ms, riff_boundary_blocker};

    fn riff_header(declared_size: u32) -> [u8; 12] {
        let mut header = [0u8; 12];
        header[0..4].copy_from_slice(b"RIFF");
        header[4..8].copy_from_slice(&declared_size.to_le_bytes());
        header[8..12].copy_from_slice(b"WAVE");
        header
    }

    #[test]
    fn exact_riff_boundary_is_accepted() {
        let header = riff_header(36);
        assert_eq!(riff_boundary_blocker(&header, 44), None);
    }

    #[test]
    fn declared_riff_larger_than_file_is_truncated() {
        let header = riff_header(40);
        assert_eq!(
            riff_boundary_blocker(&header, 44),
            Some("meeting_output:wav_truncated")
        );
    }

    #[test]
    fn bytes_outside_declared_riff_are_rejected_fail_closed() {
        let header = riff_header(28);
        assert_eq!(
            riff_boundary_blocker(&header, 44),
            Some("meeting_output:wav_riff_size_mismatch")
        );
    }

    #[test]
    fn impossible_riff_size_is_rejected() {
        let header = riff_header(0);
        assert_eq!(
            riff_boundary_blocker(&header, 44),
            Some("meeting_output:wav_riff_size_invalid")
        );
    }

    #[test]
    fn non_riff_header_is_left_to_runtime_decoder() {
        assert_eq!(riff_boundary_blocker(b"not a header", 12), None);
    }

    #[test]
    fn shared_duration_contract_remains_visible_to_runtime_child() {
        assert_eq!(duration_ms(16_000, 16_000), 1_000);
    }
}
