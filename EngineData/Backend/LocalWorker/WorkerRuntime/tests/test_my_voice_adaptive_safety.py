"""Authored policy examples; not executed or measured on real hardware."""
from my_voice_adaptive_safety import PressureObserver, SafetySample

def sample(gpu: int, ram: int = 4096, cache: int = 16384, saved: int = 12288):
    return SafetySample(gpu, ram, cache, saved)

def test_stable_high_vram_does_not_force_stop():
    policy = PressureObserver(require_gpu=True)
    assert [policy.observe(sample(920)).state for _ in range(4)] == [
        "watch", "watch", "watch", "watch"
    ]

def test_sustained_decline_requests_cooperative_stop():
    policy = PressureObserver(require_gpu=True)
    policy.observe(sample(1480))
    policy.observe(sample(1320))
    assert policy.observe(sample(1180)).state == "pressure"

def test_critical_vram_is_emergency():
    assert PressureObserver(require_gpu=True).observe(sample(600)).state == "critical"

def test_unknown_telemetry_fails_closed():
    assert PressureObserver(require_gpu=True).observe(sample(None)).state == "unknown"
