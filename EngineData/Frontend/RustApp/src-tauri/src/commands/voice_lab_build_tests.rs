use super::{missing_training_coverage_group, GuidedTakeContract, MAX_REFERENCE_MS, MIN_REFERENCE_MS};

fn take(line_id: u32) -> GuidedTakeContract {
    GuidedTakeContract {
        line_id,
        exact_text: format!("line {line_id}"),
        wav_file: format!("take_{line_id:04}.wav"),
    }
}

#[test]
fn many_accepted_lines_from_one_style_do_not_satisfy_recording_variety() {
    let takes = (1..=12).map(take).collect::<Vec<_>>();
    let missing = missing_training_coverage_group(&takes).expect("coverage must remain incomplete");

    assert_eq!(missing.start_line_id, 25);
    assert_eq!(missing.end_line_id, 30);
}

#[test]
fn reference_take_contract_uses_same_three_to_ten_second_window_as_actor_runtime() {
    assert_eq!(MIN_REFERENCE_MS, 3_000);
    assert_eq!(MAX_REFERENCE_MS, 10_000);
}

#[test]
fn one_accepted_line_from_each_curated_block_satisfies_recording_variety() {
    let takes = [1, 25, 31, 65, 97]
        .into_iter()
        .map(take)
        .collect::<Vec<_>>();

    assert!(missing_training_coverage_group(&takes).is_none());
}
