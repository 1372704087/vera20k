//! Logging initialization helpers.
//!
//! Binaries use this to route `log`/`env_logger` output into a file under
//! `logs/` in the current working directory.

use std::backtrace::Backtrace;
use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use env_logger::{Builder, Env, Target};

static EXCEPTION_DUMP_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Records kept for the dashboard's log view. The file log is unbounded; this
/// copy is what the UI can actually render, and it is also what the report
/// table calls "recent".
const LOG_CAPACITY: usize = 4000;
/// One record's message ceiling. Loading a map logs sprite names by the
/// thousand, and a single runaway format string must not own the ring.
const LOG_MESSAGE_LIMIT: usize = 400;
const LOG_TARGET_LIMIT: usize = 64;

/// One captured log record, in wire-friendly form.
#[derive(Debug, Clone)]
pub struct LogLine {
    pub seq: u64,
    /// 1 = error, 2 = warn, 3 = info, 4 = debug, 5 = trace.
    pub level: u8,
    pub target: String,
    pub message: String,
    /// Milliseconds since logging started.
    pub t_ms: u64,
}

/// Bounded, thread-safe copy of everything the process has logged.
///
/// `env_logger` already formats and writes the file; this is a tee on the same
/// `log` records, so the dashboard shows exactly what the file received. The
/// reader cursor is atomic and single-consumer (the telemetry capture on the
/// game thread), which keeps per-frame capturing free of lock contention.
pub struct LogRing {
    started: Instant,
    next: AtomicU64,
    sent: AtomicU64,
    lines: Mutex<VecDeque<LogLine>>,
}

impl LogRing {
    fn new() -> Self {
        Self {
            started: Instant::now(),
            next: AtomicU64::new(0),
            sent: AtomicU64::new(0),
            lines: Mutex::new(VecDeque::with_capacity(LOG_CAPACITY)),
        }
    }

    fn push(&self, record: &log::Record<'_>) {
        let seq = self.next.fetch_add(1, Ordering::Relaxed) + 1;
        let mut message = record.args().to_string();
        truncate(&mut message, LOG_MESSAGE_LIMIT);
        let mut target = record.target().to_string();
        truncate(&mut target, LOG_TARGET_LIMIT);
        let line = LogLine {
            seq,
            level: rank(record.level()),
            target,
            message,
            t_ms: self.started.elapsed().as_millis() as u64,
        };
        let mut lines = self.lines.lock().unwrap_or_else(|poll| poll.into_inner());
        if lines.len() == LOG_CAPACITY {
            lines.pop_front();
        }
        lines.push_back(line);
    }

    /// Records added since the previous call.
    ///
    /// The second value counts records the ring dropped before anyone read
    /// them, so the UI can say "N lines lost" instead of lying by omission.
    pub fn take_since_last(&self, limit: usize) -> (Vec<LogLine>, u64) {
        let start = self.sent.load(Ordering::Relaxed);
        let lines = self.lines.lock().unwrap_or_else(|poll| poll.into_inner());
        let mut picked = Vec::new();
        let mut last = start;
        for line in lines.iter().filter(|line| line.seq > start) {
            if picked.len() == limit {
                break;
            }
            last = line.seq;
            picked.push(line.clone());
        }
        drop(lines);
        // Only advance past what was handed out; a burst larger than `limit`
        // is delivered over the next few frames instead of being discarded.
        self.sent.store(last, Ordering::Relaxed);
        let end = self.next.load(Ordering::Relaxed);
        let dropped = end
            .saturating_sub(start)
            .saturating_sub(picked.len() as u64);
        (picked, dropped)
    }

    /// How many records the ring currently holds.
    pub fn len(&self) -> usize {
        self.lines.lock().map(|lines| lines.len()).unwrap_or(0)
    }
}

fn rank(level: log::Level) -> u8 {
    match level {
        log::Level::Error => 1,
        log::Level::Warn => 2,
        log::Level::Info => 3,
        log::Level::Debug => 4,
        log::Level::Trace => 5,
    }
}

fn truncate(text: &mut String, limit: usize) {
    if text.chars().count() > limit {
        let cut = text
            .char_indices()
            .nth(limit)
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        text.truncate(cut);
        text.push('…');
    }
}

static RING: OnceLock<Arc<LogRing>> = OnceLock::new();

/// The process-wide ring, once the logger is installed.
pub fn ring() -> Option<&'static Arc<LogRing>> {
    RING.get()
}

/// Forwards every record to `env_logger` and to the ring.
struct TeeLogger {
    inner: env_logger::Logger,
    ring: Arc<LogRing>,
}

impl log::Log for TeeLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        self.inner.enabled(metadata)
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.inner.enabled(record.metadata()) {
            self.ring.push(record);
            self.inner.log(record);
        }
    }

    fn flush(&self) {
        self.inner.flush();
    }
}

/// Initialize env_logger to append to `logs/<name>.log`.
///
/// Honors `RUST_LOG` and defaults to `info` when the variable is unset.
/// Returns the resolved log file path on success.
pub fn init_file_logger(name: &str) -> io::Result<PathBuf> {
    let mut log_dir = std::env::current_dir()?;
    log_dir.push("logs");
    fs::create_dir_all(&log_dir)?;

    let mut log_path = log_dir;
    log_path.push(format!("{name}.log"));

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    // Suppress wgpu_hal's per-frame "Suboptimal present" warnings which
    // flood the log (~100K+ lines per session on AMD Vulkan).
    let env = Env::default().default_filter_or("info,wgpu_hal=error");
    let mut builder = Builder::from_env(env);
    builder.format_timestamp_secs();
    builder.target(Target::Pipe(Box::new(file)));

    // Build rather than `init()`: `init` claims the global logger slot for
    // env_logger alone, and the dashboard needs the same records. The tee
    // writes both the file and the in-memory ring.
    let logger = builder.build();
    let level = logger.filter();
    let ring = Arc::new(LogRing::new());
    let _ = RING.set(Arc::clone(&ring));
    if log::set_boxed_logger(Box::new(TeeLogger { inner: logger, ring })).is_ok() {
        log::set_max_level(level);
    }

    Ok(log_path)
}

/// Register a panic hook that writes the panic info and backtrace to the log
/// file when one is available. Each outermost panic creates or replaces
/// retail's `except.txt` report in the current working directory; recursive
/// hook entry is suppressed. The default stderr hook is preserved so terminal
/// users still see output.
pub fn install_panic_hook(log_path: Option<&Path>) {
    let prev_hook = std::panic::take_hook();
    let log_path = log_path.map(Path::to_owned);

    std::panic::set_hook(Box::new(move |info| {
        let Some(_guard) = claim_exception_dump(&EXCEPTION_DUMP_ACTIVE) else {
            prev_hook(info);
            return;
        };

        // Capture while the panic stack is still live.
        let backtrace = Backtrace::force_capture();

        if let Some(log_path) = &log_path
            && let Ok(mut file) = OpenOptions::new().append(true).open(log_path)
        {
            let _ = writeln!(file, "\n========== PANIC ==========");
            let _ = writeln!(file, "{info}");
            let _ = writeln!(file, "\n{backtrace}");
            let _ = writeln!(file, "===========================");
            let _ = file.flush();
        }

        let _ = write_exception_dump(info, &backtrace);

        // Preserve default stderr output for terminal users.
        prev_hook(info);
    }));
}

struct ExceptionDumpGuard<'a> {
    active: &'a AtomicBool,
}

impl Drop for ExceptionDumpGuard<'_> {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Release);
    }
}

#[inline]
fn claim_exception_dump(active: &AtomicBool) -> Option<ExceptionDumpGuard<'_>> {
    active
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .ok()
        .map(|_| ExceptionDumpGuard { active })
}

fn write_exception_dump(
    info: &std::panic::PanicHookInfo<'_>,
    backtrace: &Backtrace,
) -> io::Result<()> {
    let report = normalize_crlf(&format!(
        "{info}\nInternal Version {}\n\n{backtrace}\n",
        crate::util::version::retail_internal_version()
    ));

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(0).attributes(0x80);
    }

    let mut file = options.open("except.txt")?;
    file.write_all(report.as_bytes())?;
    file.flush()
}

fn normalize_crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use super::{claim_exception_dump, normalize_crlf};

    #[test]
    fn exception_dump_guard_blocks_recursion_then_rearms() {
        let active = AtomicBool::new(false);
        let outer = claim_exception_dump(&active).expect("first handler entry");
        assert!(
            claim_exception_dump(&active).is_none(),
            "recursive handler entry must not overwrite the active report"
        );

        drop(outer);
        assert!(
            claim_exception_dump(&active).is_some(),
            "a later independent handler entry must replace except.txt"
        );
    }

    #[test]
    fn exception_dump_uses_windows_line_endings() {
        assert_eq!(normalize_crlf("one\ntwo\r\n"), "one\r\ntwo\r\n");
    }
}
