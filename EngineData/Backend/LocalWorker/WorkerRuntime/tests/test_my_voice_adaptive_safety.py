"""Authored policy examples; not executed or measured on real hardware."""

from my_voice_adaptive_safety import PressureObserver, SafetySample


def sample(gpu: int, ram: int = 4096, cache: int = 16384, saved: int = 12288):
    return SafetySample(gpu, ram, cache, saved)


def test_stable_high_vram_does_not_force_stop():
    policy = PressureObserver(require_gpu=True)
    assert [policy.observe(sample(920)).state for _ in range(4)] == [
        "watch",
        "watch",
        "watch",
        "watch",
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


def test_ram_pressure_requires_trend():
    monitor = PressureObserver(require_gpu=False)
    monitor.observe(SafetySample(None, 1900, 16384, 12288))
    monitor.observe(SafetySample(None, 1780, 16384, 12288))
    assert monitor.observe(SafetySample(None, 1590, 16384, 12288)).state == "pressure"


def test_watch_state_returns_to_normal_when_headroom_recovers():
    monitor = PressureObserver(require_gpu=True)
    assert monitor.observe(sample(1100)).state == "watch"
    assert monitor.observe(sample(2600)).state == "normal"


def test_soft_yield_ignores_initial_allocation_ramp():
    from my_voice_adaptive_safety import soft_stop_ready

    assert not soft_stop_ready("pressure", 20, 7)
    assert not soft_stop_ready("pressure", 100, 1)
    assert soft_stop_ready("pressure", 100, 3)
    assert not soft_stop_ready("critical", 100, 3)
