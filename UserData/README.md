# UserData

`UserData` is the runtime/user-data boundary. It is not source code, build input, project memory, or developer-validation storage.

```text
CacheData
-> disposable runtime working data

LogData
-> minimal/redacted operational diagnostics produced by the running product

SavedProject
-> retained content under current product policy: explicitly accepted My Voice data plus automatically saved recognized Meeting transcripts until user Delete
```

## Rules

- Runtime may read/write these roots through canonical application paths.
- Repository/source validation must not use UserData as its output or source-of-truth location.
- Fresh installs start from empty runtime content; development-machine UserData is never a package input.
- Do not commit real conversations, transcripts, raw recordings, generated voice, personal data, runtime logs, or saved user content.
- Raw/TTS audio is temporary by default unless the user explicitly saves an allowed persistent artifact.
- Saved Meeting transcript text is automatically retained locally under the specific Meeting contract (PR-100 through PR-104), separate from temporary raw audio and manually exported HTML. The Saved Transcripts UI must disclose retention and Delete; deleting an archive never silently deletes a previously exported HTML copy.
- Automatic general History and Meeting-specific saved transcript archives are different product semantics; do not collapse them into one History service.

Git keeps only the structural README placeholders. Actual runtime contents are ignored.
