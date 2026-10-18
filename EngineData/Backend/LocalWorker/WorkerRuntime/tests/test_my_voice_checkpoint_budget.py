"""Source-only arithmetic fixtures; intentionally NOT executed."""
from voice_lab_gpt_sovits_build import (
    MAX_FULL_GPT_SAVE_EPOCHS,
    MAX_FULL_SOVITS_PAIR_SAVES,
    optimizer_checkpoint_interval,
)

def test_fixed_release_epoch_cadence_bounds_optimizer_saves():
    assert optimizer_checkpoint_interval(8, MAX_FULL_SOVITS_PAIR_SAVES) == 2
    assert optimizer_checkpoint_interval(15, MAX_FULL_GPT_SAVE_EPOCHS) == 3
    sovits_pairs = 8 // 2
    gpt_saves = 15 // 3
    # Each SoVITS save is G + D + pair receipt. GPT may keep both
    # epoch-specific and last-version checkpoint per scheduled save.
    assert sovits_pairs * 3 + gpt_saves * 2 + 2 <= 28

def test_nonuniform_epochs_do_not_exceed_configured_save_budget():
    for epochs in range(1, 65):
        interval = optimizer_checkpoint_interval(epochs, 5)
        assert epochs // interval <= 5
