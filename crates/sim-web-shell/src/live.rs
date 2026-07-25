//! The live browser session bridge (VIEW4.05).
//!
//! This module turns the embedded browser shell into a live edit surface over
//! the blocking HTTP server: the browser posts an Intent, the server submits it
//! through a server-held [`Session`], pumps the resulting Scene diff, and
//! responds with the patch(es). The browser applies each patch and repaints. It
//! is a submit/response bridge -- each Intent is one request -- not a streaming
//! channel.
//!
//! # Wire format
//!
//! The browser already speaks plain, untagged JSON: `intent.js` builds untagged
//! Intent objects and `diff.js`/`scene.js` consume untagged Scene patches and
//! Scenes. So the bridge uses `sim-codec-json`'s untagged interop projection in
//! both directions. The cookbook route hand-rolls its JSON and never decodes an
//! `Expr` from a request body, so there was no existing body codec to reuse;
//! this is the bridge's own decode/encode surface.
//!
//! The untagged projection is intentionally lossy (it cannot tell a symbol from
//! a string), so [`decode_intent_body`] lifts the well-known Intent envelope
//! back to faithful `Expr`s: the `kind` tag and `origin.operator` become
//! symbols, and each `path` segment tag (`k`/`i`) becomes a symbol so the
//! universal editor's path parser accepts it. Every other field passes through
//! as decoded. The universal default editor edits at the root path, which is the
//! only shape the shipped browser shell emits, so this lift is sufficient for
//! the live surface today.
//!
//! # Future work
//!
//! This is a request/response bridge. A WebSocket (or SSE) channel would let the
//! server push patches without a client Intent -- needed for agent peers and
//! collaborative edits -- but that requires an async server and is out of scope
//! for the blocking HTTP shell. When that lands, the same [`Session::pump`]
//! output should be streamed rather than returned per request.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::sync::Arc;
use std::time::{Duration, Instant};

use sim_codec_json::{JsonProjectionMode, project_expr_to_json, project_json_to_expr};
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Result as SimResult, Symbol};
use sim_lib_view::{LensRegistry, UNIVERSAL_SURFACE_CODEC_ID, register_universal_default, surface};
use sim_lib_web_bridge::{FixtureTransport, SceneUpdate, Session};

/// The namespace every Intent `kind` symbol lives in (mirrors `sim-lib-intent`).
const INTENT_NAMESPACE: &str = "intent";

/// The default pane the shell opens the demo resource into. The shipped
/// `app.js` posts Intents for this pane.
pub const DEFAULT_PANE: &str = "pane-main";

/// The default resource seeded into the live session for the demo shell.
pub const DEFAULT_RESOURCE: &str = "demo";

/// A server-held live session: a [`Session`] over a deterministic in-memory
/// [`FixtureTransport`], its [`LensRegistry`] (with the universal default lens
/// registered), and the runtime [`Cx`] used to render Scenes.
///
/// The blocking HTTP server is single-threaded, so the shell owns one of these
/// directly and serves every request against it in turn; no lock is needed. A
/// multi-threaded server would hold this behind a `Mutex`.
pub struct LiveSession {
    session: Session<FixtureTransport>,
    registry: LensRegistry,
    cx: Cx,
}

impl LiveSession {
    /// Build a live session, seed the demo resource, and open it into the
    /// default pane so Intents can be submitted immediately.
    pub fn new() -> SimResult<Self> {
        let mut transport = FixtureTransport::new();
        transport.set(Symbol::new(DEFAULT_RESOURCE), demo_value());
        let mut registry = LensRegistry::new();
        register_universal_default(&mut registry, false);
        // bin-boot-exempt: the LiveSession is the realize/EvalFabric Intent/Scene
        // bridge -- a distinct eval surface with its own transport, not the binary's
        // boot runtime (that goes through sim_run_core::Bootloader). It owns its cx.
        let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory)); // bin-boot-exempt
        let mut session = Session::new(transport);
        session.open_codec(
            &mut cx,
            &registry,
            Symbol::new(DEFAULT_PANE),
            Symbol::new(DEFAULT_RESOURCE),
            Symbol::new(UNIVERSAL_SURFACE_CODEC_ID),
            surface::preset("webui").expect("webui is a known surface preset"),
        )?;
        Ok(Self {
            session,
            registry,
            cx,
        })
    }

    /// Open `resource` into `pane` through the universal default lenses and
    /// return its initial Scene.
    pub fn open(&mut self, resource: &str, pane: &str) -> SimResult<Expr> {
        self.session.open_codec(
            &mut self.cx,
            &self.registry,
            Symbol::new(pane),
            Symbol::new(resource),
            Symbol::new(UNIVERSAL_SURFACE_CODEC_ID),
            surface::preset("webui").expect("webui is a known surface preset"),
        )
    }

    /// Submit a decoded Intent against `pane`, then pump and return the Scene
    /// update(s) (each carrying the diff that reconstructs its new Scene).
    pub fn submit(&mut self, pane: &str, intent: &Expr) -> SimResult<Vec<SceneUpdate>> {
        self.session
            .submit_intent(&mut self.cx, &self.registry, &Symbol::new(pane), intent)?;
        self.session.pump(&mut self.cx, &self.registry)
    }
}

/// Object-safe factory for browser-owned live surfaces.
pub trait LiveSurfaceFactory {
    fn create(&self) -> SimResult<LiveSession>;
}

/// Default shell surface factory.
#[derive(Debug, Default)]
pub struct DefaultLiveSurfaceFactory;

impl LiveSurfaceFactory for DefaultLiveSurfaceFactory {
    fn create(&self) -> SimResult<LiveSession> {
        LiveSession::new()
    }
}

/// Capacity and expiry policy for live browser sessions.
#[derive(Debug, Clone)]
pub struct LiveSessionTableConfig {
    pub capacity: usize,
    pub idle_ttl: Duration,
}

impl Default for LiveSessionTableConfig {
    fn default() -> Self {
        Self {
            capacity: 64,
            idle_ttl: Duration::from_secs(30 * 60),
        }
    }
}

struct LiveSessionEntry {
    live: LiveSession,
    last_used: Instant,
    ordinal: u64,
}

/// Bounded table of isolated live browser sessions.
pub struct LiveSessionTable {
    factory: Box<dyn LiveSurfaceFactory + Send + Sync>,
    config: LiveSessionTableConfig,
    sessions: BTreeMap<String, LiveSessionEntry>,
    next_ordinal: u64,
}

impl LiveSessionTable {
    pub fn new(factory: Box<dyn LiveSurfaceFactory + Send + Sync>) -> Self {
        Self::with_config(factory, LiveSessionTableConfig::default())
    }

    pub fn with_config(
        factory: Box<dyn LiveSurfaceFactory + Send + Sync>,
        config: LiveSessionTableConfig,
    ) -> Self {
        Self {
            factory,
            config,
            sessions: BTreeMap::new(),
            next_ordinal: 0,
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn open(
        &mut self,
        session_id: Option<&str>,
        resource: &str,
        pane: &str,
    ) -> Result<(String, Expr), String> {
        self.open_at(session_id, resource, pane, Instant::now())
    }

    pub fn submit(
        &mut self,
        session_id: &str,
        pane: &str,
        intent: &Expr,
    ) -> Result<Vec<SceneUpdate>, String> {
        self.submit_at(session_id, pane, intent, Instant::now())
    }

    pub fn close(&mut self, session_id: &str) -> Result<(), String> {
        validate_session_id(session_id)?;
        if self.sessions.remove(session_id).is_some() {
            Ok(())
        } else {
            Err("unknown session id".to_owned())
        }
    }

    pub fn open_at(
        &mut self,
        session_id: Option<&str>,
        resource: &str,
        pane: &str,
        now: Instant,
    ) -> Result<(String, Expr), String> {
        self.evict_idle(now);
        if let Some(session_id) = session_id {
            let entry = self.entry_mut(session_id, now)?;
            let scene = entry
                .live
                .open(resource, pane)
                .map_err(|err| err.to_string())?;
            return Ok((session_id.to_owned(), scene));
        }
        self.evict_for_capacity();
        if self.config.capacity == 0 || self.sessions.len() >= self.config.capacity {
            return Err("session capacity exhausted".to_owned());
        }
        let session_id = self.fresh_unused_session_id()?;
        let mut live = self.factory.create().map_err(|err| err.to_string())?;
        let scene = live.open(resource, pane).map_err(|err| err.to_string())?;
        let ordinal = self.next_ordinal;
        self.next_ordinal = self.next_ordinal.saturating_add(1);
        self.sessions.insert(
            session_id.clone(),
            LiveSessionEntry {
                live,
                last_used: now,
                ordinal,
            },
        );
        Ok((session_id, scene))
    }

    pub fn submit_at(
        &mut self,
        session_id: &str,
        pane: &str,
        intent: &Expr,
        now: Instant,
    ) -> Result<Vec<SceneUpdate>, String> {
        self.evict_idle(now);
        let entry = self.entry_mut(session_id, now)?;
        entry
            .live
            .submit(pane, intent)
            .map_err(|err| err.to_string())
    }

    fn entry_mut(
        &mut self,
        session_id: &str,
        now: Instant,
    ) -> Result<&mut LiveSessionEntry, String> {
        validate_session_id(session_id)?;
        let entry = self
            .sessions
            .get_mut(session_id)
            .ok_or_else(|| "unknown session id".to_owned())?;
        entry.last_used = now;
        Ok(entry)
    }

    fn evict_idle(&mut self, now: Instant) {
        let ttl = self.config.idle_ttl;
        self.sessions
            .retain(|_, entry| now.duration_since(entry.last_used) <= ttl);
    }

    fn evict_for_capacity(&mut self) {
        while self.config.capacity > 0 && self.sessions.len() >= self.config.capacity {
            let Some(victim) = self
                .sessions
                .iter()
                .min_by_key(|(_, entry)| (entry.last_used, entry.ordinal))
                .map(|(id, _)| id.clone())
            else {
                return;
            };
            self.sessions.remove(&victim);
        }
    }

    fn fresh_unused_session_id(&self) -> Result<String, String> {
        for _ in 0..8 {
            let session_id = fresh_session_id()?;
            if !self.sessions.contains_key(&session_id) {
                return Ok(session_id);
            }
        }
        Err("could not allocate unique session id".to_owned())
    }
}

fn validate_session_id(session_id: &str) -> Result<(), String> {
    let valid = session_id.len() == 32 && session_id.bytes().all(|b| b.is_ascii_hexdigit());
    if valid {
        Ok(())
    } else {
        Err("malformed session id".to_owned())
    }
}

fn fresh_session_id() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    File::open("/dev/urandom")
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|err| format!("could not allocate session id: {err}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The demo resource value rendered by the live shell on boot.
fn demo_value() -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("title")),
            Expr::String("SIM live session".to_owned()),
        ),
        (
            Expr::Symbol(Symbol::new("note")),
            Expr::String("edit me".to_owned()),
        ),
    ])
}

/// Decode an Intent from an untagged-JSON request body and lift its envelope
/// back to faithful `Expr`s. Returns a structured error string on malformed
/// JSON or a non-object body; it never panics.
pub fn decode_intent_body(body: &str) -> Result<Expr, String> {
    let value: serde_json::Value =
        serde_json::from_str(body).map_err(|err| format!("invalid JSON intent body: {err}"))?;
    let expr = project_json_to_expr(&value, JsonProjectionMode::UntaggedInterop);
    lift_intent(expr)
}

/// Encode a batch of Scene updates as the untagged-JSON `{ "patches": [...] }`
/// response the browser's patch listener consumes.
pub fn encode_patches(updates: &[SceneUpdate]) -> String {
    let patches: Vec<serde_json::Value> = updates
        .iter()
        .map(|update| project_expr_to_json(&update.diff, JsonProjectionMode::UntaggedInterop))
        .collect();
    serde_json::json!({ "patches": patches }).to_string()
}

/// Encode a Scene as the untagged-JSON `{ "scene": ... }` response the open
/// route returns.
pub fn encode_scene(scene: &Expr) -> String {
    serde_json::json!({ "scene": project_expr_to_json(scene, JsonProjectionMode::UntaggedInterop) })
        .to_string()
}

/// Encode a structured `{ "error": message }` JSON body.
pub fn error_json(message: &str) -> String {
    serde_json::json!({ "error": message }).to_string()
}

/// Lift an untagged-decoded Intent map back to a faithful Intent `Expr`.
fn lift_intent(expr: Expr) -> Result<Expr, String> {
    let Expr::Map(entries) = expr else {
        return Err("intent body must be a JSON object".to_owned());
    };
    let mut lifted = Vec::with_capacity(entries.len());
    for (key, value) in entries {
        let name = key_name(&key)?;
        let value = match name.as_str() {
            "kind" => lift_kind(value)?,
            "origin" => lift_origin(value),
            "path" => lift_path(value),
            _ => value,
        };
        lifted.push((Expr::Symbol(Symbol::new(name)), value));
    }
    Ok(Expr::Map(lifted))
}

/// The local name of a map key (a symbol or string key).
fn key_name(key: &Expr) -> Result<String, String> {
    match key {
        Expr::Symbol(symbol) => Ok(symbol.name.to_string()),
        Expr::String(text) => Ok(text.clone()),
        other => Err(format!("intent key must be a string, found {other:?}")),
    }
}

/// Lift a `kind` field to its `intent/<name>` symbol, stripping a redundant
/// `intent/` prefix the browser may include.
fn lift_kind(value: Expr) -> Result<Expr, String> {
    match value {
        Expr::Symbol(symbol) => Ok(Expr::Symbol(symbol)),
        Expr::String(text) => {
            let local = text.strip_prefix("intent/").unwrap_or(&text);
            Ok(Expr::Symbol(Symbol::qualified(INTENT_NAMESPACE, local)))
        }
        other => Err(format!("intent 'kind' must be a string, found {other:?}")),
    }
}

/// Lift the `origin.operator` field to a symbol, leaving the tick untouched.
fn lift_origin(value: Expr) -> Expr {
    let Expr::Map(entries) = value else {
        return value;
    };
    let lifted = entries
        .into_iter()
        .map(|(key, value)| {
            let is_operator = matches!(&key, Expr::Symbol(symbol) if &*symbol.name == "operator")
                || matches!(&key, Expr::String(text) if text == "operator");
            let value = match value {
                Expr::String(text) if is_operator => Expr::Symbol(Symbol::new(text)),
                other => other,
            };
            (key, value)
        })
        .collect();
    Expr::Map(lifted)
}

/// Lift each `path` segment to the `Vector([sym(tag), key])` wire form the
/// universal editor's path parser expects. The segment tag (`k`/`i`) becomes a
/// symbol; the key passes through. An empty path (the only shape the shipped
/// shell emits) round-trips unchanged.
fn lift_path(value: Expr) -> Expr {
    let segments = match value {
        Expr::List(segments) | Expr::Vector(segments) => segments,
        other => return other,
    };
    Expr::List(segments.into_iter().map(lift_segment).collect())
}

/// Lift a single path segment `[tag, key]` to `Vector([sym(tag), key])`.
fn lift_segment(segment: Expr) -> Expr {
    let items = match segment {
        Expr::List(items) | Expr::Vector(items) => items,
        other => return other,
    };
    let lifted = items
        .into_iter()
        .enumerate()
        .map(|(index, item)| match item {
            Expr::String(text) if index == 0 => Expr::Symbol(Symbol::new(text)),
            other => other,
        })
        .collect();
    Expr::Vector(lifted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_lib_intent::{Origin, intent};

    fn key_path(key: &str) -> Expr {
        Expr::List(vec![Expr::Vector(vec![
            Expr::Symbol(Symbol::new("k")),
            Expr::Symbol(Symbol::new(key)),
        ])])
    }

    fn edit_intent(key: &str, value: &str) -> Expr {
        intent(
            "edit-field",
            Origin::human(1),
            vec![
                ("target", demo_value()),
                ("path", key_path(key)),
                ("value", Expr::String(value.to_owned())),
            ],
        )
    }

    #[test]
    fn submit_edit_returns_a_patch_that_reconstructs_the_scene() {
        let mut live = LiveSession::new().unwrap();
        let before = live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        sim_lib_scene::validate_scene(&before).expect("initial scene is valid");

        let updates = live
            .submit(DEFAULT_PANE, &edit_intent("title", "changed"))
            .unwrap();
        assert_eq!(updates.len(), 1, "the subscribed pane updates exactly once");
        let update = &updates[0];
        assert_ne!(update.scene, before, "the Scene changed");
        let rebuilt = sim_lib_scene::apply(&before, &update.diff).unwrap();
        assert_eq!(
            rebuilt, update.scene,
            "the diff reconstructs the new Scene from the old one"
        );
    }

    #[test]
    fn open_returns_a_valid_scene() {
        let mut live = LiveSession::new().unwrap();
        let scene = live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        sim_lib_scene::validate_scene(&scene).expect("open returns a valid Scene");
    }

    #[test]
    fn a_browser_json_intent_decodes_and_drives_a_root_edit() {
        // The browser posts untagged JSON with a string `kind` and a root path;
        // the bridge must lift it into an Intent the universal editor accepts.
        let body = r#"{"kind":"intent/edit-field","origin":{"operator":"human","at-tick":2},"target":{},"path":[],"value":"hello"}"#;
        let intent = decode_intent_body(body).unwrap();
        let kind = match &intent {
            Expr::Map(entries) => entries.iter().find_map(|(key, value)| {
                matches!(key, Expr::Symbol(symbol) if &*symbol.name == "kind").then_some(value)
            }),
            _ => None,
        };
        assert!(
            matches!(kind, Some(Expr::Symbol(_))),
            "the kind tag is lifted to a symbol"
        );

        let mut live = LiveSession::new().unwrap();
        live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        let updates = live.submit(DEFAULT_PANE, &intent).unwrap();
        assert_eq!(updates.len(), 1);
    }

    #[test]
    fn a_malformed_body_is_an_error_not_a_panic() {
        assert!(decode_intent_body("this is not json").is_err());
        assert!(
            decode_intent_body("[1, 2, 3]").is_err(),
            "a non-object intent body is rejected"
        );
    }

    #[test]
    fn an_intent_without_a_kind_fails_closed_on_submit() {
        let intent = decode_intent_body(r#"{"origin":{"operator":"human","at-tick":1}}"#).unwrap();
        let mut live = LiveSession::new().unwrap();
        assert!(
            live.submit(DEFAULT_PANE, &intent).is_err(),
            "an intent without a kind is rejected, not executed"
        );
    }

    #[test]
    fn patches_scenes_and_errors_encode_as_untagged_json() {
        let mut live = LiveSession::new().unwrap();
        live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        let updates = live
            .submit(DEFAULT_PANE, &edit_intent("title", "x"))
            .unwrap();

        let patches = encode_patches(&updates);
        assert!(patches.contains("\"patches\""), "carries a patches array");
        assert!(patches.contains("scene/patch"), "patches are scene patches");

        let scene = encode_scene(&live.open(DEFAULT_RESOURCE, DEFAULT_PANE).unwrap());
        assert!(scene.contains("\"scene\""), "carries a scene field");

        assert!(
            error_json("boom").contains("boom"),
            "errors carry a message"
        );
    }

    #[test]
    fn session_ids_are_opaque_and_validated() {
        let mut table = LiveSessionTable::with_config(
            Box::new(DefaultLiveSurfaceFactory),
            LiveSessionTableConfig {
                capacity: 2,
                idle_ttl: Duration::from_secs(60),
            },
        );
        let (session_id, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        assert_eq!(session_id.len(), 32);
        assert!(session_id.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(
            table
                .submit("not-a-session", DEFAULT_PANE, &edit_intent("title", "x"))
                .is_err(),
            "malformed ids fail closed"
        );
    }

    #[test]
    fn browser_sessions_are_isolated() {
        let mut table = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory));
        let (left, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        let (right, right_before) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        table
            .submit(&left, DEFAULT_PANE, &edit_intent("title", "left-only"))
            .unwrap();
        let (_, right_after) = table
            .open(Some(&right), DEFAULT_RESOURCE, DEFAULT_PANE)
            .unwrap();
        assert_eq!(
            right_after, right_before,
            "a write in one browser session must not alter another session"
        );
    }

    #[test]
    fn close_cancels_a_session() {
        let mut table = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory));
        let (session_id, _) = table.open(None, DEFAULT_RESOURCE, DEFAULT_PANE).unwrap();
        table.close(&session_id).unwrap();
        assert!(
            table
                .submit(&session_id, DEFAULT_PANE, &edit_intent("title", "closed"))
                .is_err(),
            "closed sessions cannot be reused"
        );
    }

    #[test]
    fn idle_sessions_expire() {
        let start = Instant::now();
        let mut table = LiveSessionTable::with_config(
            Box::new(DefaultLiveSurfaceFactory),
            LiveSessionTableConfig {
                capacity: 4,
                idle_ttl: Duration::from_secs(1),
            },
        );
        let (session_id, _) = table
            .open_at(None, DEFAULT_RESOURCE, DEFAULT_PANE, start)
            .unwrap();
        let later = start + Duration::from_secs(2);
        assert!(
            table
                .submit_at(
                    &session_id,
                    DEFAULT_PANE,
                    &edit_intent("title", "expired"),
                    later,
                )
                .is_err(),
            "idle sessions are evicted before use"
        );
        assert_eq!(table.len(), 0);
    }

    #[test]
    fn capacity_evicts_the_oldest_session_deterministically() {
        let start = Instant::now();
        let mut table = LiveSessionTable::with_config(
            Box::new(DefaultLiveSurfaceFactory),
            LiveSessionTableConfig {
                capacity: 2,
                idle_ttl: Duration::from_secs(60),
            },
        );
        let (first, _) = table
            .open_at(None, DEFAULT_RESOURCE, DEFAULT_PANE, start)
            .unwrap();
        let (second, _) = table
            .open_at(
                None,
                DEFAULT_RESOURCE,
                DEFAULT_PANE,
                start + Duration::from_secs(1),
            )
            .unwrap();
        table
            .submit_at(
                &second,
                DEFAULT_PANE,
                &edit_intent("title", "touch"),
                start + Duration::from_secs(2),
            )
            .unwrap();
        let (third, _) = table
            .open_at(
                None,
                DEFAULT_RESOURCE,
                DEFAULT_PANE,
                start + Duration::from_secs(3),
            )
            .unwrap();
        assert_ne!(third, first);
        assert_eq!(table.len(), 2);
        assert!(
            table
                .submit_at(
                    &first,
                    DEFAULT_PANE,
                    &edit_intent("title", "evicted"),
                    start + Duration::from_secs(4),
                )
                .is_err(),
            "the oldest idle session is evicted first"
        );
        table
            .submit_at(
                &second,
                DEFAULT_PANE,
                &edit_intent("title", "survives"),
                start + Duration::from_secs(5),
            )
            .unwrap();
    }
}
