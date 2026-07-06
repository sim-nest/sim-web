//! Minimal blocking HTTP/1.1 server for the Web shell.
//!
//! The server serves embedded assets, the cookbook API adapter, and the Atelier
//! shell cache API. Runtime transport remains the Intent/Scene bridge over
//! `realize`/`EvalFabric`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::time::Duration;

/// Largest request body the shell will read. A larger declared `Content-Length`
/// is rejected with 413 before any allocation, so a hostile header cannot force
/// an unbounded `vec![0u8; n]`.
const MAX_BODY_BYTES: usize = 1 << 20; // 1 MiB.

/// Largest single request line or header line the shell will read. Matches the
/// 64 KiB head cap the peer HTTP readers in sim-agent-net enforce, so a hostile
/// multi-gigabyte request line or header cannot grow memory unbounded before it
/// is rejected with 413.
const MAX_HEAD_LINE_BYTES: usize = 64 * 1024;

/// Largest number of header lines the shell will read before rejecting the
/// request, so an endless stream of tiny headers cannot grow memory unbounded.
const MAX_HEADER_COUNT: usize = 256;

/// Per-read timeout on a connection, so a peer that declares a body but then
/// dribbles (or stalls) cannot block the single-threaded server forever.
const READ_TIMEOUT: Duration = Duration::from_secs(30);

use crate::assets::asset_for;
use crate::atelier::AtelierWebState;
use crate::live::{
    DEFAULT_PANE, DEFAULT_RESOURCE, LiveSession, decode_intent_body, encode_patches, encode_scene,
    error_json,
};
use sim_codec_algol::AlgolCodecLib;
use sim_codec_binary::BinaryCodecLib;
use sim_codec_chat::ChatCodecLib;
use sim_codec_json::JsonCodecLib;
use sim_kernel::{Cx, Result as SimResult};
use sim_lib_server::{CookbookWebResponse, CookbookWebState};
use sim_lib_stream_core::install_stream_core_shapes_lib;

/// Configuration for the shell server.
#[derive(Debug)]
pub struct ServeConfig {
    /// The address to bind, e.g. `127.0.0.1:8787`.
    pub addr: String,
    /// Directory containing generated Atelier cache files.
    pub atelier_root: PathBuf,
    /// Install codecs and shapes, then return before binding the socket. Lets a
    /// caller confirm the serve verb dispatches and boots without holding a port.
    pub dry_run: bool,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8787".to_owned(),
            atelier_root: PathBuf::from(".sim/atelier"),
            dry_run: false,
        }
    }
}

/// Bind and serve the shell until the process is terminated, using the
/// bootloader-provided `cx` as the cookbook eval sandbox. The `sim-web-shell`
/// binary boots through `sim_run_core::Bootloader` (see `cli.rs`), which loads the
/// `codec/lisp` boot codec and dispatches the `serve` verb into this function with
/// a ready `cx`. Read-eval is granted to that `cx` by the bootloader at the
/// web-serve composition point (`configure_web_bootloader`, through the boot
/// session's host GrantSeat), not self-granted here; `run_recipe` gates each run
/// on it (REVIEW_12 F4/F23).
pub fn serve_with_cx(cx: &mut Cx, config: &ServeConfig) -> std::io::Result<()> {
    install_codecs(cx).map_err(io_error)?;
    install_stream_core_shapes_lib(cx).map_err(io_error)?;

    if config.dry_run {
        println!("sim-web-shell: dry-run OK");
        return Ok(());
    }

    let listener = bind(&config.addr)?;
    let local = listener.local_addr()?;
    let mut state = ShellState::new(config, cx)?;
    println!("sim-web-shell: serving shell on http://{local}");
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                if let Err(err) = handle(stream, &mut state) {
                    eprintln!("sim-web-shell: connection error: {err}");
                }
            }
            Err(err) => eprintln!("sim-web-shell: accept error: {err}"),
        }
    }
    Ok(())
}

fn bind(addr: &str) -> std::io::Result<TcpListener> {
    let resolved = addr.to_socket_addrs()?.next().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "no socket address")
    })?;
    TcpListener::bind(resolved)
}

struct ShellState<'a> {
    atelier: AtelierWebState,
    cookbook: CookbookWebState,
    cookbook_cx: &'a mut Cx,
    live: LiveSession,
}

impl<'a> ShellState<'a> {
    fn new(config: &ServeConfig, cx: &'a mut Cx) -> std::io::Result<Self> {
        // The cookbook eval sandbox is the bootloader-provided `cx`, which already
        // carries the standard distribution the recipes require and read-eval,
        // granted by the bootloader at the web-serve composition point. run_recipe
        // gates each run on read-eval, so a session that never runs a recipe never
        // uses it.
        Ok(Self {
            atelier: AtelierWebState::load(config.atelier_root.clone()),
            cookbook: CookbookWebState::seeded().map_err(io_error)?,
            cookbook_cx: cx,
            live: LiveSession::new().map_err(io_error)?,
        })
    }
}

/// Installs the cookbook eval codecs. `codec/lisp` is the boot codec provided by the
/// bootloader, so it is not reinstalled here (that would double-register the symbol).
fn install_codecs(cx: &mut Cx) -> SimResult<()> {
    let json = JsonCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&json)?;
    let binary = BinaryCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&binary)?;
    let chat = ChatCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&chat)?;
    let algol = AlgolCodecLib::new(cx.registry_mut().fresh_codec_id());
    cx.load_lib(&algol)?;
    Ok(())
}

fn io_error(err: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

fn handle(mut stream: TcpStream, state: &mut ShellState<'_>) -> std::io::Result<()> {
    // Bound how long a single read may block; a slow-loris peer cannot pin the
    // server. A failure to set the timeout is non-fatal (e.g. exotic streams).
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let request = match read_request(&mut stream)? {
        ReadOutcome::Request(request) => request,
        ReadOutcome::TooLarge => {
            write_response(
                &mut stream,
                413,
                "Payload Too Large",
                "text/plain; charset=utf-8",
                b"payload too large",
            )?;
            return Ok(());
        }
        ReadOutcome::Invalid => {
            write_response(
                &mut stream,
                400,
                "Bad Request",
                "text/plain; charset=utf-8",
                b"bad request",
            )?;
            return Ok(());
        }
    };
    if path_of(&request.target) == "/api/session/intent" {
        return write_session_intent(&mut stream, &request, &mut state.live);
    }
    if path_of(&request.target) == "/api/session/open" {
        return write_session_open(&mut stream, &request, &mut state.live);
    }
    if request.target.starts_with("/api/cookbook") {
        // read-eval was granted to cookbook_cx by the bootloader (see cli.rs);
        // run_recipe gates each run on it (REVIEW_12 F4/F23).
        let response = state.cookbook.handle_request(
            &request.method,
            &request.target,
            Some(&mut *state.cookbook_cx),
        );
        return write_cookbook_response(&mut stream, &response);
    }
    if let Some(response) = state.atelier.response(&request.method, &request.target) {
        return write_response(
            &mut stream,
            response.status,
            status_text(response.status),
            response.content_type,
            response.body.as_bytes(),
        );
    }
    if request.method != "GET" {
        write_response(
            &mut stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed",
        )?;
        return Ok(());
    }
    match asset_for(&request.target) {
        Some(asset) => write_response(&mut stream, 200, "OK", asset.content_type, asset.body),
        None => write_response(
            &mut stream,
            404,
            "Not Found",
            "text/plain; charset=utf-8",
            b"not found",
        ),
    }
}

#[derive(Debug)]
struct RequestLine {
    method: String,
    target: String,
    body: String,
}

/// The outcome of reading one request: a parsed request, an oversized body
/// (answer 413), or an otherwise-unparseable request (answer 400).
#[derive(Debug)]
enum ReadOutcome {
    Request(RequestLine),
    TooLarge,
    Invalid,
}

/// Read the request line, scan headers for `Content-Length`, and read the body.
fn read_request(stream: &mut TcpStream) -> std::io::Result<ReadOutcome> {
    let mut reader = BufReader::new(stream);
    read_request_from(&mut reader)
}

/// Read one line into `buf` (clearing it first), reading at most `cap` bytes.
/// Returns `Ok(None)` when the line would exceed the cap (the caller answers
/// 413), `Ok(Some(0))` at end of input, and `Ok(Some(n))` for a line of `n`
/// bytes otherwise. The `take(cap + 1)` ceiling means a hostile unterminated
/// line is bounded before it can grow memory.
fn read_capped_line(
    reader: &mut impl BufRead,
    buf: &mut String,
    cap: usize,
) -> std::io::Result<Option<usize>> {
    buf.clear();
    let read = Read::take(&mut *reader, cap as u64 + 1).read_line(buf)?;
    if buf.len() > cap {
        return Ok(None);
    }
    Ok(Some(read))
}

/// Parse a request from any buffered reader, bounding the body at
/// [`MAX_BODY_BYTES`]. A declared `Content-Length` over the cap returns
/// [`ReadOutcome::TooLarge`] before any allocation, and the body read is capped
/// at the same limit so a lying header cannot over-read.
fn read_request_from(reader: &mut impl BufRead) -> std::io::Result<ReadOutcome> {
    let mut request_line = String::new();
    match read_capped_line(reader, &mut request_line, MAX_HEAD_LINE_BYTES)? {
        // An oversized request line is refused with 413 before it can grow memory.
        None => return Ok(ReadOutcome::TooLarge),
        Some(0) => return Ok(ReadOutcome::Invalid),
        Some(_) => {}
    }
    // Drain the rest of the header block, capturing the body length, so the peer
    // is not left mid-write. Cap each header line and the header count so a
    // hostile peer cannot grow memory unbounded with one huge header or an
    // endless stream of tiny ones.
    let mut content_length = 0usize;
    let mut header = String::new();
    let mut header_count = 0usize;
    loop {
        header_count += 1;
        if header_count > MAX_HEADER_COUNT {
            return Ok(ReadOutcome::TooLarge);
        }
        let read = match read_capped_line(reader, &mut header, MAX_HEAD_LINE_BYTES)? {
            None => return Ok(ReadOutcome::TooLarge),
            Some(read) => read,
        };
        if read == 0 || header == "\r\n" || header == "\n" {
            break;
        }
        if let Some((name, value)) = header.split_once(':')
            && name.trim().eq_ignore_ascii_case("content-length")
        {
            content_length = value.trim().parse().unwrap_or(0);
        }
    }
    // Reject an oversized declared body before allocating anything for it.
    if content_length > MAX_BODY_BYTES {
        return Ok(ReadOutcome::TooLarge);
    }
    let mut body = vec![0u8; content_length];
    if content_length > 0 {
        // Read at most the cap even if the header under-declared (defence in
        // depth): `body` is already capped, so `read_exact` cannot grow it.
        reader.read_exact(&mut body)?;
    }
    let body = String::from_utf8_lossy(&body).into_owned();
    let mut parts = request_line.split_whitespace();
    let method = parts.next();
    let target = parts.next();
    match (method, target) {
        (Some(method @ ("GET" | "POST")), Some(target)) => Ok(ReadOutcome::Request(RequestLine {
            method: method.to_owned(),
            target: target.to_owned(),
            body,
        })),
        _ => Ok(ReadOutcome::Invalid),
    }
}

/// Handle `POST /api/session/intent`: decode the Intent from the request body,
/// submit it to the live session, and respond with the resulting Scene patches.
/// Decode and validation failures respond with a structured error, never a
/// panic.
fn write_session_intent(
    stream: &mut (impl Write + ?Sized),
    request: &RequestLine,
    live: &mut LiveSession,
) -> std::io::Result<()> {
    if request.method != "POST" {
        return write_json(stream, 405, &error_json("intent route requires POST"));
    }
    let pane = query_value(&request.target, "pane").unwrap_or_else(|| DEFAULT_PANE.to_owned());
    let intent = match decode_intent_body(&request.body) {
        Ok(intent) => intent,
        Err(err) => return write_json(stream, 400, &error_json(&err)),
    };
    match live.submit(&pane, &intent) {
        Ok(updates) => write_json(stream, 200, &encode_patches(&updates)),
        Err(err) => write_json(stream, 400, &error_json(&err.to_string())),
    }
}

/// Handle `GET /api/session/open?resource=...&pane=...`: open the resource into
/// the pane and respond with its initial Scene.
fn write_session_open(
    stream: &mut (impl Write + ?Sized),
    request: &RequestLine,
    live: &mut LiveSession,
) -> std::io::Result<()> {
    if request.method != "GET" {
        return write_json(stream, 405, &error_json("open route requires GET"));
    }
    let resource =
        query_value(&request.target, "resource").unwrap_or_else(|| DEFAULT_RESOURCE.to_owned());
    let pane = query_value(&request.target, "pane").unwrap_or_else(|| DEFAULT_PANE.to_owned());
    match live.open(&resource, &pane) {
        Ok(scene) => write_json(stream, 200, &encode_scene(&scene)),
        Err(err) => write_json(stream, 400, &error_json(&err.to_string())),
    }
}

/// The path portion of a request target, with any query or fragment stripped.
fn path_of(target: &str) -> &str {
    target.split(['?', '#']).next().unwrap_or(target)
}

/// Whether a request targets the cookbook RUN route
/// (`POST /api/cookbook/recipe/<id>/run`). This is the only cookbook route that
/// evaluates a recipe, so it is the only one the shell grants read-eval for;
/// list/search/show routes stay ungated. Mirrors the run-route match in
/// `sim-lib-server`'s `CookbookWebState::handle_request`.
/// The first value of a query-string key in a request target, if present. Only a
/// plain `key=value` split is performed; values are expected to be simple
/// identifiers (pane and resource names).
fn query_value(target: &str, key: &str) -> Option<String> {
    let (_, query) = target.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        (name == key).then(|| value.to_owned())
    })
}

/// Write a JSON body with the given status.
fn write_json(stream: &mut (impl Write + ?Sized), status: u16, body: &str) -> std::io::Result<()> {
    write_response(
        stream,
        status,
        status_text(status),
        "application/json; charset=utf-8",
        body.as_bytes(),
    )
}

fn write_cookbook_response(
    stream: &mut (impl Write + ?Sized),
    response: &CookbookWebResponse,
) -> std::io::Result<()> {
    write_response(
        stream,
        response.status,
        status_text(response.status),
        response.content_type,
        response.body.as_bytes(),
    )
}

fn write_response(
    stream: &mut (impl Write + ?Sized),
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)?;
    stream.flush()
}

fn status_text(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        429 => "Too Many Requests",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        // Fall back to the reason phrase for the status class rather than
        // mislabeling every unlisted code as "OK".
        other => match other / 100 {
            1 => "Informational",
            2 => "OK",
            3 => "Redirection",
            4 => "Client Error",
            _ => "Internal Server Error",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_BODY_BYTES, MAX_HEAD_LINE_BYTES, MAX_HEADER_COUNT, ReadOutcome, read_request_from,
    };
    use std::io::{BufReader, Cursor};

    fn parse(raw: &str) -> ReadOutcome {
        let mut reader = BufReader::new(Cursor::new(raw.as_bytes().to_vec()));
        read_request_from(&mut reader).expect("read")
    }

    #[test]
    fn oversized_content_length_is_rejected_before_allocation() {
        // A 4 GB declared body must be refused with 413, never allocated.
        let raw = "POST /api/session/intent HTTP/1.1\r\nContent-Length: 4000000000\r\n\r\n";
        assert!(
            matches!(parse(raw), ReadOutcome::TooLarge),
            "an oversized Content-Length must yield TooLarge (413)"
        );
    }

    #[test]
    fn content_length_at_the_cap_boundary_is_rejected_when_over() {
        let over = MAX_BODY_BYTES + 1;
        let raw = format!("POST /x HTTP/1.1\r\nContent-Length: {over}\r\n\r\n");
        assert!(matches!(parse(&raw), ReadOutcome::TooLarge));
    }

    #[test]
    fn an_oversized_request_line_is_rejected_before_growing_memory() {
        // A request line past the head cap must be refused with 413, not read
        // into an unbounded String.
        let mut raw = String::from("GET /");
        raw.push_str(&"a".repeat(MAX_HEAD_LINE_BYTES + 16));
        raw.push_str(" HTTP/1.1\r\n\r\n");
        assert!(
            matches!(parse(&raw), ReadOutcome::TooLarge),
            "an oversized request line must yield TooLarge (413)"
        );
    }

    #[test]
    fn an_oversized_header_line_is_rejected_before_growing_memory() {
        let mut raw = String::from("GET /x HTTP/1.1\r\nX-Big: ");
        raw.push_str(&"a".repeat(MAX_HEAD_LINE_BYTES + 16));
        raw.push_str("\r\n\r\n");
        assert!(
            matches!(parse(&raw), ReadOutcome::TooLarge),
            "an oversized header line must yield TooLarge (413)"
        );
    }

    #[test]
    fn too_many_header_lines_are_rejected() {
        let mut raw = String::from("GET /x HTTP/1.1\r\n");
        for _ in 0..(MAX_HEADER_COUNT + 8) {
            raw.push_str("X-Pad: 1\r\n");
        }
        raw.push_str("\r\n");
        assert!(
            matches!(parse(&raw), ReadOutcome::TooLarge),
            "an endless header block must yield TooLarge (413)"
        );
    }

    #[test]
    fn a_small_body_within_the_cap_reads() {
        let raw = "POST /x HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello";
        match parse(raw) {
            ReadOutcome::Request(line) => {
                assert_eq!(line.method, "POST");
                assert_eq!(line.body, "hello");
            }
            other => panic!("expected a parsed request, got {other:?}"),
        }
    }
}
