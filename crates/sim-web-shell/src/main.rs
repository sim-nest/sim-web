//! Entry point for `sim-web-shell`.
//!
//! Usage: `sim-web-shell [--addr HOST:PORT] [--atelier-root PATH]`. Serves the
//! embedded SIM WebUI shell, cookbook page, and Atelier cache view. Keep this
//! file thin: argument parsing and bootstrap only; serving logic lives in
//! `serve`.

#![forbid(unsafe_code)]

use sim_web_shell::{ServeConfig, serve};

fn main() -> std::io::Result<()> {
    let config = parse_args(std::env::args().skip(1));
    serve(&config)
}

fn parse_args(args: impl Iterator<Item = String>) -> ServeConfig {
    let mut config = ServeConfig::default();
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => {
                if let Some(addr) = args.next() {
                    config.addr = addr;
                }
            }
            other if other.starts_with("--addr=") => {
                config.addr = other["--addr=".len()..].to_owned();
            }
            "--atelier-root" => {
                if let Some(root) = args.next() {
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
