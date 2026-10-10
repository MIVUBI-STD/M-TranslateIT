---
name: windows-audio-runtime-development
description: TranslateIT specialist for Windows audio: physical microphone discovery/capture, continuous Session Listening, VAD/finalization, buffers/formats, Meeting Sound, Windows device lifecycle, virtual Meeting route and TranslateIT Meeting Microphone delivery. Do not use for AI inference, desktop presentation or packaging.
---

# Windows Audio Runtime Development

## Current contract

Normal Meeting capture is continuous **Session Listening** after explicit Start. Push to Talk is outside current product scope.

```text
physical mic
→ capture/VAD/finalized utterance
→ local AI
→ translated English TTS
→ Meeting output delivery
→ TranslateIT Meeting Microphone
```

Optional incoming:

```text
Meeting Sound
→ capture/finalization
→ local AI EN→ID text
```

## Owns

- microphone discovery/selection/capture;
- Session Listening lifecycle;
- VAD/silence/finalized utterance mechanics;
- PCM/buffering/resampling where required;
- Windows device lifecycle;
- Meeting Sound capture;
- virtual route detection/configuration/use;
- TranslateIT Meeting Microphone delivery;
- audio-side capability truth and timing.

## Rules

- One canonical capture/route owner; no competing microphone pipelines.
- Device discovery is not route success.
- Speaker playback is not Meeting delivery.
- Missing route must be explicit; never silently fall back to raw mic/system speaker/another mic.
- Incoming is optional/degradable and must not block otherwise healthy outbound.
- TranslateIT's own TTS must not be re-consumed as incoming speech.
- Historical VAD constants are tuning evidence, not permanent product law.
- Raw audio remains temporary by default.
- Lifecycle must release handles cleanly and reject stale/duplicate output.

## Procedure

1. Ground the user's current capture/route symptom in real device and Meeting lifecycle evidence.
2. Identify the physical input, VAD/finalization, shared-resource, selected virtual route or output-delivery owner.
3. Preserve explicit device selection, one capture owner, generation-bound playback and no speaker/raw-mic fallback.
4. Make the smallest owning change; inspect disconnect/reconnect, cancellation, cleanup and stale output boundaries.
5. Check source contracts and any available targeted integration fixture for the changed audio path.
6. Reserve real mic, device, VB-CABLE, meeting-app reception and output timing acceptance for TARGET_WINDOWS.
7. If another domain is actually responsible, propose an evidence-bounded handoff; never silently switch specialists.

## Proof

Static proof can establish lifecycle wiring and failure mapping. Real microphone capture, VAD behavior, device routing, virtual endpoint operation and meeting-application delivery require `TARGET_WINDOWS` evidence when claimed.
