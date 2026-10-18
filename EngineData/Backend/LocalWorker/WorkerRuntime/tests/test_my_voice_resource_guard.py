"""Source regression cases authored only; no runtime validation performed."""
import pytest

from my_voice_resource_guard import (
    CRITICAL_DISK_MIB,
    START_CACHE_MIB,
    START_SAVED_MIB,
    ResourceGuardError,
    _existing_volume,
    disk_free_mib,
)

def test_relative_root_rejected():
    with pytest.raises(ResourceGuardError, match="resource_root_not_absolute"):
        _existing_volume("relative/path")

def test_missing_root_rejected():
    with pytest.raises(ResourceGuardError, match="resource_root_missing"):
        _existing_volume("")

def test_free_disk_read_is_bounded(tmp_path):
    free = disk_free_mib(str(tmp_path))
    assert type(free) is int and free >= 0

def test_thresholds_have_room_beyond_critical():
    assert START_CACHE_MIB > CRITICAL_DISK_MIB
    assert START_SAVED_MIB > CRITICAL_DISK_MIB
