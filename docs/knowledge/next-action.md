# Next Action

## Current Status

- `Local` remains the sole active authority for development, CI, proof, continuation, and release-source validation.
- Core architecture is structurally hardened across Meeting, helper bridge, Text, My Voice, setup, diagnostics, Windows audio, and local voice-runtime ownership.
- Translation quality now uses a 170-case semantic-risk regression corpus plus a separate 40-case held-out benchmark with risk-tag gates. Do not tune against the held-out set.
- ASR quality now carries 60 linguistic/accent/acoustic/device/meeting-compression acceptance cases.
- TTS/My Voice quality now carries 30 synthesis acceptance cases covering intelligibility, speaker similarity, artifacts, long form, acronyms, lists, numbers, entities, and repeated-synthesis targets.
- Quality Readiness remains fail-closed over Translation + ASR + TTS evidence with candidate identity and report-hash binding.
- Diagnostics already expose queue/drop and outbound stage timing needed for later native performance evidence.

## Active Boundary

REMOTE_GITHUB architecture work is complete for currently evidenced defects. Continue only source-side work that adds falsifiable quality coverage, reliability, accessibility, or release integrity.

Do not add speculative runtime abstractions, extra providers, languages, cloud fallback, document/image translation, or performance optimizations without measured evidence.

Physical microphone behavior, real CUDA/resource pressure, meeting-app reception, audible voice quality, practical latency, and long-session stability require **TARGET_WINDOWS / NATIVE_ACCEPTANCE** evidence.

## Next Step

When native testing becomes available:

1. Run `docs/knowledge/operations/target-windows-performance.md` with outbound-only baseline first.
2. Record speech boundary, finalization, queue, audio prepare, ASR, translation tokenize/inference/decode/throughput, TTS, delivery, total latency, overflow/drop, CPU/RAM/GPU/VRAM, and meeting-app reception.
3. Route only the first measured bottleneck:
   - queue growth → bounded playback/AI decoupling;
   - native output jitter → persistent output stream;
   - warm Start proof cost → proof reuse/rebinding;
   - TTS dominance → quality-preserving fragments.

Until then, preserve one canonical MiLMMT pipeline, model/voice quality, generation authority, bounded queues, at-most-once output, terminology determinism, and fail-closed readiness.
