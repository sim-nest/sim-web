//! Repository recipe harness for Lisp surface fixtures and Rust examples.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn run() -> Result<(), String> {
    let root = std::env::current_dir().map_err(|error| format!("current dir: {error}"))?;
    let mut manifests = Vec::new();
    collect_recipe_manifests(&root.join("crates"), &mut manifests)?;
    manifests.sort();
    if manifests.is_empty() {
        return Err("check-recipes: no recipe.toml files found".to_owned());
    }

    let status = Command::new("cargo")
        .args(["test", "--workspace", "--quiet"])
        .current_dir(&root)
        .status()
        .map_err(|error| format!("run workspace recipe tests: {error}"))?;
    if !status.success() {
        return Err(format!(
            "check-recipes: workspace recipe tests failed with {status}"
        ));
    }

    let mut examples = 0;
    for manifest in &manifests {
        let text = fs::read_to_string(manifest)
            .map_err(|error| format!("read {}: {error}", manifest.display()))?;
        validate_files(&root, manifest, &text)?;
        match optional_field(&text, "harness") {
            Some("cargo-example") => {
                run_example(&root, manifest, &text)?;
                examples += 1;
            }
            Some(other) => {
                return Err(format!(
                    "{}: unsupported recipe harness {other:?}",
                    relative(&root, manifest)
                ));
            }
            None if field(&text, "codec")? == "lisp" => {}
            None => {
                return Err(format!(
                    "{}: a non-Lisp recipe requires an executable harness",
                    relative(&root, manifest)
                ));
            }
        }
    }
    println!(
        "check-recipes: checked {} recipe(s), including {examples} exact cargo example(s)",
        manifests.len()
    );
    Ok(())
}

fn validate_files(root: &Path, manifest: &Path, text: &str) -> Result<(), String> {
    let directory = manifest
        .parent()
        .ok_or_else(|| format!("recipe has no directory: {}", manifest.display()))?;
    for key in ["setup", "purpose"] {
        let name = field(text, key)?;
        let path = directory.join(name);
        if !path.is_file() {
            return Err(format!(
                "{}: {key} file {} is missing",
                relative(root, manifest),
                path.display()
            ));
        }
    }
    Ok(())
}

fn run_example(root: &Path, manifest: &Path, text: &str) -> Result<(), String> {
    let package = field(text, "package")?;
    let example = field(text, "example")?;
    let expected_name = field(text, "expected")?;
    let expected_path = manifest
        .parent()
        .expect("validated recipe directory")
        .join(expected_name);
    let expected = fs::read_to_string(&expected_path)
        .map_err(|error| format!("read {}: {error}", expected_path.display()))?;
    let output = Command::new("cargo")
        .args(["run", "--quiet", "-p", package, "--example", example])
        .current_dir(root)
        .output()
        .map_err(|error| format!("run recipe {}: {error}", relative(root, manifest)))?;
    if !output.status.success() {
        return Err(format!(
            "recipe {} failed:\n{}",
            relative(root, manifest),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let actual = String::from_utf8(output.stdout)
        .map_err(|error| format!("recipe {} emitted non-UTF-8: {error}", manifest.display()))?;
    if actual.trim_end() != expected.trim_end() {
        return Err(format!(
            "recipe {} output mismatch\nexpected:\n{}\nactual:\n{}",
            relative(root, manifest),
            expected.trim_end(),
            actual.trim_end()
        ));
    }
    Ok(())
}

fn field<'a>(text: &'a str, name: &str) -> Result<&'a str, String> {
    optional_field(text, name).ok_or_else(|| format!("recipe metadata missing quoted {name}"))
}

fn optional_field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let prefix = format!("{name} = \"");
    text.lines()
        .find_map(|line| line.strip_prefix(&prefix)?.strip_suffix('"'))
}

fn collect_recipe_manifests(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    if !path.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path).map_err(|error| format!("read {}: {error}", path.display()))? {
        let entry = entry.map_err(|error| format!("read {} entry: {error}", path.display()))?;
        let path = entry.path();
        if path.is_dir() {
            collect_recipe_manifests(&path, files)?;
        } else if path.file_name().and_then(|name| name.to_str()) == Some("recipe.toml") {
            files.push(path);
        }
    }
    Ok(())
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
