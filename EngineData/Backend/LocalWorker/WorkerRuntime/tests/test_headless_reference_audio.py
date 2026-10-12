from __future__ import annotations

import struct
import wave

import numpy as np

from voice_lab_upstream_stage import _load_reference_audio


def test_reference_decoder_preserves_pcm_scale_channels_and_sample_rate(tmp_path):
    reference = tmp_path / "reference.wav"
    with wave.open(str(reference), "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(32_000)
        writer.writeframes(struct.pack("<3h", -32768, 0, 16384))

    audio, sample_rate = _load_reference_audio(reference)
    assert sample_rate == 32_000
    assert tuple(audio.shape) == (1, 3)
    np.testing.assert_array_equal(audio.numpy(), [[-1.0, 0.0, 0.5]])
