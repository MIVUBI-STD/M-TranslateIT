from types import SimpleNamespace

from my_voice_compute_probe import detect_compute


def test_cpu_mode_is_unverified_not_ready() -> None:
    report = detect_compute(SimpleNamespace(cuda=SimpleNamespace(is_available=lambda: False)), "Test CPU")
    assert report["state"] == "cpu_only"
    assert report["gpu_name"] is None
    assert report["vram_mib"] is None
    assert "unverified" in report["detail"]


def test_cuda_probe_reports_total_not_available_vram() -> None:
    cuda = SimpleNamespace(
        is_available=lambda: True,
        get_device_name=lambda index: "Test GPU",
        get_device_properties=lambda index: SimpleNamespace(total_memory=12 * 1024**3),
        mem_get_info=lambda index: (10 * 1024**3, 12 * 1024**3),
    )
    report = detect_compute(SimpleNamespace(cuda=cuda), "Test CPU")
    assert report["state"] == "cuda_detected"
    assert report["gpu_name"] == "Test GPU"
    assert report["vram_mib"] == 12 * 1024
    assert report["free_vram_mib"] == 10 * 1024
    assert report["safe_budget_mib"] >= 3072
    assert report["admission_state"] == "admitted"
    assert "not yet been verified" in report["detail"]


def test_failed_cuda_probe_never_claims_cpu_or_gpu_readiness() -> None:
    def fail(_: int) -> str:
        raise RuntimeError("driver unavailable")

    cuda = SimpleNamespace(
        is_available=lambda: True,
        get_device_name=fail,
        get_device_properties=lambda index: SimpleNamespace(total_memory=16 * 1024**3),
    )
    report = detect_compute(SimpleNamespace(cuda=cuda), "Test CPU")
    assert report["state"] == "unavailable"
    assert report["gpu_name"] is None
