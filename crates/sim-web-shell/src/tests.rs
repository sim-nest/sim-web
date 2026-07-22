//! Tests for the shell asset routing.

// conformance: web shell host opens runtime-backed workspaces.

use std::sync::Arc;

use crate::{AtelierCliLib, BrowseCliLib, ServeConfig, assets::asset_for, serve_with_cx};
use sim_codec_lisp::LispCodecLib;
use sim_kernel::{
    Args, Cx, DefaultFactory, EagerPolicy, NoopEvalPolicy, Symbol, Value, read_eval_capability,
};
use sim_lib_server::CookbookWebState;
use sim_test_support::register_core_classes;

trait GrantOutcome {
    fn expect_granted(self);
}

impl GrantOutcome for () {
    fn expect_granted(self) {}
}

impl GrantOutcome for sim_kernel::Result<()> {
    fn expect_granted(self) {
        self.unwrap();
    }
}

macro_rules! expect_granted {
    ($grant:expr) => {{
        #[allow(clippy::let_unit_value)]
        let grant_result = $grant;
        #[allow(clippy::unit_arg)]
        grant_result.expect_granted();
    }};
}

#[test]
fn root_serves_the_shell_page() {
    let asset = asset_for("/").expect("root must resolve to the shell");
    assert_eq!(asset.content_type, "text/html; charset=utf-8");
    let body = std::str::from_utf8(asset.body).expect("shell is utf-8");
    assert!(
        body.contains("SIM Web-UI"),
        "shell page must carry the title"
    );
}

#[test]
fn index_html_aliases_root() {
    let root = asset_for("/").expect("root");
    let index = asset_for("/index.html").expect("index");
    assert_eq!(root.body, index.body);
}

#[test]
fn query_strings_are_ignored_when_routing() {
    let asset = asset_for("/styles/theme.css?v=1").expect("css with query string");
    assert_eq!(asset.content_type, "text/css; charset=utf-8");
}

#[test]
fn unknown_paths_fail_closed() {
    assert!(asset_for("/secret").is_none());
    assert!(asset_for("/../Cargo.toml").is_none());
}

#[test]
fn interpreter_modules_are_served_as_javascript() {
    for path in [
        "/interpreter/app.js",
        "/interpreter/glasses.js",
        "/interpreter/scene.js",
        "/interpreter/diff.js",
        "/interpreter/intent.js",
        "/interpreter/keymap.js",
    ] {
        let asset = asset_for(path).unwrap_or_else(|| panic!("{path} must be served"));
        assert_eq!(asset.content_type, "text/javascript; charset=utf-8");
        assert!(!asset.body.is_empty(), "{path} must have a body");
    }
}

#[test]
fn interpreter_module_import_graph_is_served() {
    let mut seen = std::collections::BTreeSet::new();
    assert_served_import_graph("/interpreter/app.js", &mut seen);

    assert!(
        seen.contains("/interpreter/keymap.js"),
        "scene.js imports keymap.js and the router must serve it"
    );
    assert!(
        seen.contains("/interpreter/glasses.js"),
        "app.js imports the browser-local glasses client"
    );
}

#[test]
fn the_session_bridge_module_is_served() {
    let asset = asset_for("/interpreter/session.js").expect("session module must be served");
    assert_eq!(asset.content_type, "text/javascript; charset=utf-8");
    let body = std::str::from_utf8(asset.body).unwrap();
    assert!(
        body.contains("/api/session/intent"),
        "the bridge posts intents to the session route"
    );
}

#[test]
fn the_app_wires_the_live_session_bridge() {
    let js = asset_text("/interpreter/app.js");
    assert!(
        js.contains("postIntent"),
        "app forwards intents to the bridge"
    );
    assert!(js.contains("openSession"), "app opens the initial scene");
    assert!(
        js.contains("sim-scene-patch"),
        "app dispatches scene patches for diff.js to apply"
    );
}

#[test]
fn the_shell_page_loads_the_interpreter_module() {
    let asset = asset_for("/").expect("root");
    let body = std::str::from_utf8(asset.body).unwrap();
    assert!(
        body.contains("/interpreter/app.js"),
        "the shell page must load the interpreter entry module"
    );
}

#[test]
fn root_shell_has_cookbook_nav_link() {
    let body = asset_text("/");
    assert!(
        body.contains("href=\"/cookbook\""),
        "root links to cookbook"
    );
    assert!(body.contains(">Cookbook<"), "link is labeled Cookbook");
    assert!(body.contains("href=\"/atelier\""), "root links to Atelier");
    assert!(body.contains(">Atelier<"), "link is labeled Atelier");
}

#[test]
fn cookbook_route_serves_page_layout() {
    let asset = asset_for("/cookbook").expect("cookbook page");
    assert_eq!(asset.content_type, "text/html; charset=utf-8");
    let body = std::str::from_utf8(asset.body).unwrap();
    assert!(body.contains("id=\"cookbook-search\""), "has search box");
    assert!(body.contains("id=\"cookbook-tree\""), "has left rail tree");
    assert!(body.contains("id=\"recipe-pane\""), "has main pane");
    assert!(
        body.contains("data-api-root=\"/api/cookbook\""),
        "uses cookbook API"
    );
}

#[test]
fn cookbook_assets_are_served() {
    let css = asset_for("/cookbook/cookbook.css").expect("cookbook css");
    assert_eq!(css.content_type, "text/css; charset=utf-8");
    let js = asset_for("/cookbook/cookbook.js").expect("cookbook js");
    assert_eq!(js.content_type, "text/javascript; charset=utf-8");
}

#[test]
fn atelier_route_serves_page_layout() {
    let asset = asset_for("/atelier").expect("atelier page");
    assert_eq!(asset.content_type, "text/html; charset=utf-8");
    let body = std::str::from_utf8(asset.body).unwrap();
    assert!(body.contains("id=\"atelier-app\""), "has Atelier app mount");
    assert!(
        body.contains("data-api-root=\"/api/atelier\""),
        "uses Atelier API"
    );
    assert!(body.contains("/atelier/atelier.js"), "loads Atelier script");
}

#[test]
fn atelier_assets_are_served() {
    let css = asset_for("/atelier/atelier.css").expect("atelier css");
    assert_eq!(css.content_type, "text/css; charset=utf-8");
    let js = asset_for("/atelier/atelier.js").expect("atelier js");
    assert_eq!(js.content_type, "text/javascript; charset=utf-8");
}

#[test]
fn atelier_script_targets_shell_api_and_panels() {
    let js = asset_text("/atelier/atelier.js");
    for expected in [
        "/api/atelier",
        "#atelier-navigation",
        "#atelier-panels",
        "#atelier-radar",
        "#atelier-firewall",
    ] {
        assert!(js.contains(expected), "missing {expected}");
    }
}

#[test]
fn atelier_cli_lib_claims_loaded_cli_entrypoint() {
    let mut cx = cli_cx();
    cx.load_lib(&AtelierCliLib).unwrap();
    let envelope = cli_envelope(&mut cx, "atelier", &["atelier", "--dry-run"]);
    let value = cx
        .call_function(
            &Symbol::qualified("cli", "main/atelier"),
            Args::new(vec![envelope]),
        )
        .unwrap();

    assert!(value.object().truth(&mut cx).unwrap());
}

#[test]
fn browse_cli_lib_claims_loaded_cli_entrypoint() {
    let mut cx = cli_cx();
    cx.load_lib(&BrowseCliLib).unwrap();
    let envelope = cli_envelope(&mut cx, "browse", &["browse"]);
    let value = cx
        .call_function(
            &Symbol::qualified("cli", "main/browse"),
            Args::new(vec![envelope]),
        )
        .unwrap();

    assert!(value.object().truth(&mut cx).unwrap());
}

#[test]
fn cookbook_script_targets_required_apis() {
    let js = asset_text("/cookbook/cookbook.js");
    for expected in [
        "/api/cookbook",
        "/search?q=",
        "/recipe/",
        "/run",
        "method: \"POST\"",
    ] {
        assert!(js.contains(expected), "missing {expected}");
    }
    assert!(js.contains("Next recipe ->"), "has next control text");
}

#[test]
fn cookbook_script_renders_lib_tree_with_badges() {
    // The browser renders the lib-first tree when the API provides `libs`, while
    // retaining the family fallback for older payloads.
    let js = asset_text("/cookbook/cookbook.js");
    for expected in [
        "data.libs",
        "state.libs",
        "state.hasLibTree",
        "hasLibTree(data)",
        "renderLibTree",
        "lib-title",
        "group-title",
        "data.families",
        "state.families",
        "renderFamilyTree",
        "family-title",
        "domain-title",
        "recipeBadge",
        "sandbox-descriptor",
    ] {
        assert!(js.contains(expected), "missing {expected}");
    }
    let css = asset_text("/cookbook/cookbook.css");
    for expected in [
        ".badge.runnable",
        ".badge.descriptor",
        ".lib-title",
        ".group-title",
        ".domain-title",
    ] {
        assert!(css.contains(expected), "css missing {expected}");
    }
}

#[test]
fn cookbook_script_persists_branch_state() {
    let js = asset_text("/cookbook/cookbook.js");
    for expected in [
        "function treeStateKey(kind, id)",
        "`sim-cookbook:${kind}:${id}`",
        "localStorage.getItem(treeStateKey(kind, id))",
        "localStorage.setItem(treeStateKey(kind, id), open ? \"1\" : \"0\")",
        "function setDetailsOpen(details, kind, id, defaultOpen, forceOpen = false)",
        "details.open = forceOpen || (saved == null ? defaultOpen : saved === \"1\")",
        "state.visibleIds !== null",
        "setDetailsOpen(libEl, \"lib\", lib.id, true, searching)",
        "setDetailsOpen(groupEl, \"group\", `${lib.id}/${group.name}`, false, searching)",
    ] {
        assert!(js.contains(expected), "missing {expected}");
    }
}

#[test]
fn cookbook_script_renders_lifecycle_actions() {
    let js = asset_text("/cookbook/cookbook.js");
    for expected in [
        "recipe.action === \"load\"",
        "recipe.action === \"unload\"",
        "recipe.lib",
        "recipe.loaded",
        "actionLabel(recipe)",
        "runSelectedRecipe(recipe, results)",
        "await loadCookbook({",
        "recipe.action === \"unload\" ? \"load\" : null",
        "dataset.recipeAction",
        "dataset.recipeLib",
        "dataset.recipeLoaded",
    ] {
        assert!(js.contains(expected), "missing {expected}");
    }
    let css = asset_text("/cookbook/cookbook.css");
    for expected in [
        ".badge.lifecycle.load",
        ".badge.lifecycle.unload",
        ".lifecycle-meta",
        ".recipe-actions button.lifecycle-action.load",
        ".recipe-actions button.lifecycle-action.unload",
    ] {
        assert!(css.contains(expected), "css missing {expected}");
    }
}

#[test]
fn web_serve_does_not_preload_demo_codecs() {
    let serve = include_str!("serve.rs");
    for forbidden in [
        "JsonCodecLib",
        "BinaryCodecLib",
        "ChatCodecLib",
        "AlgolCodecLib",
        "install_codecs",
    ] {
        assert!(
            !serve.contains(forbidden),
            "serve.rs still contains {forbidden}"
        );
    }
    assert!(
        serve.contains("CookbookWebState::seeded()"),
        "serve.rs must build cookbook state from the loadable directory"
    );

    let mut cx = cli_cx();
    serve_with_cx(
        &mut cx,
        &ServeConfig {
            dry_run: true,
            ..ServeConfig::default()
        },
    )
    .unwrap();
    let loaded: Vec<String> = cx
        .registry()
        .libs()
        .iter()
        .map(|lib| lib.manifest.id.as_qualified_str())
        .collect();
    for forbidden in [
        "codec/json",
        "codec/binary",
        "codec/chat",
        "codec/algol",
        "numbers/i64",
        "numbers/cas",
    ] {
        assert!(
            !loaded.iter().any(|id| id == forbidden),
            "dry-run preloaded {forbidden}: {loaded:?}"
        );
    }
}

#[test]
fn standalone_serve_config_uses_seeded_fixture_directory() {
    let mut cx = cookbook_cx();
    let response = crate::serve::cookbook_index_for_test(&mut cx, &ServeConfig::default()).unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let json: serde_json::Value = serde_json::from_str(&response.body).unwrap();
    let libs = json["libs"]
        .as_array()
        .unwrap_or_else(|| panic!("libs array in {}", response.body));

    assert!(
        libs.iter()
            .any(|lib| lib["id"].as_str() == Some("numbers/cas")),
        "{}",
        response.body
    );
}

#[test]
fn serve_config_can_use_host_cookbook_state() {
    let mut cx = cookbook_cx();
    let response = crate::serve::cookbook_index_for_test(
        &mut cx,
        &ServeConfig {
            cookbook: Some(Arc::new(CookbookWebState::empty())),
            ..ServeConfig::default()
        },
    )
    .unwrap();
    assert_eq!(response.status, 200, "{}", response.body);
    let json: serde_json::Value = serde_json::from_str(&response.body).unwrap();

    assert_eq!(
        json["libs"]
            .as_array()
            .unwrap_or_else(|| panic!("libs array in {}", response.body))
            .len(),
        0
    );
    assert!(!response.body.contains("numbers/cas"), "{}", response.body);
}

fn asset_text(path: &str) -> String {
    let asset = asset_for(path).unwrap_or_else(|| panic!("{path} must be served"));
    std::str::from_utf8(asset.body).unwrap().to_owned()
}

fn assert_served_import_graph(path: &str, seen: &mut std::collections::BTreeSet<String>) {
    if !seen.insert(path.to_owned()) {
        return;
    }

    let source = asset_text(path);
    for import in relative_module_imports(&source) {
        let next = resolve_relative_module_path(path, &import)
            .unwrap_or_else(|| panic!("could not resolve import {import:?} from {path}"));
        let asset = asset_for(&next).unwrap_or_else(|| panic!("{path} imports unrouted {next}"));
        assert_eq!(
            asset.content_type, "text/javascript; charset=utf-8",
            "{next} must be served as JavaScript"
        );
        assert_served_import_graph(&next, seen);
    }
}

fn relative_module_imports(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with("import ") {
                return None;
            }
            let quoted = line.split('"').nth(1).or_else(|| line.split('\'').nth(1))?;
            quoted.starts_with('.').then(|| quoted.to_owned())
        })
        .collect()
}

fn resolve_relative_module_path(from: &str, import: &str) -> Option<String> {
    if !import.starts_with("./") {
        return None;
    }
    let dir = from.rsplit_once('/')?.0;
    Some(format!("{dir}/{}", import.trim_start_matches("./")))
}

fn cli_cx() -> Cx {
    Cx::new(Arc::new(NoopEvalPolicy), Arc::new(DefaultFactory))
}

fn cookbook_cx() -> Cx {
    let (mut cx, seat) = Cx::new_seated(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    register_core_classes(&mut cx);
    let lisp = LispCodecLib::new(cx.registry_mut().fresh_codec_id()).unwrap();
    cx.load_lib(&lisp).unwrap();
    expect_granted!(seat.grant(&mut cx, read_eval_capability()));
    cx
}

fn cli_envelope(cx: &mut Cx, verb: &str, args: &[&str]) -> Value {
    let verb = cx.factory().string(verb.to_owned()).unwrap();
    let args = cx
        .factory()
        .list(
            args.iter()
                .map(|arg| cx.factory().string((*arg).to_owned()).unwrap())
                .collect(),
        )
        .unwrap();
    cx.factory()
        .table(vec![
            (Symbol::new("verb"), verb),
            (Symbol::new("args"), args),
        ])
        .unwrap()
}

#[test]
fn the_theme_defines_motion_focus_and_reduced_motion_rules() {
    let css = asset_text("/styles/theme.css");
    // Reduced-motion mode disables animation in the interpreter, not per lens.
    assert!(
        css.contains("prefers-reduced-motion"),
        "honors OS reduced-motion"
    );
    assert!(
        css.contains("data-reduced-motion"),
        "honors explicit reduced-motion"
    );
    // Visible focus on canvas/graph surfaces, not only DOM controls.
    assert!(
        css.contains(":focus-visible"),
        "defines a visible focus ring"
    );
    assert!(css.contains("--focus-ring"), "has a focus-ring token");
    // Motion and icon tokens live in the theme, not per lens.
    assert!(css.contains("--motion-base"), "has motion tokens");
    assert!(css.contains("--icon-play"), "has an icon set");
    // Status never relies on color alone: badges carry a shape glyph.
    assert!(
        css.contains(".badge.error::before"),
        "status carries a non-color token"
    );
}

#[test]
fn the_interpreter_labels_interactive_nodes_for_screen_readers() {
    let js = asset_text("/interpreter/scene.js");
    assert!(
        js.contains("aria-label"),
        "interactive nodes carry screen-reader labels"
    );
    assert!(js.contains("tabindex"), "canvas nodes are focusable");
    assert!(js.contains("role"), "interactive nodes carry roles");
}

#[test]
fn the_shell_supports_reduced_motion_and_keyboard_operation() {
    let js = asset_text("/interpreter/app.js");
    assert!(
        js.contains("prefers-reduced-motion"),
        "applies reduced-motion"
    );
    assert!(js.contains("keydown"), "installs a keyboard spine");
}
