//! The `sim-web-shell` binary: a thin bootloader that serves the SIM WebUI shell.
//!
//! `sim-web-shell [--addr HOST:PORT] [--atelier-root PATH]` boots through
//! [`sim_run_core::Bootloader`] (via [`sim_web_shell::web_bootloader`]) -- the same
//! runtime the `sim` binary uses -- with the `codec/lisp` boot codec loaded, and
//! dispatches the `cli/main/serve` entrypoint. Equivalent to `sim serve ...`. This
//! binary constructs no `Cx`; the serve loop runs in the bootloader-provided cx.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    // parse_args drops argv[0]; set the lisp boot codec and inject the `serve` verb so
    // `sim-web-shell --addr ...` dispatches the web serve library like `sim serve ...`.
    let mut args: Vec<OsString> = ["sim-web-shell", "--codec", "lisp", "serve"]
        .into_iter()
        .map(OsString::from)
        .collect();
    args.extend(std::env::args_os().skip(1));

    match sim_web_shell::web_bootloader().run(args) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(code) => ExitCode::from(code as u8),
        Err(err) => {
            eprintln!("sim-web-shell: {err}");
            ExitCode::from(2)
        }
    }
}
