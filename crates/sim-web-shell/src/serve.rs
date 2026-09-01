//! Minimal blocking HTTP/1.1 server for the Web shell.
//!
//! The server serves embedded assets, the cookbook API adapter, and the Atelier
//! shell cache API. Runtime transport remains the Intent/Scene bridge over
//! `realize`/`EvalFabric`.

use std::fmt;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::sync::Arc;
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
    DEFAULT_PANE, DEFAULT_RESOURCE, DefaultLiveSurfaceFactory, LiveSessionTable,
    LiveSurfaceFactory, decode_intent_body, encode_patches, encode_scene, error_json,
};
use sim_kernel::Cx;
use sim_lib_net_core::{CapOutcome, read_capped_line};
use sim_lib_server::{CookbookWebResponse, CookbookWebState};
use sim_transport_ports::{SocketAddress, Stream, TransportError};

/// Configuration for the shell server.
pub struct ServeConfig {
    /// The address to bind, e.g. `127.0.0.1:8787`.
    pub addr: String,
    /// Directory containing generated Atelier cache files.
    pub atelier_root: PathBuf,
    /// Return before binding the socket. Lets a caller confirm the serve verb
    /// dispatches without holding a port.
    pub dry_run: bool,
    /// Host-provided cookbook state. When absent, the standalone shell uses the
    /// small fixture directory from `sim-lib-cookbook`.
    pub cookbook: Option<Arc<CookbookWebState>>,
}

impl fmt::Debug for ServeConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServeConfig")
            .field("addr", &self.addr)
            .field("atelier_root", &self.atelier_root)
            .field("dry_run", &self.dry_run)
            .field("cookbook", &self.cookbook.as_ref().map(|_| "<provided>"))
            .finish()
    }
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:8787".to_owned(),
            atelier_root: PathBuf::from(".sim/atelier"),
            dry_run: false,
            cookbook: None,
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
/// on it.
pub fn serve_with_cx(
    cx: &mut Cx,
    config: &ServeConfig,
    services: Arc<dyn crate::ShellServices>,
) -> std::io::Result<()> {
    serve_with_surface_factory(
        cx,
        config,
        Box::new(DefaultLiveSurfaceFactory::new(sim_kernel::HandleSeed::new(
            0x5745_4253,
        ))),
        services,
    )
}

/// Bind and serve the shell with a caller-provided browser surface factory.
///
/// Domain products use this composition point to supply their own
/// `SurfaceCodec`, transport, resource, and diminished authority while retaining
/// the shell's HTTP lifecycle, opaque browser-session table, and one generic
/// Scene interpreter.
pub fn serve_with_surface_factory(
    cx: &mut Cx,
    config: &ServeConfig,
    surface_factory: Box<dyn LiveSurfaceFactory + Send + Sync>,
    services: Arc<dyn crate::ShellServices>,
) -> std::io::Result<()> {
    if config.dry_run {
        println!("sim-web-shell: dry-run OK");
        return Ok(());
    }

    let listener = bind(&config.addr, &services.transport()).map_err(io_error)?;
    let local = listener.local_address().map_err(io_error)?;
    let mut state =
        ShellState::with_surface_factory(config, cx, surface_factory, Arc::clone(&services))?;
    println!(
        "sim-web-shell: serving shell on http://{}",
        display_address(&local)
    );
    loop {
        match listener.accept() {
            Ok(Some(stream)) => {
                if let Err(err) = handle(stream, &mut state) {
                    eprintln!("sim-web-shell: connection error: {err}");
                }
            }
            Ok(None) => continue,
            Err(err) => eprintln!("sim-web-shell: accept error: {err}"),
        }
    }
}

fn bind(
    addr: &str,
    transport: &sim_transport_ports::TransportServices,
) -> Result<Box<dyn sim_transport_ports::Listener>, TransportError> {
    let (host, port) = addr.rsplit_once(':').ok_or_else(|| {
        TransportError::new(
            sim_transport_ports::TransportErrorKind::InvalidAddress,
            "address must be HOST:PORT",
        )
    })?;
    let port = port.parse::<u16>().map_err(|_| {
        TransportError::new(
            sim_transport_ports::TransportErrorKind::InvalidAddress,
            "invalid port",
        )
    })?;
    let address = match host.parse() {
        Ok(address) => SocketAddress::Ip { address, port },
        Err(_) => transport
            .dns
            .resolve(host, port)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                TransportError::new(
                    sim_transport_ports::TransportErrorKind::DnsFailure,
                    "host resolved to no addresses",
                )
            })?,
    };
    transport.sockets.listen_tcp(&address)
}

fn display_address(address: &SocketAddress) -> String {
    match address {
        SocketAddress::Ip { address, port } => format!("{address}:{port}"),
    }
}

struct ShellState<'a> {
    atelier: AtelierWebState,
    cookbook: Arc<CookbookWebState>,
    cookbook_cx: &'a mut Cx,
    live: LiveSessionTable,
}

impl<'a> ShellState<'a> {
    #[cfg(test)]
    fn new(config: &ServeConfig, cx: &'a mut Cx) -> std::io::Result<Self> {
        let model = Arc::new(sim_transport_ports::model::ScriptedStreamPort::new([]));
        let services = Arc::new(crate::ModelShellServices::new(
            model.services(),
            Default::default(),
        ));
        Self::with_surface_factory(
            config,
            cx,
            Box::new(DefaultLiveSurfaceFactory::new(sim_kernel::HandleSeed::new(
                0x5745_4254,
            ))),
            services,
        )
    }

    fn with_surface_factory(
        config: &ServeConfig,
        cx: &'a mut Cx,
        surface_factory: Box<dyn LiveSurfaceFactory + Send + Sync>,
        services: Arc<dyn crate::ShellServices>,
    ) -> std::io::Result<Self> {
        // The cookbook eval sandbox is the bootloader-provided `cx`, which already
        // carries the standard distribution the recipes require and read-eval,
        // granted by the bootloader at the web-serve composition point. run_recipe
        // gates each run on read-eval, so a session that never runs a recipe never
        // uses it.
        Ok(Self {
            atelier: AtelierWebState::load(config.atelier_root.clone(), |path| {
                services.read_mount(path)
            }),
            cookbook: match &config.cookbook {
                Some(cookbook) => Arc::clone(cookbook),
                None => Arc::new(CookbookWebState::seeded().map_err(io_error)?),
            },
            cookbook_cx: cx,
            live: LiveSessionTable::with_config_and_services(
                surface_factory,
                Default::default(),
                services,
            ),
        })
    }
}

#[cfg(test)]
pub(crate) fn cookbook_index_for_test(
    cx: &mut Cx,
    config: &ServeConfig,
) -> std::io::Result<CookbookWebResponse> {
    let state = ShellState::new(config, cx)?;
    Ok(state
        .cookbook
        .handle_request("GET", "/api/cookbook", Some(&mut *state.cookbook_cx)))
}

fn io_error(err: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(err.to_string())
}

fn handle(mut stream: Box<dyn Stream>, state: &mut ShellState<'_>) -> std::io::Result<()> {
    // Bound how long a single read may block; a slow-loris peer cannot pin the
    // server. A failure to set the timeout is non-fatal (e.g. exotic streams).
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let request = match read_request(&mut *stream)? {
        ReadOutcome::Request(request) => request,
        ReadOutcome::TooLarge => {
            write_response(
                &mut *stream,
                413,
                "Payload Too Large",
                "text/plain; charset=utf-8",
                b"payload too large",
            )?;
            return Ok(());
        }
        ReadOutcome::Invalid => {
            write_response(
                &mut *stream,
                400,
                "Bad Request",
                "text/plain; charset=utf-8",
                b"bad request",
            )?;
            return Ok(());
        }
    };
    if path_of(&request.target) == "/api/session/intent" {
        return write_session_intent(&mut *stream, &request, &mut state.live);
    }
    if path_of(&request.target) == "/api/session/open" {
        return write_session_open(&mut *stream, &request, &mut state.live);
    }
    if path_of(&request.target) == "/api/session/close" {
        return write_session_close(&mut *stream, &request, &mut state.live);
    }
    if request.target.starts_with("/api/cookbook") {
        // read-eval was granted to cookbook_cx by the bootloader (see cli.rs);
        // run_recipe gates each run on it.
        let response = state.cookbook.handle_request(
            &request.method,
            &request.target,
            Some(&mut *state.cookbook_cx),
        );
        return write_cookbook_response(&mut *stream, &response);
    }
    if let Some(response) = state.atelier.response(&request.method, &request.target) {
        return write_response(
            &mut *stream,
            response.status,
            status_text(response.status),
            response.content_type,
            response.body.as_bytes(),
        );
    }
    if request.method != "GET" {
        write_response(
            &mut *stream,
            405,
            "Method Not Allowed",
            "text/plain; charset=utf-8",
            b"method not allowed",
        )?;
        return Ok(());
    }
    match asset_for(&request.target) {
        Some(asset) => write_response(&mut *stream, 200, "OK", asset.content_type, asset.body),
        None => write_response(
            &mut *stream,
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
fn read_request(stream: &mut dyn Stream) -> std::io::Result<ReadOutcome> {
    let mut reader = BufReader::new(stream);
    read_request_from(&mut reader)
}

/// Parse a request from any buffered reader, bounding the body at
/// [`MAX_BODY_BYTES`]. A declared `Content-Length` over the cap returns
/// [`ReadOutcome::TooLarge`] before any allocation, and the body read is capped
/// at the same limit so a lying header cannot over-read.
fn read_request_from(reader: &mut impl BufRead) -> std::io::Result<ReadOutcome> {
    let mut request_line = String::new();
    match read_capped_line(reader, &mut request_line, MAX_HEAD_LINE_BYTES)? {
        // An oversized request line is refused with 413 before it can grow memory.
        CapOutcome::TooLarge => return Ok(ReadOutcome::TooLarge),
        CapOutcome::Eof => return Ok(ReadOutcome::Invalid),
        CapOutcome::Line => {}
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
        match read_capped_line(reader, &mut header, MAX_HEAD_LINE_BYTES)? {
            CapOutcome::TooLarge => return Ok(ReadOutcome::TooLarge),
            CapOutcome::Eof => break,
            CapOutcome::Line => {}
        }
        if header == "\r\n" || header == "\n" {
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
    live: &mut LiveSessionTable,
) -> std::io::Result<()> {
    if request.method != "POST" {
        return write_json(stream, 405, &error_json("intent route requires POST"));
    }
    let session_id = match query_value(&request.target, "session") {
        Ok(Some(value)) => value,
        Ok(None) => return write_json(stream, 400, &error_json("missing session id")),
        Err(err) => return write_json(stream, 400, &error_json(&err.to_string())),
    };
    let pane = match query_value(&request.target, "pane") {
        Ok(Some(value)) => value,
        Ok(None) => DEFAULT_PANE.to_owned(),
        Err(err) => return write_json(stream, 400, &error_json(&err.to_string())),
    };
    let intent = match decode_intent_body(&request.body) {
        Ok(intent) => intent,
        Err(err) => return write_json(stream, 400, &error_json(&err)),
    };
    match live.submit(&session_id, &pane, &intent) {
        Ok(updates) => write_json(stream, 200, &encode_patches(&updates)),
        Err(err) => write_json(stream, 400, &error_json(&err.to_string())),
    }
}

/// Handle `GET /api/session/open?resource=...&pane=...`: open the resource into
/// the pane and respond with its initial Scene.
fn write_session_open(
    stream: &mut (impl Write + ?Sized),
    request: &RequestLine,
    live: &mut LiveSessionTable,
) -> std::io::Result<()> {
    if request.method != "GET" {
        return write_json(stream, 405, &error_json("open route requires GET"));
    }
    let session_id = match query_value(&request.target, "session") {
        Ok(value) => value,
        Err(err) => return write_json(stream, 400, &error_json(&err.to_string())),
    };
    let resource = match query_value(&request.target, "resource") {
        Ok(Some(value)) => value,
        Ok(None) => DEFAULT_RESOURCE.to_owned(),
        Err(err) => return write_json(stream, 400, &error_json(&err.to_string())),
    };
    let pane = match query_value(&request.target, "pane") {
        Ok(Some(value)) => value,
        Ok(None) => DEFAULT_PANE.to_owned(),
        Err(err) => return write_json(stream, 400, &error_json(&err.to_string())),
    };
    match live.open(session_id.as_deref(), &resource, &pane) {
        Ok((session_id, scene)) => {
            write_json(stream, 200, &encode_session_open(&session_id, &scene))
        }
        Err(err) => write_json(stream, 400, &error_json(&err.to_string())),
    }
}

/// Handle `POST /api/session/close?session=...`: cancel and remove a browser
/// session so its authority and connection state cannot be reused.
fn write_session_close(
    stream: &mut (impl Write + ?Sized),
    request: &RequestLine,
    live: &mut LiveSessionTable,
) -> std::io::Result<()> {
    if request.method != "POST" {
        return write_json(stream, 405, &error_json("close route requires POST"));
    }
    let session_id = match query_value(&request.target, "session") {
        Ok(Some(value)) => value,
        Ok(None) => return write_json(stream, 400, &error_json("missing session id")),
        Err(err) => return write_json(stream, 400, &error_json(&err.to_string())),
    };
    match live.close(&session_id) {
        Ok(()) => write_json(stream, 200, r#"{"ok":true}"#),
        Err(err) => write_json(stream, 400, &error_json(&err)),
    }
}

fn encode_session_open(session_id: &str, scene: &sim_kernel::Expr) -> String {
    let mut value: serde_json::Value =
        serde_json::from_str(&encode_scene(scene)).expect("encode_scene emits JSON object");
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "session".to_owned(),
            serde_json::Value::String(session_id.to_owned()),
        );
    }
    value.to_string()
}

/// The path portion of a request target, with any query or fragment stripped.
fn path_of(target: &str) -> &str {
    target.split(['?', '#']).next().unwrap_or(target)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueryError {
    key: String,
    reason: String,
}

impl QueryError {
    fn new(key: &str, reason: impl Into<String>) -> Self {
        Self {
            key: key.to_owned(),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "malformed query value for '{}': {}",
            self.key, self.reason
        )
    }
}

/// Whether a request targets the cookbook RUN route
/// (`POST /api/cookbook/recipe/<id>/run`). This is the only cookbook route that
/// evaluates a recipe, so it is the only one the shell grants read-eval for;
/// list/search/show routes stay ungated. Mirrors the run-route match in
/// `sim-lib-server`'s `CookbookWebState::handle_request`.
/// The first value of a query-string key in a request target, if present. Only a
/// value for the matching key is percent-decoded; malformed escapes are errors
/// so the session routes can reject them with a structured 400 response.
fn query_value(target: &str, key: &str) -> Result<Option<String>, QueryError> {
    let Some((_, query_and_fragment)) = target.split_once('?') else {
        return Ok(None);
    };
    let query = query_and_fragment
        .split('#')
        .next()
        .unwrap_or(query_and_fragment);
    for pair in query.split('&') {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        if name == key {
            return percent_decode_query(value)
                .map(Some)
                .map_err(|reason| QueryError::new(key, reason));
        }
    }
    Ok(None)
}

fn percent_decode_query(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len() {
                    return Err("incomplete percent escape".to_owned());
                }
                let high = hex_digit(bytes[index + 1])
                    .ok_or_else(|| "invalid percent escape".to_owned())?;
                let low = hex_digit(bytes[index + 2])
                    .ok_or_else(|| "invalid percent escape".to_owned())?;
                decoded.push((high << 4) | low);
                index += 3;
            }
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8(decoded).map_err(|_| "decoded value is not UTF-8".to_owned())
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
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
#[path = "serve_tests.rs"]
mod tests;
