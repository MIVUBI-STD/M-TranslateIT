# Natural Bahasa Indonesia voice comparison (explicit cloud fixture preparation)

The source of truth remains the **48-case canonical audio scenario suite**. The first neural voice comparison is deliberately limited to five public sentences **A01, A03, A04, A06 and A08**, spoken by two independent Indonesian voices: Microsoft Azure `id-ID-ArdiNeural` (male) and `id-ID-GadisNeural` (female). Ten audio files are generated only after explicit invocation. These voices are listed by Microsoft as supported Indonesian neural voices.

```powershell
python tools/audio_validation/natural_voice_comparison.py plan
```

The `plan` action needs no credentials, network or billing. To run the **optional billed, cloud-based syntheses** in a separately authorized local environment with a valid Azure Speech resource:

```powershell
# Set AZURE_SPEECH_KEY and AZURE_SPEECH_REGION securely in the environment.
python tools/audio_validation/natural_voice_comparison.py generate --destination UserData/CacheData/audio_validation/azure-voice-compare-v1
```

The generator never logs, commits or embeds the Speech key. It uses fixed Azure REST endpoints, constrained voice choices, bounded request count (10), SSML escaping, request timeout, SHA-256 manifests and WAV integrity checks. The provider produces 24 kHz mono PCM16 WAV; comparison WAVs are deterministically resampled to **16 kHz mono PCM16** for the existing replay input. The original high-resolution WAV hash is retained as provenance but the native sample file is not retained. Both voice identities and textual references remain visible in `comparison.json`.

**This is not automatic CI and does not alter TranslateIT's runtime architecture.** Cloud is for optional *fixture creation only*, never live ASR, translation, TTS, microphone or Meeting. Do not provide API keys in this chat. Check your organization's service/billing and audio-data reuse terms before invoking. A generated waveform is not proof of natural intelligibility, ASR quality or native delivery.

The comparison has a distinct schema `translateit.natural_voice_comparison.v1`. It must not be passed to the 48-case `translateit.audio_fixtures.v1` runner as if it were an identical canonical suite. After subjective quality review, an approved voice-profile and variant manifest can be linked to a second, explicitly versioned fixture suite without changing historical eSpeak hashes.
