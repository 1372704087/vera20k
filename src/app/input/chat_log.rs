//! On-disk chat log for the VeraX chat box.
//!
//! The chat box keeps a bounded (100-row) in-memory history, which dies with
//! the process. Every posted chat line is also appended to `chat.log` under the
//! writable VERA settings root (`PathsConfig::settings_dir`), next to
//! `VERA.INI`, so a session can be reviewed afterwards. The native layout
//! never calls this: gamemd has no chat history at all.
//!
//! The file rotates at `CHAT_LOG_MAX_BYTES`: the full file becomes
//! `chat.log.1` (replacing the previous one) and a fresh `chat.log` starts,
//! so a long-lived profile never grows without bound.
//!
//! Appends are best-effort. A read-only profile directory must not disturb
//! gameplay, so every IO failure is ignored and the line still reaches the
//! message list.

use std::io::Write;

use crate::app::AppState;

/// Rotate once the log passes this size. A busy chat line is well under
/// 200 bytes, so 2 MiB holds many sessions before a rotation.
const CHAT_LOG_MAX_BYTES: u64 = 2 * 1024 * 1024;

/// Append one `name: text` row. The caller formats the line.
pub(crate) fn append(state: &AppState, line: &str) {
    let Some(config) = state.platform.game_config.as_ref() else {
        return;
    };
    let directory = config.paths.settings_dir();
    if std::fs::create_dir_all(directory).is_err() {
        return;
    }
    let path = directory.join("chat.log");
    rotate_if_full(&path, &directory.join("chat.log.1"));
    let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

/// Move the log aside once it reaches the cap. The previous archive is
/// replaced, so the profile keeps at most two files.
fn rotate_if_full(path: &std::path::Path, archive: &std::path::Path) {
    let full = std::fs::metadata(path).is_ok_and(|meta| meta.len() >= CHAT_LOG_MAX_BYTES);
    if !full {
        return;
    }
    let _ = std::fs::remove_file(archive);
    let _ = std::fs::rename(path, archive);
}
