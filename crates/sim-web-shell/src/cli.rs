//! Loadable CLI claims for the web shell surfaces.

use std::sync::Arc;

use sim_codec_lisp::LispCodecLib;
use sim_kernel::{
    AbiVersion, Args, CORE_FUNCTION_CLASS_ID, Callable, ClassRef, CodecId, Cx, Error, Export, Expr,
    Lib, LibManifest, LibTarget, Linker, LoadCx, Object, ObjectCompat, Result, Symbol, Value,
    Version,
};
use sim_run_core::{Bootloader, cli_main_entrypoint_symbol};

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

/// A [`Bootloader`] pre-configured to serve the web shell: the `codec/lisp` boot
/// codec plus the `serve` verb. The thin `sim-web-shell` binary is just
/// `web_bootloader().run(..)`.
pub fn web_bootloader() -> Bootloader {
    Bootloader::standard()
        .host_lib("codec/lisp", || {
            Box::new(LispCodecLib::new(CodecId(1)).expect("lisp boot codec"))
        })
        .host_verb(WEB_SERVE_VERB, "lib/web-serve", || Box::new(WebServeLib))
}

/// Loadable library exporting the web-shell `serve` entrypoint.
pub struct WebServeLib;

impl Lib for WebServeLib {
    fn manifest(&self) -> LibManifest {
        LibManifest {
            id: Symbol::qualified("lib", "web-serve"),
            version: Version(env!("CARGO_PKG_VERSION").to_owned()),
            abi: AbiVersion { major: 0, minor: 1 },
            target: LibTarget::HostRegistered,
            requires: Vec::new(),
            capabilities: Vec::new(),
            exports: vec![Export::Function {
                symbol: web_serve_entrypoint_symbol(),
                function_id: None,
            }],
        }
    }

    fn load(&self, cx: &mut LoadCx, linker: &mut Linker<'_>) -> Result<()> {
        linker.function_value(
            web_serve_entrypoint_symbol(),
            cx.factory().opaque(Arc::new(WebServeEntrypoint))?,
        )?;
        Ok(())
    }
}

#[derive(Clone)]
struct WebServeEntrypoint;

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
        let config = match args.values().first() {
            Some(envelope) => {
                let payload = envelope_args(cx, envelope)?;
                parse_serve_config(payload.into_iter().skip(1))
            }
            None => ServeConfig::default(),
        };
        serve_with_cx(cx, &config)
            .map_err(|err| Error::Eval(format!("web serve failed: {err}")))?;
        cx.factory().bool(true)
    }
}

fn parse_serve_config(args: impl Iterator<Item = String>) -> ServeConfig {
    let mut config = ServeConfig::default();
    let mut iter = args;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--addr" => {
                if let Some(addr) = iter.next() {
                    config.addr = addr;
                }
            }
            other if other.starts_with("--addr=") => {
                config.addr = other["--addr=".len()..].to_owned();
            }
            "--atelier-root" => {
                if let Some(root) = iter.next() {
                    config.atelier_root = root.into();
                }
            }
            other if other.starts_with("--atelier-root=") => {
                config.atelier_root = other["--atelier-root=".len()..].into();
            }
            _ => {}
        }
    }
    config
}
