"""Authored contract examples only; not executed on the user's device."""
from types import SimpleNamespace
from my_voice_compute_probe import detect_compute
from my_voice_gpu_identity import normalize_uuid, parse_sensor_row

UUID_A = "GPU-11111111-2222-3333-4444-555555555555"
UUID_B = "GPU-99999999-aaaa-bbbb-cccc-dddddddddddd"

def fake_torch(items):
    cuda = SimpleNamespace(
        is_available=lambda: bool(items),
        device_count=lambda: len(items),
        get_device_properties=lambda i: SimpleNamespace(
            total_memory=items[i][1] * 1024**2,
            uuid=items[i][0],
        ),
        get_device_name=lambda i: f"GPU Device {i}",
        mem_get_info=lambda i: (
            items[i][2] * 1024**2, items[i][1] * 1024**2
        ),
    )
    return SimpleNamespace(cuda=cuda)

def sensor_read(uuid):
    totals = {UUID_A: (8192, 7500), UUID_B: (12288, 10000)}
    return totals[uuid]

def test_multiple_devices_choose_more_usable_memory():
    report = detect_compute(fake_torch([
        (UUID_A, 8192, 7000), (UUID_B, 12288, 9400),
    ]), "CPU", sensor_reader=sensor_read)
    assert report["gpu_uuid"] == UUID_B
    assert report["cuda_index"] == 1
    assert report["visible_cuda_devices"] == 2
    assert report["profile"] == "SAFE_CUDA_V1"
    assert report["admission_state"] == "admitted"

def test_required_uuid_never_falls_back_to_another_card():
    report = detect_compute(fake_torch([
        (UUID_A, 8192, 7000), (UUID_B, 12288, 9400),
    ]), "CPU", required_uuid=UUID_A, sensor_reader=sensor_read)
    assert report["gpu_uuid"] == UUID_A

def test_missing_cuda_uuid_fails_closed():
    cuda = SimpleNamespace(
        is_available=lambda: True, device_count=lambda: 1,
        get_device_properties=lambda _: SimpleNamespace(total_memory=8192*1024**2),
    )
    assert detect_compute(SimpleNamespace(cuda=cuda), "CPU")["state"] == "unavailable"

def test_cpu_only_is_not_implicitly_admitted():
    report = detect_compute(fake_torch([]), "CPU")
    assert report["state"] == "cpu_only"
    assert report["gpu_uuid"] is None

def test_bad_gpu_sensor_identity_blocks_admission():
    report = detect_compute(fake_torch([(UUID_A, 8192, 7000)]), "CPU",
                            sensor_reader=lambda _: (8192, 7000) if False else (_ for _ in ()).throw(ValueError()))
    assert report["admission_state"] == "unknown"

def test_nvidia_sensor_row_checks_uuid_not_ordinal():
    assert parse_sensor_row(UUID_B + ", 12288, 10500", UUID_B) == (12288, 10500)
    try:
        parse_sensor_row(UUID_B + ", 12288, 10500", UUID_A)
        assert False
    except Exception:
        pass
    assert normalize_uuid(UUID_A) == UUID_A
