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
        get_device_capability=lambda i: (8, 6),
        mem_get_info=lambda i: (items[i][2] * 1024**2, items[i][1] * 1024**2),
    )
    return SimpleNamespace(cuda=cuda)


def sensor_read(uuid):
    totals = {UUID_A: (8192, 7500), UUID_B: (12288, 10000)}
    return totals[uuid]


def test_multiple_devices_choose_more_usable_memory():
    report = detect_compute(
        fake_torch(
            [
                (UUID_A, 8192, 7000),
                (UUID_B, 12288, 9400),
            ]
        ),
        "CPU",
        sensor_reader=sensor_read,
    )
    assert report["gpu_uuid"] == UUID_B
    assert report["cuda_index"] == 1
    assert report["visible_cuda_devices"] == 2
    assert report["profile"] == "SAFE_CUDA_V1"
    assert report["admission_state"] == "admitted"


def test_bad_other_gpu_does_not_block_valid_gpu():
    def sensor(uuid):
        if uuid == UUID_A:
            raise RuntimeError("sensor unavailable for first GPU")
        return 12288, 10000

    report = detect_compute(
        fake_torch(
            [
                (UUID_A, 8192, 7000),
                (UUID_B, 12288, 9400),
            ]
        ),
        "CPU",
        sensor_reader=sensor,
    )
    assert report["gpu_uuid"] == UUID_B
    assert report["admission_state"] == "admitted"


def test_required_uuid_never_falls_back_to_another_card():
    report = detect_compute(
        fake_torch(
            [
                (UUID_A, 8192, 7000),
                (UUID_B, 12288, 9400),
            ]
        ),
        "CPU",
        required_uuid=UUID_A,
        sensor_reader=sensor_read,
    )
    assert report["gpu_uuid"] == UUID_A


def test_missing_cuda_uuid_fails_closed():
    cuda = SimpleNamespace(
        is_available=lambda: True,
        device_count=lambda: 1,
        get_device_properties=lambda _: SimpleNamespace(total_memory=8192 * 1024**2),
    )
    assert detect_compute(SimpleNamespace(cuda=cuda), "CPU")["state"] == "unavailable"


def test_known_but_unreviewed_cuda_architecture_is_blocked():
    model = fake_torch([(UUID_A, 8192, 7000)])
    model.cuda.get_device_capability = lambda _: (6, 1)
    info = detect_compute(model, "CPU", sensor_reader=sensor_read)
    assert info["state"] == "cuda_detected"
    assert info["compute_capability"] == "6.1"
    assert info["admission_state"] == "blocked_unsupported_precision"


def test_cpu_only_is_not_implicitly_admitted():
    report = detect_compute(fake_torch([]), "CPU")
    assert report["state"] == "cpu_only"
    assert report["gpu_uuid"] is None


def test_bad_gpu_sensor_identity_blocks_admission():
    def invalid_sensor(_):
        raise RuntimeError("wrong physical UUID")

    report = detect_compute(fake_torch([(UUID_A, 8192, 7000)]), "CPU", sensor_reader=invalid_sensor)
    assert report["admission_state"] == "unknown"


def test_nvidia_sensor_row_checks_uuid_not_ordinal():
    assert parse_sensor_row(UUID_B + ", 12288, 10500", UUID_B) == (12288, 10500)
    import pytest

    from my_voice_gpu_identity import DeviceIdentityError

    with pytest.raises(DeviceIdentityError):
        parse_sensor_row(UUID_B + ", 12288, 10500", UUID_A)
    assert normalize_uuid(UUID_A) == UUID_A


def test_inventory_reader_matches_exact_requested_device():
    from subprocess import CompletedProcess
    from unittest.mock import patch

    from my_voice_gpu_identity import sensor_inventory_mib

    rows = UUID_A + ", 8192, 7100\n" + UUID_B + ", 12288, 9500\n"
    with patch("my_voice_gpu_identity.sensor_executable", return_value="nvidia-smi"):
        with patch(
            "my_voice_gpu_identity.subprocess.run", return_value=CompletedProcess([], 0, rows, "")
        ) as call:
            inventory = sensor_inventory_mib()
    assert inventory[UUID_A] == (8192, 7100)
    assert inventory[UUID_B] == (12288, 9500)
    assert call.call_count == 1


def test_autodiscovery_uses_one_inventory_read_even_for_multiple_cards():
    from unittest.mock import patch

    inventory = {UUID_A: (8192, 7500), UUID_B: (12288, 10000)}
    with patch("my_voice_compute_probe.sensor_inventory_mib", return_value=inventory) as reader:
        result = detect_compute(fake_torch([(UUID_A, 8192, 7000), (UUID_B, 12288, 9400)]), "CPU")
    assert result["gpu_uuid"] == UUID_B
    reader.assert_called_once()
