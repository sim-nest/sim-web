// SPDX-License-Identifier: MPL-2.0
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Thin launcher: simdoc uses the isolated encoder, and file-size checks use
//! the shared tooling workspace.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run(args: Vec<String>) -> Result<(), String> {
    let program = args.first().map(String::as_str).unwrap_or("xtask");
    let root = env::current_dir().map_err(|err| format!("current dir: {err}"))?;
    match args.get(1).map(String::as_str) {
        Some("simdoc") => run_simdoc(&root, &args),
        Some("check-file-sizes") if args.len() == 2 => run_file_sizes(&root),
        _ => Err(format!(
            "usage: {program} simdoc [--check] | check-file-sizes"
        )),
    }
}

fn run_simdoc(root: &Path, args: &[String]) -> Result<(), String> {
    let manifest = locate_simdoc_manifest(root)?;
    let mut command = Command::new("cargo");
    command.args(["run", "--locked", "--offline", "--manifest-path"]);
    command.arg(&manifest);
    command.args(["--quiet", "--", "simdoc", "--repo-root"]);
    command.arg(root);
    for arg in args.iter().skip(2) {
        command.arg(arg);
    }
    finish(command, "isolated simdoc encoder")
}

fn run_file_sizes(root: &Path) -> Result<(), String> {
    let manifest = locate_sim_tooling_manifest(root)?;
    let mut command = Command::new("cargo");
    command.args(["run", "--manifest-path"]);
    command.arg(&manifest);
    command.args(["--quiet", "--", "check-file-sizes", "--repo-root"]);
    command.arg(root);
    finish(command, "shared file-size checker")
}

fn finish(mut command: Command, name: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|err| format!("run {name}: {err}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} failed with status {status}"))
    }
}

fn locate_simdoc_manifest(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("SIMDOC_TOOLING_MANIFEST") {
        // CI and release generation set this to the tooling workspace
        // manifest. That manifest refuses the simdoc verb.
        return isolated_manifest_from_hint(Path::new(&path));
    }
    let isolated = repo_root
        .parent()
        .unwrap_or(repo_root)
        .join("sim-tooling")
        .join("crates")
        .join("simdoc")
        .join("Cargo.toml");
    if isolated.is_file() {
        return Ok(isolated);
    }
    Err(
        "set SIMDOC_TOOLING_MANIFEST to the sim-tooling checkout or crates/simdoc/Cargo.toml"
            .to_owned(),
    )
}

fn isolated_manifest_from_hint(hint: &Path) -> Result<PathBuf, String> {
    let isolated_rel = Path::new("crates").join("simdoc").join("Cargo.toml");
    let candidates = [
        hint.to_path_buf(),
        hint.join("Cargo.toml"),
        hint.join(&isolated_rel),
        hint.parent().unwrap_or(hint).join(&isolated_rel),
    ];
    for candidate in candidates {
        if candidate.is_file() && candidate.ends_with(&isolated_rel) {
            return Ok(candidate);
        }
    }
    Err(format!(
        "SIMDOC_TOOLING_MANIFEST={} does not locate crates/simdoc/Cargo.toml",
        hint.display()
    ))
}

fn locate_sim_tooling_manifest(repo_root: &Path) -> Result<PathBuf, String> {
    if let Ok(path) = env::var("SIMDOC_TOOLING_MANIFEST") {
        return Ok(PathBuf::from(path));
    }
    let sibling = repo_root
        .parent()
        .unwrap_or(repo_root)
        .join("sim-tooling")
        .join("Cargo.toml");
    if sibling.is_file() {
        return Ok(sibling);
    }
    Err("set SIMDOC_TOOLING_MANIFEST to the sim-tooling Cargo.toml".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn isolated_rel() -> PathBuf {
        Path::new("crates").join("simdoc").join("Cargo.toml")
    }

    #[test]
    fn tooling_hints_resolve_to_the_isolated_encoder() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives in the repository root")
            .to_path_buf();
        let selected = locate_simdoc_manifest(&repo).expect("isolated encoder");
        assert!(selected.ends_with(isolated_rel()));
        assert!(selected.with_file_name("Cargo.lock").is_file());

        let tooling = selected
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .expect("crates/simdoc lives in the tooling checkout");
        assert_eq!(
            isolated_manifest_from_hint(&tooling.join("Cargo.toml")).expect("workspace hint"),
            selected
        );
        assert_eq!(
            isolated_manifest_from_hint(&selected).expect("isolated hint"),
            selected
        );
        assert_eq!(
            isolated_manifest_from_hint(tooling).expect("checkout hint"),
            selected
        );
    }

    #[test]
    fn missing_tooling_hint_is_an_error() {
        let missing = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("no-such-tooling")
            .join("Cargo.toml");
        let error = isolated_manifest_from_hint(&missing).expect_err("missing hint");
        assert!(error.contains("does not locate crates/simdoc/Cargo.toml"));
    }

    #[test]
    fn file_size_checks_keep_the_shared_workspace_manifest() {
        let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("xtask lives in the repository root")
            .to_path_buf();
        let selected = locate_sim_tooling_manifest(&repo).expect("shared workspace");
        assert!(selected.ends_with("Cargo.toml"));
        assert!(!selected.ends_with(isolated_rel()));
    }
}

// conformance: Generated-doc launcher tests resolve the isolated simdoc encoder from a tooling workspace manifest, the encoder manifest, and the tooling checkout, and reject a hint that does not locate that encoder.
