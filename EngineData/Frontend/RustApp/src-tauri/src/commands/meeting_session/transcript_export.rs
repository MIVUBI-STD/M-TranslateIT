// One self-contained, offline HTML file per explicit export. No script, CDN,
// web resources or speaker inference. Data remains in the user-owned archive.
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::engine::paths::ProjectPaths;
use crate::engine::runtime_state::{latest_runtime_session_state, APPLICATION_MEETING_OWNER_ID};
use super::transcript_archive::{
    read_saved_turns, saved_session_entry_count, saved_session_operation_guard,
    session_warning, stream_saved_turns,
};
use super::MeetingCommittedTurn;

static EXPORT_NONCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct HtmlTranscriptResult {
    pub ok: bool,
    pub file_path: Option<String>,
    pub turn_count: usize,
    pub incomplete: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HtmlTranscriptPreview {
    pub ok: bool,
    pub html: String,
    pub entry_count: usize,
    pub message: String,
}

const MAX_HTML_PREVIEW_ENTRIES: usize = 12;

fn unix_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis()).unwrap_or(0)
}
fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;")
        .replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}
fn valid_id(id: &str) -> bool {
    id.starts_with("meeting_") && id.len() <= 96
        && id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
}
fn failure(message: &str) -> HtmlTranscriptResult {
    HtmlTranscriptResult { ok: false, file_path: None, turn_count: 0,
        incomplete: true, message: message.to_string() }
}
fn utc_date(unix_ms: u128) -> String {
    let mut day = unix_ms / 86_400_000;
    if day > 60_000 { return "Date unavailable".to_string(); }
    let mut year = 1970;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let count = if leap {366} else {365};
        if day < count { break }
        day -= count;
        year += 1;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let months = [31,if leap {29} else {28},31,30,31,30,31,31,30,31,30,31];
    let mut month = 1;
    for length in months {
        if day < length { break }
        day -= length;
        month += 1;
    }
    format!("{year:04}-{month:02}-{:02}", day + 1)
}
fn utc_clock(unix_ms: u128) -> String {
    let s = (unix_ms / 1000) % 86_400;
    format!("{:02}:{:02}:{:02} UTC", s/3600, (s/60)%60, s%60)
}
const STYLE: &str = r#"
:root { color-scheme: dark; --bg:#101316; --surface:#171b1f; --raised:#1d2227;
 --border:#2b3238; --text:#f5f7fa; --muted:#a8b0b7; --warning:#efbb68;
 font-family:Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif; }
* { box-sizing:border-box; }
body { margin:0; background:var(--bg); color:var(--text); line-height:1.6; }
.page { max-width:920px; margin:0 auto; padding:38px 30px 72px; }
.brand { display:flex; align-items:center; gap:12px; font-size:13px; font-weight:650; letter-spacing:.01em; }
.brandmark { display:grid; place-items:center; width:35px; height:35px;
  border:1px solid #3b444c; border-radius:10px; background:var(--raised); font-weight:750; }
.kicker { font-size:10px; color:var(--muted); letter-spacing:.13em; text-transform:uppercase; }
h1 { margin:26px 0 0; font-size:30px; font-weight:700; letter-spacing:-.035em; line-height:1.17; }
.subtitle { margin:6px 0 23px; font-size:13px; color:var(--muted); }
.summary { display:flex; flex-wrap:wrap; gap:10px; margin:0 0 30px; }
.fact { flex:1 1 180px; padding:13px 15px; border:1px solid var(--border); border-radius:10px; background:var(--surface); }
.fact span { display:block; font-size:10px; color:var(--muted); margin-bottom:5px; }
.fact strong { font-size:13px; font-weight:600; overflow-wrap:anywhere; }
.heading { display:flex; justify-content:space-between; align-items:center; gap:16px; margin-bottom:15px; }
h2 { margin:0; font-size:17px; letter-spacing:-.02em; }
.reading { position:sticky; top:0; z-index:2; display:flex; align-items:center; flex-wrap:wrap;
  gap:12px; padding:12px 0; margin-bottom:12px; background:var(--bg); border-bottom:1px solid var(--border); }
.reading-title { font-size:12px; color:var(--muted); font-weight:600; }
.segment { display:inline-flex; padding:3px; gap:2px; border:1px solid var(--border);
 background:var(--surface); border-radius:9px; }
.segment label { cursor:pointer; display:flex; position:relative; align-items:center; }
.segment input { position:absolute; opacity:0; width:1px; height:1px; }
.segment span { display:block; padding:5px 12px; font-size:12px; border-radius:6px; color:var(--muted); font-weight:550; }
.segment input:checked + span { background:var(--raised); color:var(--text); }
.segment input:focus-visible + span { outline:2px solid var(--text); outline-offset:2px; }
body:has(#view-id:checked) .version[data-language="en"],
body:has(#view-en:checked) .version[data-language="id"] { display:none; }
.conversation { border:1px solid var(--border); border-radius:14px; overflow:hidden; background:var(--surface); }
.turn { padding:18px 22px; border-bottom:1px solid var(--border); break-inside:avoid; }
.turn:last-child { border-bottom:0; }
.turn-head { display:flex; justify-content:space-between; gap:14px; align-items:baseline; margin-bottom:11px; }
.speaker { font-size:12px; font-weight:650; }
.time { color:var(--muted); font-size:11px; white-space:nowrap; font-variant-numeric:tabular-nums; }
.version { margin:0 0 9px; }
.version:last-child { margin-bottom:0; }
.version[data-role="translation"] { background:var(--raised); border-radius:8px; padding:11px 13px; margin-top:10px; }
.lang-label { display:block; color:var(--muted); font-size:10px; letter-spacing:.05em; font-weight:650; margin-bottom:3px; }
.body-text { white-space:pre-wrap; overflow-wrap:anywhere; margin:0; font-size:14px; line-height:1.6; }
.unavailable { color:var(--warning); }
.original-fallback { display:none; margin-top:9px; border-top:1px solid var(--border); padding-top:9px; }
body:has(#view-id:checked) .original-fallback,
body:has(#view-en:checked) .original-fallback { display:block; }
.conversation > .notice { margin:15px 20px; }
.print-mode { display:none; }
.notice { margin:22px 0; border:1px solid var(--border); border-radius:10px;
  background:var(--surface); padding:14px 16px; font-size:12px; color:var(--muted); }
.notice strong { color:var(--text); }
.footer { margin-top:30px; padding-top:15px; border-top:1px solid var(--border); font-size:11px; color:var(--muted); }
@media(max-width:600px) { .page{padding:21px 14px 50px;} h1{font-size:25px;}
 .turn{padding:14px;} .reading{top:0;} .segment span{padding:6px 9px;} }
@media print {
 :root { color-scheme:light; --bg:#fff; --surface:#fff; --raised:#f2f4f5;
  --border:#d9dde1; --text:#161b1f; --muted:#535d67; --warning:#864900; }
 @page { size:A4; margin:16mm; }
 body { background:#fff; font-size:11pt; }
 .page { max-width:none; padding:0; margin:0; }
 .reading { display:none; }
 .print-mode { display:block; font-size:10pt; color:var(--muted); margin:0 0 12px; }
 .print-mode-id, .print-mode-en { display:none; }
 body:has(#view-id:checked) .print-mode-both,
 body:has(#view-en:checked) .print-mode-both { display:none; }
 body:has(#view-id:checked) .print-mode-id { display:inline; }
 body:has(#view-en:checked) .print-mode-en { display:inline; }
 .fact { background:#fff; }
 .conversation { border:0; background:#fff; overflow:visible; }
 .turn { padding:12px 0; break-inside:avoid-page; }
 .version[data-role="translation"] { background:#f2f4f5; print-color-adjust:exact; }
 .notice, .fact { break-inside:avoid; }
 h1, h2, .turn-head { break-after:avoid; }
}
"#;

fn write_header<W: Write>(
    w: &mut W, id: &str, count: usize, preview: bool, incomplete: bool,
) -> std::io::Result<()> {
    let start = id.strip_prefix("meeting_").and_then(|x| x.split('_').next())
        .and_then(|x| x.parse::<u128>().ok()).unwrap_or(0);
    write!(w, "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><meta name=\"color-scheme\" content=\"dark light\"><title>Meeting Transcript · TranslateIT</title><style>{STYLE}</style></head><body><main class=\"page\"><header><div class=\"brand\"><span class=\"brandmark\" aria-hidden=\"true\">T</span><span>TranslateIT</span></div><h1>Meeting Transcript</h1><p class=\"subtitle\">Bilingual conversation · Indonesian / English</p><div class=\"summary\"><div class=\"fact\"><span>MEETING DATE (UTC)</span><strong>{}</strong></div><div class=\"fact\"><span>{}</span><strong>{count}</strong></div><div class=\"fact\"><span>SESSION REFERENCE</span><strong>{}</strong></div></div></header><section aria-labelledby=\"conversation-heading\"><div class=\"heading\"><h2 id=\"conversation-heading\">Conversation</h2></div><div class=\"reading\" role=\"group\" aria-label=\"Read transcript in\"><span class=\"reading-title\">Read transcript in</span><div class=\"segment\"><label><input type=\"radio\" id=\"view-both\" name=\"reading-view\" checked><span>Bilingual</span></label><label><input type=\"radio\" id=\"view-id\" name=\"reading-view\"><span>Bahasa Indonesia</span></label><label><input type=\"radio\" id=\"view-en\" name=\"reading-view\"><span>English</span></label></div></div><p class=\"print-mode\">Reading view: <span class=\"print-mode-both\">Bilingual</span><span class=\"print-mode-id\">Bahasa Indonesia</span><span class=\"print-mode-en\">English</span></p><div class=\"conversation\">", utc_date(start), if preview { "PREVIEW ENTRIES" } else { "RECORDED ENTRIES" }, html_escape(id))?;
    if incomplete {
        w.write_all(b"<aside class=\"notice\" role=\"note\"><strong>Transcript completeness unverified.</strong> Some recognized entries may be missing after interrupted or unsuccessful local storage. The conversation below contains only the available saved records.</aside>")?;
    }
    Ok(())
}

fn write_turn<W: Write>(w: &mut W, turn: &MeetingCommittedTurn) -> std::io::Result<()> {
    let speaker = if turn.lane == "you" { "You" } else { "Meeting audio" };
    write!(w, "<article class=\"turn\"><header class=\"turn-head\"><span class=\"speaker\">{speaker}</span><time class=\"time\">{}</time></header>", utc_clock(turn.created_unix_ms))?;
    // Language attributes come only from recognized fixed codes, never from
    // editable archive bytes, so stored text cannot inject HTML attributes.
    let source_code = if turn.source_language == "id" { "id" } else { "en" };
    let target_code = if turn.target_language == "id" { "id" } else { "en" };
    let source_lang = if source_code == "id" { "Indonesian" } else { "English" };
    let target_lang = if target_code == "id" { "Indonesian" } else { "English" };
    write!(w, "<div class=\"version\" data-language=\"{}\" data-role=\"original\"><span class=\"lang-label\">ORIGINAL · {source_lang}</span><p class=\"body-text\" lang=\"{}\">{}</p></div>",
        source_code, source_code, html_escape(&turn.source_text))?;
    write!(w, "<div class=\"version\" data-language=\"{}\" data-role=\"translation\"><span class=\"lang-label\">TRANSLATION · {target_lang}</span>",
        target_code)?;
    if turn.translation_state == "translated" && !turn.translated_text.trim().is_empty() {
        write!(w, "<p class=\"body-text\" lang=\"{}\">{}</p>", target_code, html_escape(&turn.translated_text))?;
    } else {
        // The target-language lane must not pretend that source-language
        // fallback text was translated. Reveal the source only in
        // single-language reading modes, with its actual language marked.
        write!(w, "<p class=\"body-text unavailable\">Translation unavailable.</p><div class=\"original-fallback\"><span class=\"lang-label\">ORIGINAL FALLBACK · {source_lang}</span><p class=\"body-text\" lang=\"{source_code}\">{}</p></div>", html_escape(&turn.source_text))?;
    }
    w.write_all(b"</div></article>")
}

fn write_footer<W: Write>(w: &mut W) -> std::io::Result<()> {
    w.write_all(b"</div></section>")?;
    w.write_all(b"<aside class=\"notice\" role=\"note\"><strong>Transcript scope.</strong> This file contains available recognized speech records, not a verified recording of every spoken sentence. Meeting audio may combine multiple speakers; individual identities are not known. Translation may be unavailable for some entries. Local playback is not proof of delivery to remote participants.</aside><footer class=\"footer\">Generated locally by TranslateIT · Self-contained offline HTML · No scripts or external resources</footer></main></body></html>")
}

pub fn preview_saved_html(session_id: String) -> HtmlTranscriptPreview {
    let failed = |message: String| HtmlTranscriptPreview {
        ok: false, html: String::new(), entry_count: 0, message,
    };
    if !valid_id(&session_id) {
        return failed("Invalid Meeting session identity.".to_string());
    }
    // Match Export/Delete source-archive ownership while rendering the
    // bounded sample. Without this, Read Preview could observe a directory
    // halfway through explicit Delete and report a misleading partial result.
    let _operation = match saved_session_operation_guard() {
        Ok(guard) => guard,
        Err(error) => return failed(error),
    };
    // Keep the HTML render path identical to the actual export, but only
    // render a small sample. Avoid sending a huge document through the Tauri
    // bridge or rendering thousands of transcript nodes in the app.
    let turns = match read_saved_turns(&session_id, Some(MAX_HTML_PREVIEW_ENTRIES)) {
        Ok(turns) if !turns.is_empty() => turns,
        Ok(_) => return failed("No saved conversation entries are available.".to_string()),
        Err(reason) => return failed(reason),
    };
    let incomplete = session_warning(&session_id).unwrap_or(true);
    let mut buffer = Vec::new();
    let rendered = (|| -> std::io::Result<()> {
        write_header(&mut buffer, &session_id, turns.len(), true, incomplete)?;
        buffer.write_all(b"<aside class=\"notice\" role=\"note\"><strong>Sample preview.</strong> Only the latest saved entries are shown here. Export HTML after stopping the Meeting to include all archived entries. This preview does not prove that all spoken audio was recognized.</aside>")?;
        for turn in &turns {
            write_turn(&mut buffer, turn)?;
        }
        write_footer(&mut buffer)
    })();
    if rendered.is_err() {
        return failed("Could not generate HTML preview.".to_string());
    }
    match String::from_utf8(buffer) {
        Ok(html) => HtmlTranscriptPreview {
            ok: true, html, entry_count: turns.len(),
            message: "Sample preview generated using the same HTML renderer as the full export.".to_string(),
        },
        Err(_) => failed("HTML preview contained invalid UTF-8.".to_string()),
    }
}

pub fn export_saved_html(session_id: String) -> HtmlTranscriptResult {
    if !valid_id(&session_id) {
        return failure("Invalid Meeting session identity.");
    }
    if latest_runtime_session_state().snapshot.as_ref().is_some_and(|snapshot|
        snapshot.owner_id == APPLICATION_MEETING_OWNER_ID && snapshot.session_id == session_id
    ) {
        return failure("Stop this Meeting before exporting a stable full-session HTML transcript.");
    }
    // Protect the source archive against a concurrent explicit Delete.
    // This lock is only for saved-session commands, not the live writer.
    let _operation = match saved_session_operation_guard() {
        Ok(guard) => guard,
        Err(error) => return failure(&error),
    };
    let incomplete = match session_warning(&session_id) {
        Ok(value) => value,
        Err(error) => return failure(&error),
    };
    // Listing filenames obtains the header count; the actual HTML pass
    // validates and decodes each JSON entry exactly once. A corrupt entry
    // aborts before publication and retains the original archive untouched.
    let count = match saved_session_entry_count(&session_id) {
        Ok(count) => count,
        Err(error) => return failure(&error),
    };
    if count == 0 {
        return failure("No saved conversation entries are available for this Meeting.");
    }
    let root = PathBuf::from(ProjectPaths::discover().user_saved_dir)
        .join("Transcripts").join("Exports").join(&session_id);
    if fs::create_dir_all(&root).is_err() {
        return failure("Cannot create the HTML export folder.");
    }
    let nonce = EXPORT_NONCE.fetch_add(1, Ordering::Relaxed);
    let filename = format!("Transcript_{}_{}.html", unix_ms(), nonce);
    let destination = root.join(&filename);
    let temporary = root.join(format!("{filename}.partial"));
    if destination.exists() {
        return failure("An HTML file with this generated name already exists. Retry export; the existing file was preserved.");
    }
    let write_result = (|| -> Result<(), String> {
        let file = File::create_new(&temporary).map_err(|_| "Could not start the HTML export file.".to_string())?;
        let mut w = BufWriter::new(file);
        write_header(&mut w, &session_id, count, false, incomplete).map_err(|_| "Cannot write HTML header.".to_string())?;
        let exported = stream_saved_turns(&session_id, |turn| {
            write_turn(&mut w, turn).map_err(|_| "Cannot write a transcript entry.".to_string())
        })?;
        if exported != count {
            return Err("Saved transcript changed during export; retry after storage settles.".to_string());
        }
        write_footer(&mut w).map_err(|_| "Cannot finish the HTML document.".to_string())?;
        w.flush().map_err(|_| "Cannot flush HTML document.".to_string())?;
        w.get_ref().sync_all().map_err(|_| "Cannot sync HTML document.".to_string())?;
        drop(w);
        fs::rename(&temporary, &destination).map_err(|_| "Cannot publish the finished HTML file.".to_string())?;
        Ok(())
    })();
    if let Err(message) = write_result {
        let _ = fs::remove_file(&temporary);
        return failure(&message);
    }
    HtmlTranscriptResult {
        ok: true, file_path: Some(destination.to_string_lossy().replace('\\', "/")),
        turn_count: count, incomplete,
        message: if incomplete {
            "HTML transcript saved. Storage completeness could not be verified; review its warning.".to_string()
        } else {
            format!("HTML transcript saved with {count} available entries.")
        },
    }
}

// Only app-owned exported HTML paths may be opened through the desktop shell.
// A supplied file name cannot escape the session's generated export folder.
pub fn open_saved_html(session_id: String, file_name: String, show_folder: bool) -> super::transcript_archive::ArchiveActionResult {
    use super::transcript_archive::ArchiveActionResult;
    if !valid_id(&session_id)
        || !file_name.starts_with("Transcript_")
        || !file_name.ends_with(".html")
        || file_name.len() > 120
        || !file_name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.' || c == b'-')
    {
        return ArchiveActionResult { ok: false, message: "Invalid saved HTML identity.".to_string() };
    }
    let root = PathBuf::from(ProjectPaths::discover().user_saved_dir)
        .join("Transcripts").join("Exports").join(session_id);
    let file = root.join(file_name);
    if !file.is_file() {
        return ArchiveActionResult { ok: false, message: "The exported HTML file was not found.".to_string() };
    }
    #[cfg(target_os = "windows")]
    {
        // explorer.exe opens the selected file or its containing folder using
        // the user's Windows shell; no cmd.exe, shell interpolation or URL open.
        let target = if show_folder { root.as_path() } else { file.as_path() };
        match std::process::Command::new("explorer.exe").arg(target).spawn() {
            Ok(_) => ArchiveActionResult { ok: true, message: "Windows was asked to open the saved location.".to_string() },
            Err(_) => ArchiveActionResult { ok: false, message: "Windows could not open the saved location.".to_string() },
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = show_folder;
        ArchiveActionResult { ok: false, message: "Open File/Folder is available only on supported Windows builds. Copy the file location instead.".to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::{html_escape, utc_date, utc_clock, write_header, write_turn};
    use super::super::MeetingCommittedTurn;
    #[test]
    fn export_escapes_untrusted_speech_in_html() {
        assert_eq!(html_escape("<script>alert('x') & \"q\"</script>"),
            "&lt;script&gt;alert(&#39;x&#39;) &amp; &quot;q&quot;&lt;/script&gt;");
    }
    #[test]
    fn language_mode_never_labels_untranslated_source_as_target_language() {
        let turn = MeetingCommittedTurn {
            session_id: "meeting_1760600000000_1".to_string(),
            sequence: 1,
            generation: None,
            utterance_id: 1,
            lane: "incoming".to_string(),
            source_language: "en".to_string(),
            target_language: "id".to_string(),
            source_text: "We need to reschedule.".to_string(),
            translated_text: String::new(),
            translation_state: "failed".to_string(),
            delivery_state: None,
            outbound_timing: None,
            created_unix_ms: 1_760_600_000_000,
            updated_unix_ms: 1_760_600_000_001,
        };
        let mut html = Vec::new();
        write_turn(&mut html, &turn).unwrap();
        let html = String::from_utf8(html).unwrap();
        assert!(html.contains("TRANSLATION · Indonesian"));
        assert!(html.contains("Translation unavailable."));
        assert!(html.contains("ORIGINAL FALLBACK · English"));
        assert!(html.contains("lang=\"en\""));
        assert!(!html.contains("Translation unavailable. Original text:"));
    }

    #[test]
    fn incomplete_storage_warning_appears_before_conversation_entries() {
        let mut html = Vec::new();
        write_header(&mut html, "meeting_1760600000000_1", 2, false, true).unwrap();
        let html = String::from_utf8(html).unwrap();
        let conversation = html.find("<div class=\"conversation\">").unwrap();
        let warning = html.find("Transcript completeness unverified.").unwrap();
        assert!(warning > conversation, "warning should precede turn entries");
        assert!(html.contains("Reading view:"));
        assert!(html.contains("print-mode-id"));
        assert!(html.contains("print-mode-en"));
    }

    #[test]
    fn exported_times_are_explicitly_utc() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_clock(0), "00:00:00 UTC");
        assert_eq!(utc_clock(3_661_000), "01:01:01 UTC");
    }
}
