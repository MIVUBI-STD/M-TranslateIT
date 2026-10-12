## Meeting transcript refinement — 2026-10-12

**Decision:** use WhatsApp-like left/right transcript presentation, `You` for microphone turns on the right and `Person 1` on the left for the **single combined incoming lane**. This is **not** speaker diarization and must not infer identities or separate different participants without actual speaker evidence. Display local recorded time and a readable session date/time; retain original and translation as separate fields. One reusable bubble component renders Live and Last Meeting.

**Manual export policy:** every Meeting already has a distinct runtime session ID, but only the active plus most recently ended transcript is available in memory. The new manual export uses a unique folder under `SavedProject/Transcripts/Meeting_<session-id>/`. No background transcript saving and no persistent archive listing are added. Users must export before starting another meeting or exiting. The current 240-entry bounded buffer may omit early phrases; its incomplete warning is retained. Full-session temporary spooling and multi-speaker diarization are separate future changes requiring explicit implementation and acceptance.

**Browser limitations:** incoming WASAPI endpoint-loopback may include non-meeting system sounds and may miss browser audio routed to another speaker. An active capture stream is not transcription or participant-identity proof. Windows two-device meeting test, simultaneous speech, suppression, 20-second defer/drop behavior and transcript export are still NOT EXECUTED for this source revision.

**Quality ceiling:** source changes and contract assertions alone. Do not call it target Windows tested, fully transcribed or diarization-ready.

## Meeting transcript refinement — 2026-10-12

**Decision:** use WhatsApp-like left/right transcript presentation, `You` for microphone turns on the right and `Person 1` on the left for the **single combined incoming lane**. This is **not** speaker diarization and must not infer identities or separate different participants without actual speaker evidence. Display local recorded time and a readable session date/time; retain original and translation as separate fields. One reusable bubble component renders Live and Last Meeting.

**Manual export policy:** every Meeting already has a distinct runtime session ID, but only the active plus most recently ended transcript is available in memory. The new manual export uses a unique folder under `SavedProject/Transcripts/Meeting_<session-id>/`. No background transcript saving and no persistent archive listing are added. Users must export before starting another meeting or exiting. The current 240-entry bounded buffer may omit early phrases; its incomplete warning is retained. Full-session temporary spooling and multi-speaker diarization are separate future changes requiring explicit implementation and acceptance.

**Browser limitations:** incoming WASAPI endpoint-loopback may include non-meeting system sounds and may miss browser audio routed to another speaker. An active capture stream is not transcription or participant-identity proof. Windows two-device meeting test, simultaneous speech, suppression, 20-second defer/drop behavior and transcript export are still NOT EXECUTED for this source revision.

**Quality ceiling:** source changes and contract assertions alone. Do not call it target Windows tested, fully transcribed or diarization-ready.

