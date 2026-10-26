//! OS clipboard and external-URL handoff for the VeraX chat box.
//!
//! Both are presentation-only client duties: the chat history can be copied
//! out of the game, and a link posted in chat can be opened in the desktop
//! browser. Neither touches simulation state, so nothing here is reached from
//! `sim/`.
//!
//! Each platform uses what it already ships: Win32's clipboard on Windows
//! (`windows-sys` is already a client dependency for `ClipCursor`), `pbcopy`
//! and `open` on macOS, and the Wayland/X11 clipboard helpers on Linux. A
//! desktop without those helpers reports failure and the caller keeps the
//! selection, so the copy is a no-op rather than an error.

/// Put `text` on the system clipboard. False when the platform refused.
#[cfg(target_os = "windows")]
pub(crate) fn set_text(text: &str) -> bool {
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock,
    };
    use windows_sys::Win32::System::Ole::CF_UNICODETEXT;
    // SAFETY: the clipboard is opened, written once, and closed on every path;
    // the handle is freed only when SetClipboardData did not take ownership.
    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return false;
        }
        let written = if EmptyClipboard() == 0 {
            false
        } else {
            // CF_UNICODETEXT is NUL-terminated UTF-16.
            let units: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let mem = GlobalAlloc(GMEM_MOVEABLE, units.len() * 2);
            if mem.is_null() {
                false
            } else {
                let dst = GlobalLock(mem) as *mut u16;
                if dst.is_null() {
                    GlobalFree(mem);
                    false
                } else {
                    std::ptr::copy_nonoverlapping(units.as_ptr(), dst, units.len());
                    GlobalUnlock(mem);
                    let taken = !SetClipboardData(CF_UNICODETEXT as u32, mem).is_null();
                    if !taken {
                        GlobalFree(mem);
                    }
                    taken
                }
            }
        };
        CloseClipboard();
        written
    }
}

/// Put `text` on the system clipboard. False when the platform refused.
#[cfg(target_os = "macos")]
pub(crate) fn set_text(text: &str) -> bool {
    pipe_to("pbcopy", &[], text)
}

/// Put `text` on the system clipboard. False when the platform refused.
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn set_text(text: &str) -> bool {
    pipe_to("wl-copy", &[], text) || pipe_to("xclip", &["-selection", "clipboard"], text)
}

/// Put `text` on the system clipboard. False when the platform refused.
#[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
pub(crate) fn set_text(_text: &str) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
fn pipe_to(program: &str, args: &[&str], text: &str) -> bool {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let Ok(mut child) = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    else {
        return false;
    };
    let Some(stdin) = child.stdin.as_mut() else {
        return false;
    };
    if stdin.write_all(text.as_bytes()).is_err() {
        return false;
    }
    child.wait().is_ok_and(|status| status.success())
}

/// Hand a link to the desktop's default browser. Only http(s) reaches here —
/// the caller filters the row text — so no other scheme is ever dispatched.
pub(crate) fn open_url(url: &str) -> bool {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return false;
    }
    #[cfg(target_os = "windows")]
    let (program, args): (&str, &[&str]) = ("rundll32", &["url.dll,FileProtocolHandler"]);
    #[cfg(target_os = "macos")]
    let (program, args): (&str, &[&str]) = ("open", &[]);
    #[cfg(all(unix, not(target_os = "macos")))]
    let (program, args): (&str, &[&str]) = ("xdg-open", &[]);
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    let (program, args): (&str, &[&str]) = ("", &[]);

    if program.is_empty() {
        return false;
    }
    std::process::Command::new(program)
        .args(args)
        .arg(url)
        .spawn()
        .is_ok()
}
