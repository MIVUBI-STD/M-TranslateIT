"""Authored only. Uses fictitious times, never runs real GPU training."""
from my_voice_stall_policy import EpochStallObserver

def test_device_specific_baseline_no_false_slow_gpu_kill():
    obs = EpochStallObserver()
    obs.observe_epoch(1, 600.0)
    obs.observe_epoch(2, 2400.0)
    obs.observe_epoch(3, 4200.0)  # 30-minute epochs on slower GPU
    assert obs.possible_stall(7200.0) is None
    assert obs.possible_stall(15001.0) == "epoch_duration_exceeds_measured_device_baseline"
    assert obs.possible_stall(15002.0) is None

def test_no_training_epoch_warns_once_but_never_proves_hang():
    obs = EpochStallObserver()
    obs.begin(0.0)
    assert obs.possible_stall(60.0) is None
    assert obs.possible_stall(3600.0) == "first_epoch_not_observed_no_baseline"
    assert obs.possible_stall(7200.0) is None

def test_new_epoch_resets_previous_warning():
    obs = EpochStallObserver()
    obs.observe_epoch(1, 50.0)
    assert obs.possible_stall(2800.0) == "epoch_not_seen_for_45_minutes_no_baseline"
    obs.observe_epoch(2, 2900.0)
    assert obs.possible_stall(2901.0) is None


def test_multiple_epochs_between_samples_do_not_inflate_baseline():
    obs = EpochStallObserver()
    obs.begin(0.0)
    obs.observe_epoch(1, 10.0)
    obs.observe_epoch(3, 30.0)
    assert list(obs.intervals) == [10.0]
