//! Loadable CLI claims for the web shell surfaces.

use std::sync::Arc;

use sim_codec_lisp::LispCodecLib;
use sim_kernel::{
    AbiVersion, Args, CORE_FUNCTION_CLASS_ID, Callable, CapabilityName, ClassRef, CodecId, Cx,
    Error, Export, Expr, Lib, LibManifest, LibTarget, Linker, LoadCx, Object, ObjectCompat, Result,
    Symbol, Value, Version, read_eval_capability,
};
use sim_lib_server::{CookbookCapabilityProfile, CookbookWebState};
use sim_run_core::{Bootloader, RuntimeConfigState, cli_main_entrypoint_symbol};

use crate::serve::{ServeConfig, serve_with_cx};

/// Loadable lib that claims the `atelier` command-line verb.
pub struct AtelierCliLib;

/// Loadable lib that claims the `browse` command-line verb.
pub struct BrowseCliLib;

impl Lib for AtelierCliLib {
    fn manifest(&self) -> LibManifest {
        cli_manifest("atelier", "cli/main/atelier")
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        register_cli_entrypoint(cx, linker, "atelier")
    }
}

impl Lib for BrowseCliLib {
    fn manifest(&self) -> LibManifest {
        cli_manifest("browse", "cli/main/browse")
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        register_cli_entrypoint(cx, linker, "browse")
    }
}

fn cli_manifest(id: &str, entrypoint: &str) -> LibManifest {
    LibManifest {
        id: Symbol::new(id),
        version: Version(env!("CARGO_PKG_VERSION").to_owned()),
        abi: AbiVersion { major: 0, minor: 1 },
        target: LibTarget::HostRegistered,
        requires: Vec::new(),
        capabilities: Vec::new(),
        exports: vec![Export::Function {
            symbol: symbol_from_slash(entrypoint),
            function_id: None,
        }],
    }
}

fn register_cli_entrypoint(
    cx: &mut LoadCx,
    linker: &mut Linker<'_>,
    verb: &'static str,
) -> Result<()> {
    linker.function_value(
        Symbol::qualified("cli", format!("main/{verb}")),
        cx.factory()
            .opaque(Arc::new(WebShellCliEntrypoint { verb }))?,
    )?;
    Ok(())
}

#[derive(Clone)]
struct WebShellCliEntrypoint {
    verb: &'static str,
}

impl Object for WebShellCliEntrypoint {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok(format!("#<function cli/main/{}>", self.verb))
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for WebShellCliEntrypoint {
    fn class(&self, cx: &mut Cx) -> Result<ClassRef> {
        if let Some(value) = cx
            .registry()
            .class_by_symbol(&Symbol::qualified("core", "Function"))
        {
            return Ok(value.clone());
        }
        cx.factory().class_stub(
            CORE_FUNCTION_CLASS_ID,
            Symbol::qualified("core", "Function"),
        )
    }

    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for WebShellCliEntrypoint {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        verify_cli_envelope(cx, &args, self.verb)?;
        cx.factory().bool(true)
    }
}

fn verify_cli_envelope(cx: &mut Cx, args: &Args, verb: &str) -> Result<()> {
    let envelope = args
        .values()
        .first()
        .ok_or_else(|| Error::Eval(format!("cli/main/{verb} expects a CLI envelope")))?;
    let envelope_verb = envelope_string_field(cx, envelope, "verb")?;
    if envelope_verb != verb {
        return Err(Error::Eval(format!(
            "cli/main/{verb} received verb {envelope_verb}"
        )));
    }
    let payload_args = envelope_args(cx, envelope)?;
    if payload_args.first().map(String::as_str) != Some(verb) {
        return Err(Error::Eval(format!(
            "cli/main/{verb} expects the first payload argument to be {verb}"
        )));
    }
    Ok(())
}

fn envelope_string_field(cx: &mut Cx, envelope: &Value, field: &str) -> Result<String> {
    let Some(table) = envelope.object().as_table_impl() else {
        return Err(Error::Eval("CLI envelope is not a table".to_owned()));
    };
    match table.get(cx, Symbol::new(field))?.object().as_expr(cx)? {
        Expr::String(text) => Ok(text),
        Expr::Nil => Err(Error::Eval(format!("CLI envelope field {field} is nil"))),
        other => Err(Error::Eval(format!(
            "CLI envelope field {field} is not a string: {other:?}"
        ))),
    }
}

fn envelope_args(cx: &mut Cx, envelope: &Value) -> Result<Vec<String>> {
    let Some(table) = envelope.object().as_table_impl() else {
        return Err(Error::Eval("CLI envelope is not a table".to_owned()));
    };
    let value = table.get(cx, Symbol::new("args"))?;
    let Some(list) = value.object().as_list() else {
        return Err(Error::Eval(
            "CLI envelope field args is not a list".to_owned(),
        ));
    };
    list.to_vec(cx, Some(64))?
        .into_iter()
        .map(|value| match value.object().as_expr(cx)? {
            Expr::String(text) => Ok(text),
            other => Err(Error::Eval(format!(
                "CLI payload argument is not a string: {other:?}"
            ))),
        })
        .collect()
}

fn symbol_from_slash(text: &str) -> Symbol {
    match text.split_once('/') {
        Some((head, tail)) => Symbol::qualified(head, tail),
        None => Symbol::new(text),
    }
}

// ---------------------------------------------------------------------------
// The loadable `serve` verb: boots the web shell through the sim-run bootloader.
// ---------------------------------------------------------------------------

/// The verb the bootloader dispatches to serve the web shell (`sim serve ...`).
pub const WEB_SERVE_VERB: &str = "serve";

/// Returns the function symbol exported for the bootloader handoff.
pub fn web_serve_entrypoint_symbol() -> Symbol {
    cli_main_entrypoint_symbol(WEB_SERVE_VERB)
}

/// Builds host-owned cookbook state from the effective boot config.
pub type CookbookStateFactory = Arc<dyn Fn(&RuntimeConfigState) -> CookbookWebState + Send + Sync>;

/// Registers the `codec/lisp` boot codec and the web-shell `serve` verb onto an
/// existing [`Bootloader`], returning it for further composition. A downstream binary
/// can stack this with other serve libraries (e.g. MCP) onto one bootloader.
pub fn configure_web_bootloader(loader: Bootloader) -> Bootloader {
    configure_web_bootloader_base(loader).host_verb(WEB_SERVE_VERB, "lib/web-serve", || {
        Box::new(WebServeLib::new())
    })
}

/// Registers the web-shell `serve` verb with a host-provided cookbook state
/// factory. The supplied config library ids are read before the serve library is
/// instantiated, so the factory sees the effective config tables it needs.
pub fn configure_web_bootloader_with_cookbook(
    loader: Bootloader,
    config_libs: Vec<Symbol>,
    cookbook: CookbookStateFactory,
) -> Bootloader {
    configure_web_bootloader_base(loader).host_verb_with_config(
        WEB_SERVE_VERB,
        "lib/web-serve",
        config_libs,
        move |config| Box::new(WebServeLib::with_cookbook(cookbook(config))),
    )
}

fn configure_web_bootloader_base(loader: Bootloader) -> Bootloader {
    // Seat the cookbook eval Cx with the whole capability profile at the trusted
    // host boundary, where the bootloader holds the boot session's GrantSeat.
    // The profile grants pure/offline/deterministic vocabulary and omits live
    // effectful capabilities, so recipes that demand a denied capability fail
    // closed. run_recipe still gates each run on read-eval, so the capability is
    // required, not ambient.
    let loader = CookbookCapabilityProfile::granted()
        .into_iter()
        .fold(loader, |loader, capability| {
            loader.with_capability(capability)
        });
    // Modeled glasses voice recipes need host authority for the consent gate,
    // but recipe eval is still diminished by explicit allow-capability tags.
    let loader = loader.with_capability(CapabilityName::new("glasses/mic"));
    loader.host_lib("codec/lisp", || {
        Box::new(LispCodecLib::new(CodecId(1)).expect("lisp boot codec"))
    })
}

/// A standalone [`Bootloader`] pre-configured to serve the web shell: the `codec/lisp`
/// boot codec plus the `serve` verb. The thin `sim-web-shell` binary is just
/// `web_bootloader().run(..)`.
pub fn web_bootloader() -> Bootloader {
    configure_web_bootloader(Bootloader::standard())
}

/// Loadable library exporting the web-shell `serve` entrypoint.
pub struct WebServeLib {
    cookbook: Option<Arc<CookbookWebState>>,
}

impl WebServeLib {
    /// Builds the standalone web-serve library.
    pub fn new() -> Self {
        Self { cookbook: None }
    }

    /// Builds a web-serve library with host-provided cookbook state.
    pub fn with_cookbook(cookbook: CookbookWebState) -> Self {
        Self {
            cookbook: Some(Arc::new(cookbook)),
        }
    }
}

impl Default for WebServeLib {
    fn default() -> Self {
        Self::new()
    }
}

impl Lib for WebServeLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::qualified("lib", "web-serve"),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: vec![read_eval_capability()],
            exports: vec![Export::Function {
                symbol: web_serve_entrypoint_symbol(),
                function_id: None,
            }],
        }
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.function_value(
            web_serve_entrypoint_symbol(),
            cx.factory().opaque(Arc::new(WebServeEntrypoint {
                cookbook: self.cookbook.clone(),
            }))?,
        )?;
        Ok(())
    }
}

#[derive(Clone)]
struct WebServeEntrypoint {
    cookbook: Option<Arc<CookbookWebState>>,
}

impl Object for WebServeEntrypoint {
    fn display(&self, _cx: &mut Cx) -> Result<String> {
        Ok("cli/main/serve".to_owned())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl ObjectCompat for WebServeEntrypoint {
    fn as_callable(&self) -> Option<&dyn Callable> {
        Some(self)
    }
}

impl Callable for WebServeEntrypoint {
    fn call(&self, cx: &mut Cx, args: Args) -> Result<Value> {
        // Parse `--addr` / `--atelier-root` from the boot envelope (skipping the
        // `serve` verb), then run the blocking HTTP loop in the bootloader cx.
        let mut config = match args.values().first() {
            Some(envelope) => {
                let payload = envelope_args(cx, envelope)?;
                parse_serve_config(payload.into_iter().skip(1))?
            }
            None => ServeConfig::default(),
        };
        config.cookbook.clone_from(&self.cookbook);
        serve_with_cx(cx, &config)
            .map_err(|err| Error::Eval(format!("web serve failed: {err}")))?;
        cx.factory().bool(true)
    }
}

/// Parse the serve envelope arguments, failing closed on malformed input: a
/// bare `--addr`/`--atelier-root` with no value, or any unknown flag/positional,
/// is an error rather than a silently-ignored argument (so `--add 0.0.0.0:80`
/// cannot quietly leave the shell bound to loopback).
fn parse_serve_config(args: impl Iterator<Item = String>) -> Result<ServeConfig> {
    let mut config = ServeConfig::default();
    let mut iter = args;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--addr" => {
                config.addr = iter
                    .next()
                    .ok_or_else(|| Error::Eval("--addr requires a value".to_owned()))?;
            }
            other if other.starts_with("--addr=") => {
                config.addr = other["--addr=".len()..].to_owned();
            }
            "--atelier-root" => {
                config.atelier_root = iter
                    .next()
                    .ok_or_else(|| Error::Eval("--atelier-root requires a value".to_owned()))?
                    .into();
            }
            other if other.starts_with("--atelier-root=") => {
                config.atelier_root = other["--atelier-root=".len()..].into();
            }
            "--dry-run" => {
                config.dry_run = true;
            }
            other => {
                return Err(Error::Eval(format!("unknown serve argument: {other}")));
            }
        }
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::parse_serve_config;

    fn parse(args: &[&str]) -> super::Result<super::ServeConfig> {
        parse_serve_config(args.iter().map(|a| (*a).to_owned()))
    }

    #[test]
    fn missing_addr_value_errors() {
        let err = parse(&["--addr"]).expect_err("bare --addr must error");
        assert!(err.to_string().contains("--addr requires a value"));
    }

    #[test]
    fn missing_atelier_root_value_errors() {
        let err = parse(&["--atelier-root"]).expect_err("bare --atelier-root must error");
        assert!(err.to_string().contains("--atelier-root requires a value"));
    }

    #[test]
    fn unknown_flag_errors() {
        // A typo such as `--add` must fail visibly, not silently bind loopback.
        let err = parse(&["--add", "0.0.0.0:80"]).expect_err("unknown flag must error");
        assert!(err.to_string().contains("unknown serve argument: --add"));
    }

    #[test]
    fn unknown_positional_errors() {
        let err = parse(&["serve-extra"]).expect_err("stray positional must error");
        assert!(
            err.to_string()
                .contains("unknown serve argument: serve-extra")
        );
    }

    #[test]
    fn dry_run_still_succeeds() {
        let config = parse(&["--dry-run"]).expect("--dry-run must parse");
        assert!(config.dry_run);
    }

    #[test]
    fn addr_and_atelier_root_parse() {
        let config = parse(&["--addr", "127.0.0.1:9000", "--atelier-root", "/tmp/atelier"])
            .expect("valid args must parse");
        assert_eq!(config.addr, "127.0.0.1:9000");
        assert_eq!(config.atelier_root.to_str(), Some("/tmp/atelier"));
        assert!(!config.dry_run);
    }

    #[test]
    fn inline_addr_value_parses() {
        let config = parse(&["--addr=127.0.0.1:9100"]).expect("inline addr must parse");
        assert_eq!(config.addr, "127.0.0.1:9100");
    }
}
