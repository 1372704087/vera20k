//! Telemetry server: the browser dashboard's transport.
//!
//! A std-only WebSocket + static-file server on one port. It exists so the
//! visualization frontend can watch a live match from any browser without
//! touching the deterministic simulation: the app publishes
//! `sim::telemetry::TelemetrySnapshot` JSON at dashboard rate and this server
//! fans it out to connected clients.
//!
//! ## Deliberate simplifications (a debug tool, not a product server)
//!
//! - **No async runtime.** One accept thread plus one writer thread per
//!   client, all on `std::thread`; the payload is small and the cadence is
//!   5–10 Hz. `publish` serializes once on the caller's thread and hands the
//!   same string to every client queue.
//! - **Slow clients drop frames, never block the game.** Each client queue
//!   holds [`CLIENT_QUEUE_DEPTH`] snapshots; a full queue drops the new
//!   frame. A dead client is pruned when its queue write fails.
//! - **Inbound frames are commands, not chat.** Each client's writer thread
//!   also reads masked text frames (bounded at [`MAX_INBOUND_FRAME`]) and
//!   decodes them as [`DebugCommand`]s onto one bounded queue; the game
//!   thread drains it once per frame. Anything else (ping/pong/binary,
//!   malformed JSON, oversized frames) is ignored or closes that client.
//! - **Stop is polled.** The listener is nonblocking and the accept loop
//!   sleeps [`ACCEPT_POLL`], so `Drop` joins the thread within one poll.
//!
//! ## Dependency rules
//! - Part of the app layer; depends on `sim::telemetry` types, the
//!   [`crate::app::telemetry`] command types, and `std` only.

use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::Duration;

use crate::app::telemetry::DebugCommand;

/// RFC 6455 handshake GUID, concatenated to the client's key before the
/// SHA-1 digest.
const WS_MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
/// Request heads larger than this are refused rather than buffered.
const MAX_HTTP_HEAD: usize = 8 * 1024;
/// Snapshots buffered per client before frames start dropping.
const CLIENT_QUEUE_DEPTH: usize = 4;
/// Accept-loop poll interval; bounds `Drop`'s join wait.
const ACCEPT_POLL: Duration = Duration::from_millis(50);
/// Writer-thread wake interval for the stop check.
const WRITER_POLL: Duration = Duration::from_millis(250);
/// Read timeout while polling the client socket for command frames; the
/// write path stays blocking.
const INBOUND_READ_TIMEOUT: Duration = Duration::from_millis(1);
/// Command frames larger than this are a protocol violation (a switch click
/// is well under 100 bytes).
const MAX_INBOUND_FRAME: usize = 4096;
/// Commands buffered for the game thread; a full queue drops the newest.
const COMMAND_QUEUE_DEPTH: usize = 64;
/// Commands handed to the app in one drain (bounds per-frame work).
const MAX_COMMANDS_PER_DRAIN: usize = 32;

/// Compute the `Sec-WebSocket-Accept` value for a client's
/// `Sec-WebSocket-Key`: base64 of SHA-1(key + magic GUID).
pub fn websocket_accept_key(client_key: &str) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(WS_MAGIC.as_bytes());
    base64_encode(&hasher.finalize())
}

/// Encode one server-to-client text WebSocket frame (unmasked, as RFC 6455
/// requires of servers). Handles all three payload-length classes.
pub fn encode_text_frame(payload: &str) -> Vec<u8> {
    let bytes = payload.as_bytes();
    let mut frame = Vec::with_capacity(bytes.len() + 10);
    frame.push(0x81); // FIN + opcode 1 (text)
    match bytes.len() {
        len @ 0..=125 => frame.push(len as u8),
        len @ 126..=65535 => {
            frame.push(126);
            frame.extend_from_slice(&(len as u16).to_be_bytes());
        }
        len => {
            frame.push(127);
            frame.extend_from_slice(&(len as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(bytes);
    frame
}

/// Standard base64 (RFC 4648) — the only alphabet the handshake needs.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = u32::from(chunk[0]);
        let b1 = chunk.get(1).copied().map(u32::from).unwrap_or(0);
        let b2 = chunk.get(2).copied().map(u32::from).unwrap_or(0);
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) as usize & 0x3F] as char);
        out.push(ALPHABET[(triple >> 12) as usize & 0x3F] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(triple >> 6) as usize & 0x3F] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[triple as usize & 0x3F] as char
        } else {
            '='
        });
    }
    out
}

/// Shared server state: the client registry, the command intake and the stop
/// flag.
struct Shared {
    clients: Mutex<Vec<SyncSender<String>>>,
    /// Bounded intake for browser commands; the game thread drains it.
    command_tx: Mutex<SyncSender<DebugCommand>>,
    stop: AtomicBool,
}

impl Shared {
    fn clients(&self) -> MutexGuard<'_, Vec<SyncSender<String>>> {
        // A panicking writer must not poison the registry for the game thread.
        self.clients
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn command_tx(&self) -> MutexGuard<'_, SyncSender<DebugCommand>> {
        self.command_tx
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }
}

/// A running telemetry server. Dropping it stops the accept thread and every
/// client writer within one poll interval.
pub struct TelemetryServer {
    addr: SocketAddr,
    shared: Arc<Shared>,
    accept_thread: Option<thread::JoinHandle<()>>,
    /// The game-thread end of the command queue; only this thread drains it.
    commands: Mutex<mpsc::Receiver<DebugCommand>>,
    /// Directory this server hands out files from. The end-of-match report
    /// writes under it so the dashboard can link the file it just wrote.
    web_root: PathBuf,
}

impl TelemetryServer {
    /// Bind `addr` and serve the dashboard files from `web_root`.
    pub fn start(addr: SocketAddr, web_root: PathBuf) -> std::io::Result<Self> {
        let listener = TcpListener::bind(addr)?;
        let bound = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let (command_tx, command_rx) = mpsc::sync_channel::<DebugCommand>(COMMAND_QUEUE_DEPTH);
        let shared = Arc::new(Shared {
            clients: Mutex::new(Vec::new()),
            command_tx: Mutex::new(command_tx),
            stop: AtomicBool::new(false),
        });
        let accept_shared = Arc::clone(&shared);
        // The accept loop owns its own copy: the server keeps the root so the
        // end-of-match report can write into the directory it serves from.
        let accept_thread = thread::Builder::new()
            .name("telemetry-accept".to_string())
            .spawn({
                let serve_root = web_root.clone();
                move || accept_loop(listener, accept_shared, serve_root)
            })?;
        Ok(Self {
            addr: bound,
            shared,
            accept_thread: Some(accept_thread),
            commands: Mutex::new(command_rx),
            web_root,
        })
    }

    /// The directory this server serves files from.
    pub fn web_root(&self) -> &Path {
        &self.web_root
    }

    /// The bound address (equals the requested one unless port 0 was asked).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Fan one serialized snapshot out to every client, pruning the dead.
    /// Never blocks: a full client queue drops this frame.
    pub fn publish(&self, json: String) {
        self.shared
            .clients()
            .retain(|client| client.try_send(json.clone()).is_ok());
    }

    /// Take the browser commands that arrived since the last call. Called
    /// from the game thread once per frame; never blocks.
    pub fn drain_commands(&self) -> Vec<DebugCommand> {
        let commands = self
            .commands
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut drained = Vec::new();
        while drained.len() < MAX_COMMANDS_PER_DRAIN {
            match commands.try_recv() {
                Ok(command) => drained.push(command),
                Err(_) => break,
            }
        }
        drained
    }
}

impl Drop for TelemetryServer {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
    }
}

fn accept_loop(listener: TcpListener, shared: Arc<Shared>, web_root: PathBuf) {
    while !shared.stop.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nodelay(true).ok();
                if let Err(error) = serve_connection(stream, &shared, &web_root) {
                    log::debug!("telemetry connection closed: {error}");
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(ACCEPT_POLL);
            }
            Err(error) => {
                log::warn!("telemetry accept failed: {error}");
                thread::sleep(ACCEPT_POLL);
            }
        }
    }
}

/// Serve one accepted connection: either the WebSocket upgrade on `/ws` or a
/// static file, then close.
fn serve_connection(
    mut stream: TcpStream,
    shared: &Arc<Shared>,
    web_root: &Path,
) -> std::io::Result<()> {
    let head = read_request_head(&mut stream)?;
    let Some((method, target)) = request_line(&head) else {
        return serve_status(&mut stream, 400, "bad request");
    };
    if method != "GET" {
        return serve_status(&mut stream, 405, "method not allowed");
    }
    let path = target.split('?').next().unwrap_or(target);
    if path == "/ws" {
        return upgrade_websocket(&mut stream, &head, shared);
    }
    serve_static(&mut stream, web_root, path)
}

/// Read request bytes up to the head terminator, capped at
/// [`MAX_HTTP_HEAD`].
fn read_request_head(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut buf = [0u8; MAX_HTTP_HEAD];
    let mut filled = 0usize;
    loop {
        let read = stream.read(&mut buf[filled..])?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "connection closed before the request head",
            ));
        }
        filled += read;
        if let Some(end) = find_subsequence(&buf[..filled], b"\r\n\r\n") {
            return String::from_utf8(buf[..end].to_vec())
                .map_err(|_| std::io::Error::other("request head is not UTF-8"));
        }
        if filled == buf.len() {
            return Err(std::io::Error::other("request head too large"));
        }
    }
}

/// First line of the request head: `("GET", "/path")`.
fn request_line(head: &str) -> Option<(&str, &str)> {
    let line = head.lines().next()?;
    let mut parts = line.split_whitespace();
    let method = parts.next()?;
    let target = parts.next()?;
    Some((method, target))
}

/// Header value lookup, case-insensitive on the name (RFC 7230 §3.2).
fn header_value<'a>(head: &'a str, name: &str) -> Option<&'a str> {
    head.lines()
        .skip(1)
        .filter_map(|line| line.split_once(':'))
        .find(|(key, _)| key.trim().eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
}

/// Complete the RFC 6455 upgrade and register a client writer.
fn upgrade_websocket(
    stream: &mut TcpStream,
    head: &str,
    shared: &Arc<Shared>,
) -> std::io::Result<()> {
    let Some(key) = header_value(head, "Sec-WebSocket-Key") else {
        return serve_status(stream, 400, "missing Sec-WebSocket-Key");
    };
    let response = format!(
        "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
        websocket_accept_key(key)
    );
    stream.write_all(response.as_bytes())?;
    let (tx, rx) = mpsc::sync_channel::<String>(CLIENT_QUEUE_DEPTH);
    shared.clients().push(tx);
    let writer_shared = Arc::clone(shared);
    let writer_stream = stream.try_clone()?;
    thread::Builder::new()
        .name("telemetry-writer".to_string())
        .spawn(move || writer_loop(writer_stream, rx, writer_shared))?;
    Ok(())
}

fn writer_loop(mut stream: TcpStream, rx: mpsc::Receiver<String>, shared: Arc<Shared>) {
    // Command frames are read on this same socket. A short read timeout keeps
    // the write path blocking while the poll stays cheap.
    stream.set_read_timeout(Some(INBOUND_READ_TIMEOUT)).ok();
    let mut inbound = InboundBuffer::default();
    loop {
        match inbound.poll(&mut stream) {
            Ok(payloads) => {
                for payload in payloads {
                    match serde_json::from_slice::<DebugCommand>(&payload) {
                        Ok(command) => {
                            if shared.command_tx().try_send(command).is_err() {
                                log::debug!("telemetry command queue full; one command dropped");
                            }
                        }
                        Err(error) => log::debug!("telemetry command ignored: {error}"),
                    }
                }
            }
            // Peer closed or violated the protocol: the registry prunes the
            // sender on the next publish.
            Err(_) => break,
        }
        match rx.recv_timeout(WRITER_POLL) {
            Ok(json) => {
                let frame = encode_text_frame(&json);
                if stream.write_all(&frame).is_err() {
                    break; // peer went away; the registry prunes the sender
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if shared.stop.load(Ordering::Relaxed) {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

/// Bytes read from one browser since the last complete frame.
#[derive(Default)]
struct InboundBuffer {
    bytes: Vec<u8>,
}

impl InboundBuffer {
    /// Read whatever is pending and return every complete text payload.
    /// `Err` means the client closed or broke the protocol.
    fn poll(&mut self, stream: &mut TcpStream) -> std::io::Result<Vec<Vec<u8>>> {
        let mut payloads = Vec::new();
        let mut scratch = [0u8; 1024];
        loop {
            loop {
                match parse_client_frame(&self.bytes) {
                    Ok(Some((payload, consumed))) => {
                        self.bytes.drain(..consumed);
                        if let Some(payload) = payload {
                            payloads.push(payload);
                        }
                    }
                    Ok(None) => break,
                    Err(reason) => return Err(std::io::Error::other(reason)),
                }
            }
            match stream.read(&mut scratch) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "client closed the socket",
                    ));
                }
                Ok(read) => {
                    self.bytes.extend_from_slice(&scratch[..read]);
                    if self.bytes.len() > MAX_INBOUND_FRAME * 2 {
                        return Err(std::io::Error::other("inbound buffer overflow"));
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                    ) =>
                {
                    return Ok(payloads);
                }
                Err(error) => return Err(error),
            }
        }
    }
}

/// Parse one client-to-server frame from the front of `buf`.
///
/// `Ok(None)` means "need more bytes"; `Ok(Some((payload, consumed)))` is one
/// complete frame, where `payload` is `None` for control/binary frames we do
/// not consume. `Err` is a protocol violation (unmasked or fragmented client
/// frame, oversized payload, close frame).
fn parse_client_frame(buf: &[u8]) -> Result<Option<(Option<Vec<u8>>, usize)>, &'static str> {
    if buf.len() < 2 {
        return Ok(None);
    }
    let first = buf[0];
    if first & 0x80 == 0 {
        return Err("fragmented client frames are not supported");
    }
    let opcode = first & 0x0F;
    let second = buf[1];
    if second & 0x80 == 0 {
        return Err("client frames must be masked");
    }
    let mut len = u64::from(second & 0x7F);
    let mut header = 2usize;
    if len == 126 {
        if buf.len() < 4 {
            return Ok(None);
        }
        len = u64::from(u16::from_be_bytes([buf[2], buf[3]]));
        header = 4;
    } else if len == 127 {
        if buf.len() < 10 {
            return Ok(None);
        }
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&buf[2..10]);
        len = u64::from_be_bytes(bytes);
        header = 10;
    }
    if len > MAX_INBOUND_FRAME as u64 {
        return Err("command frame too large");
    }
    let total = header + 4 + len as usize;
    if buf.len() < total {
        return Ok(None);
    }
    let mask = [
        buf[header],
        buf[header + 1],
        buf[header + 2],
        buf[header + 3],
    ];
    let mut payload = buf[header + 4..total].to_vec();
    for (index, byte) in payload.iter_mut().enumerate() {
        *byte ^= mask[index % 4];
    }
    match opcode {
        0x1 => Ok(Some((Some(payload), total))),
        0x8 => Err("client sent a close frame"),
        0x0 | 0x2 | 0x9 | 0xA => Ok(Some((None, total))),
        _ => Err("unsupported opcode"),
    }
}

/// Serve a dashboard file from the web root. Paths are confined to the root:
/// no `..` components, no absolute escapes, and a directory or the root
/// itself answers with `index.html`.
fn serve_static(stream: &mut TcpStream, web_root: &Path, path: &str) -> std::io::Result<()> {
    let relative = path.trim_start_matches('/');
    let relative = if relative.is_empty() {
        "index.html"
    } else {
        relative
    };
    if relative.split(['/', '\\']).any(|part| part == "..") {
        return serve_status(stream, 404, "not found");
    }
    let file_path = web_root.join(relative);
    let body = match std::fs::read(&file_path) {
        Ok(body) => body,
        Err(_) => return serve_status(stream, 404, "not found"),
    };
    let content_type = content_type_for(&file_path);
    // A dev tool serves its freshest files: no cache headers at all would let
    // browsers heuristic-cache the dashboard across rebuilds.
    let header = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(&body)
}

fn serve_status(stream: &mut TcpStream, code: u16, reason: &str) -> std::io::Result<()> {
    let body = reason.as_bytes();
    let header = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)
}

fn content_type_for(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript",
        Some("css") => "text/css",
        Some("json") => "application/json",
        Some("map") => "application/json",
        _ => "application/octet-stream",
    }
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// The environment variable naming the bind address (e.g.
/// `127.0.0.1:9741`). Unset means no server.
pub const ENV_ADDR: &str = "VERA_TELEMETRY_ADDR";
/// The environment variable naming the dashboard directory. Defaults to
/// `tools/telemetry-dashboard` (run from the repo root).
pub const ENV_WEB_ROOT: &str = "VERA_TELEMETRY_WEB";
/// Default dashboard directory, relative to the working directory.
pub const DEFAULT_WEB_ROOT: &str = "tools/telemetry-dashboard";

/// Start a server from the environment, if `VERA_TELEMETRY_ADDR` is set.
pub fn start_from_env() -> Option<Result<TelemetryServer, String>> {
    let raw = std::env::var(ENV_ADDR).ok()?;
    let addr: SocketAddr = match raw.parse() {
        Ok(addr) => addr,
        Err(_) => {
            let ip: IpAddr = raw
                .parse()
                .map_err(|error| format!("{ENV_ADDR}={raw} is not an address: {error}"))
                .ok()?;
            SocketAddr::new(ip, DEFAULT_PORT)
        }
    };
    let web_root = std::env::var_os(ENV_WEB_ROOT)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_WEB_ROOT));
    Some(
        TelemetryServer::start(addr, web_root)
            .map_err(|error| format!("telemetry server on {addr}: {error}")),
    )
}

/// Port used when `VERA_TELEMETRY_ADDR` names a bare IP.
pub const DEFAULT_PORT: u16 = 9741;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accept_key_matches_the_rfc6455_vector() {
        // RFC 6455 §1.3 worked example.
        assert_eq!(
            websocket_accept_key("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    #[test]
    fn text_frame_encodes_all_length_classes() {
        let short = encode_text_frame("hi");
        assert_eq!(short, vec![0x81, 0x02, b'h', b'i']);

        let medium_payload = "x".repeat(200);
        let medium = encode_text_frame(&medium_payload);
        assert_eq!(medium[0], 0x81);
        assert_eq!(medium[1], 126);
        assert_eq!(&medium[2..4], &200u16.to_be_bytes());
        assert_eq!(medium.len(), 4 + 200);

        let long_payload = "y".repeat(70_000);
        let long = encode_text_frame(&long_payload);
        assert_eq!(long[1], 127);
        assert_eq!(&long[2..10], &70_000u64.to_be_bytes());
        assert_eq!(long.len(), 10 + 70_000);
    }

    /// A raw test client: performs the upgrade, then reads one text frame's
    /// payload.
    struct TestClient {
        stream: TcpStream,
    }

    impl TestClient {
        fn connect(addr: SocketAddr, path: &str) -> std::io::Result<Self> {
            let mut stream = TcpStream::connect(addr)?;
            write!(
                stream,
                "GET {path} HTTP/1.1\r\nHost: localhost\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nConnection: Upgrade\r\nUpgrade: websocket\r\n\r\n"
            )?;
            let mut byte = [0u8; 1];
            // The 101 response is short; read until the accept header ends.
            let mut head = String::new();
            loop {
                let read = stream.read(&mut byte)?;
                if read == 0 {
                    break;
                }
                head.push(byte[0] as char);
                if head.ends_with("\r\n\r\n") {
                    break;
                }
            }
            assert!(head.starts_with("HTTP/1.1 101"), "upgrade failed: {head:?}");
            assert!(head.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));
            Ok(Self { stream })
        }

        /// Send one masked text frame, as a browser must.
        fn send_text(&mut self, payload: &str) {
            let mask = [0x12u8, 0x34, 0x56, 0x78];
            let bytes = payload.as_bytes();
            assert!(bytes.len() <= 125, "test payload stays in the short class");
            let mut frame = vec![0x81u8, 0x80 | bytes.len() as u8];
            frame.extend_from_slice(&mask);
            for (index, byte) in bytes.iter().enumerate() {
                frame.push(byte ^ mask[index % 4]);
            }
            self.stream.write_all(&frame).expect("write command frame");
        }

        fn read_frame_payload(&mut self) -> String {
            let mut header = [0u8; 2];
            self.stream.read_exact(&mut header).expect("frame header");
            assert_eq!(header[0], 0x81, "expected a FIN text frame");
            let len = match header[1] {
                len @ 0..=125 => len as u64,
                126 => {
                    let mut ext = [0u8; 2];
                    self.stream.read_exact(&mut ext).expect("extended length");
                    u64::from(u16::from_be_bytes(ext))
                }
                127 => {
                    let mut ext = [0u8; 8];
                    self.stream.read_exact(&mut ext).expect("extended length");
                    u64::from_be_bytes(ext)
                }
                other => panic!("unexpected length marker {other}"),
            };
            let mut payload = vec![0u8; len as usize];
            self.stream.read_exact(&mut payload).expect("frame payload");
            String::from_utf8(payload).expect("payload is UTF-8 JSON")
        }
    }

    fn http_get(addr: SocketAddr, path: &str) -> (String, String) {
        let mut stream = TcpStream::connect(addr).expect("connect");
        write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").expect("write request");
        let mut response = String::new();
        stream.read_to_string(&mut response).expect("read response");
        let (head, body) = response.split_once("\r\n\r\n").expect("head/body split");
        (head.to_string(), body.to_string())
    }

    #[test]
    fn server_serves_web_root_and_rejects_escapes() {
        let root = std::env::temp_dir().join(format!("vera-telemetry-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temp dir");
        std::fs::write(root.join("index.html"), "<html>dashboard</html>").expect("write index");

        let server = TelemetryServer::start("127.0.0.1:0".parse().unwrap(), root.clone())
            .expect("server starts");
        let addr = server.addr();

        let (head, body) = http_get(addr, "/");
        assert!(head.starts_with("HTTP/1.1 200"), "{head}");
        assert!(head.contains("text/html"));
        assert_eq!(body, "<html>dashboard</html>");

        let (head, _) = http_get(addr, "/../Cargo.toml");
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");

        let (head, _) = http_get(addr, "/vendor/three.module.js");
        assert!(head.starts_with("HTTP/1.1 404"), "{head}");

        drop(server);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn parse_client_frame_unmasks_and_rejects_unmasked() {
        // Masked "hi": 0x81, MASK|2, key, 'h'^k0, 'i'^k1.
        let mut masked = vec![0x81u8, 0x82, 1, 2, 3, 4];
        masked.push(b'h' ^ 1);
        masked.push(b'i' ^ 2);
        let (payload, consumed) = parse_client_frame(&masked).unwrap().unwrap();
        assert_eq!(payload.as_deref(), Some(&b"hi"[..]));
        assert_eq!(consumed, masked.len());

        // An unmasked client frame is a protocol violation (RFC 6455 §5.1).
        assert!(parse_client_frame(&[0x81, 0x02, b'h', b'i']).is_err());
        // A partial frame asks for more bytes rather than failing.
        assert!(parse_client_frame(&masked[..4]).unwrap().is_none());
    }

    #[test]
    fn browser_commands_reach_the_server_queue() {
        use crate::app::telemetry::{DebugCommand, DebugSwitch};

        let root = std::env::temp_dir().join(format!("vera-telemetry-cmd-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temp dir");
        let server = TelemetryServer::start("127.0.0.1:0".parse().unwrap(), root.clone())
            .expect("server starts");
        let addr = server.addr();

        let mut client = TestClient::connect(addr, "/ws").expect("upgrade");
        client.send_text(r#"{"cmd":"toggle","switch":"heightmap"}"#);
        client.send_text(r#"{"cmd":"frame_step"}"#);

        // The writer thread polls the socket on its wake interval; give it one.
        let mut received = Vec::new();
        for _ in 0..40 {
            received.extend(server.drain_commands());
            if received.len() >= 2 {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        assert_eq!(
            received,
            vec![
                DebugCommand::Toggle {
                    switch: DebugSwitch::Heightmap
                },
                DebugCommand::FrameStep,
            ]
        );

        drop(client);
        drop(server);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn server_publishes_snapshots_to_websocket_clients() {
        let root = std::env::temp_dir().join(format!("vera-telemetry-ws-{}", std::process::id()));
        std::fs::create_dir_all(&root).expect("temp dir");
        let server = TelemetryServer::start("127.0.0.1:0".parse().unwrap(), root.clone())
            .expect("server starts");
        let addr = server.addr();

        let mut client = TestClient::connect(addr, "/ws").expect("upgrade");
        let snapshot = crate::sim::world::Simulation::new().telemetry_snapshot();
        server.publish(serde_json::to_string(&snapshot).expect("serialize"));
        let received: serde_json::Value =
            serde_json::from_str(&client.read_frame_payload()).expect("payload parses as JSON");
        assert_eq!(
            received["version"],
            crate::sim::telemetry::TELEMETRY_SCHEMA_VERSION
        );
        assert_eq!(received["entities_total"], 0);

        drop(client);
        drop(server);
        std::fs::remove_dir_all(&root).ok();
    }
}
