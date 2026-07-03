//! Surface-neutral command palette, focus model, accessibility metadata, and
//! diagnostics presentation (VIEW4.06).
//!
//! These are the interaction semantics SHARED by every view surface -- the TUI
//! (the `sim-view-tty` consumer) and the Web UI alike. The module is
//! deliberately surface-neutral: it emits ordinary
//! [`Scene`](sim_lib_scene) values and [`Intent`](sim_lib_intent) values and
//! reads them back, but it never spells ANSI, DOM, or ARIA. A terminal renderer
//! turns a focus annotation into a highlight; a browser turns the same
//! annotation into a `:focus` ring and the [`A11y`] record into ARIA
//! attributes. The core Scene carries only open metadata, so neither surface's
//! vocabulary leaks into the shared model.
//!
//! Four facilities live here:
//!
//! - a [`with_focus`]/[`focused_id`]/[`move_focus`] focus model stored as a
//!   `focus` metadata field, not a new scene kind;
//! - a [`Command`] palette: [`palette_scene`] renders a filtered, deterministic
//!   overlay and [`palette_intent`] reduces a chosen command to a validated
//!   [`Intent`](sim_lib_intent);
//! - an [`A11y`] accessibility record attached as an open `a11y` map via
//!   [`with_a11y`] and read back with [`a11y_of`];
//! - [`diagnostics_scene`], which presents a rejected [`Draft`]'s diagnostics as
//!   a deterministic overlay.
//!
//! # Example
//!
//! ```
//! use sim_kernel::Symbol;
//! use sim_lib_view::palette::{Command, CommandKind, palette_intent, palette_scene};
//!
//! let cmd = Command {
//!     id: Symbol::new("run"),
//!     label: "Run validation".to_owned(),
//!     kind: CommandKind::Invoke,
//! };
//! // The overlay lists the command and validates as a Scene.
//! let scene = palette_scene(std::slice::from_ref(&cmd), "run");
//! assert!(sim_lib_scene::validate_scene(&scene).is_ok());
//! // The chosen command reduces to a validated Intent.
//! let intent = palette_intent(&cmd, "main", 7).unwrap();
//! assert!(sim_lib_intent::validate_intent(&intent).is_ok());
//! ```

use sim_kernel::{Diagnostic, Error, Expr, Result, Severity, Symbol};
use sim_lib_intent::{Origin, intent, validate_intent};
use sim_lib_scene::node;
use sim_value::build::{list, map, sym, text};

use crate::contract::Draft;

/// The map key under which the focus model stores the focused node id.
pub const FOCUS_KEY: &str = "focus";

/// The map key under which [`with_a11y`] stores the accessibility record.
pub const A11Y_KEY: &str = "a11y";

/// A direction to advance focus in [`move_focus`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusDir {
    /// Advance to the next id in order, wrapping past the end to the first.
    Next,
    /// Advance to the previous id in order, wrapping past the start to the last.
    Prev,
}

/// Annotates `scene` with `focused_id` as open `focus` metadata.
///
/// Focus is a metadata field on the scene node, never a new scene kind: the id
/// is stored as a bare symbol under [`FOCUS_KEY`], so a surface that does not
/// understand focus simply ignores it and the scene still validates. An
/// existing focus annotation is replaced.
pub fn with_focus(scene: Expr, focused_id: &str) -> Expr {
    sim_value::access::set(&scene, FOCUS_KEY, Expr::Symbol(Symbol::new(focused_id)))
}

/// Reads the focused node id annotated by [`with_focus`], if any.
pub fn focused_id(scene: &Expr) -> Option<Symbol> {
    sim_value::access::field_sym(scene, FOCUS_KEY)
}

/// Advances focus deterministically through `ids_in_order`, wrapping at the
/// ends, and returns the re-annotated scene.
///
/// The current focus is located in `ids_in_order` and moved one step in `dir`;
/// the order wraps, so [`FocusDir::Next`] past the last id lands on the first
/// and [`FocusDir::Prev`] past the first lands on the last. When the scene has
/// no focus yet (or its focus is not in the list), [`FocusDir::Next`] seeds the
/// first id and [`FocusDir::Prev`] the last. An empty `ids_in_order` leaves the
/// scene unchanged.
pub fn move_focus(scene: &Expr, ids_in_order: &[&str], dir: FocusDir) -> Expr {
    let len = ids_in_order.len();
    if len == 0 {
        return scene.clone();
    }
    let current = focused_id(scene);
    let here = current
        .as_ref()
        .and_then(|symbol| ids_in_order.iter().position(|id| *id == &*symbol.name));
    let next = match (here, dir) {
        (Some(index), FocusDir::Next) => (index + 1) % len,
        (Some(index), FocusDir::Prev) => (index + len - 1) % len,
        (None, FocusDir::Next) => 0,
        (None, FocusDir::Prev) => len - 1,
    };
    with_focus(scene.clone(), ids_in_order[next])
}

/// How a [`Command`] reduces to an [`Intent`](sim_lib_intent) when chosen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandKind {
    /// Reduces to an `intent/invoke` acting on the active pane.
    Invoke,
    /// Reduces to an `intent/ask` posing the command label as a question.
    Ask,
    /// Reduces to an `intent/open` opening the command id in the active pane.
    Open,
}

/// One palette command: a stable id, a human label, and the kind of Intent it
/// reduces to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    /// Stable command id (for example `run` or `open-source`).
    pub id: Symbol,
    /// Human-readable label shown in the palette and matched by the filter.
    pub label: String,
    /// The kind of Intent [`palette_intent`] produces for this command.
    pub kind: CommandKind,
}

/// Selects the commands whose label contains `filter`, case-insensitively,
/// preserving input order.
///
/// An empty `filter` selects every command. This is the one shared predicate
/// behind both [`palette_scene`] and any surface-side selection, so the TUI and
/// Web UI filter identically.
pub fn filter_commands<'a>(commands: &'a [Command], filter: &str) -> Vec<&'a Command> {
    let needle = filter.to_lowercase();
    commands
        .iter()
        .filter(|command| command.label.to_lowercase().contains(&needle))
        .collect()
}

/// Builds a `scene/overlay` listing the commands matching `filter`.
///
/// Order is deterministic (input order, filtered by [`filter_commands`]). Each
/// command becomes a `scene/button` carrying its label and a `command` field
/// with its id, so a surface can route a click or keypress back to the chosen
/// [`Command`]. The overlay carries its `role` and the active `filter` as
/// metadata.
pub fn palette_scene(commands: &[Command], filter: &str) -> Expr {
    let items = filter_commands(commands, filter)
        .into_iter()
        .map(|command| {
            node(
                "button",
                vec![
                    ("label", text(command.label.clone())),
                    ("command", Expr::Symbol(command.id.clone())),
                ],
            )
        })
        .collect();
    node(
        "overlay",
        vec![
            ("role", sym("command-palette")),
            ("filter", text(filter.to_owned())),
            ("children", list(items)),
        ],
    )
}

/// Reduces a chosen [`Command`] to its matching validated Intent for `pane` at
/// `tick`.
///
/// The required fields of each Intent kind are filled from the command and the
/// active pane: an [`CommandKind::Invoke`] becomes `intent/invoke`
/// (`target`/`op`/`args`); [`CommandKind::Ask`] becomes `intent/ask`
/// (`mission`/`question`); [`CommandKind::Open`] becomes `intent/open`
/// (`value`/`pane`). The result is re-checked with [`validate_intent`] before
/// it is returned, so a caller never sees a malformed Intent.
pub fn palette_intent(command: &Command, pane: &str, tick: u64) -> Result<Expr> {
    let origin = Origin::human(tick);
    let built = match command.kind {
        CommandKind::Invoke => intent(
            "invoke",
            origin,
            vec![
                ("target", text(pane.to_owned())),
                ("op", Expr::Symbol(command.id.clone())),
                ("args", list(Vec::new())),
            ],
        ),
        CommandKind::Ask => intent(
            "ask",
            origin,
            vec![
                ("mission", text(pane.to_owned())),
                ("question", text(command.label.clone())),
            ],
        ),
        CommandKind::Open => intent(
            "open",
            origin,
            vec![
                ("value", Expr::Symbol(command.id.clone())),
                ("pane", text(pane.to_owned())),
            ],
        ),
    };
    validate_intent(&built).map_err(|error| {
        Error::HostError(format!("palette produced an invalid intent: {error}"))
    })?;
    Ok(built)
}

/// An accessibility record carried as open metadata on a scene node.
///
/// The record names a semantic role, an accessible label and description, and
/// an urgency token. It is intentionally surface-neutral: a browser maps these
/// to ARIA attributes and a terminal to its own affordances, but neither
/// vocabulary is stored in the node. Round-trips through [`with_a11y`] /
/// [`a11y_of`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct A11y {
    /// Semantic role token (for example `button`, `alert`, `list`).
    pub role: String,
    /// Accessible label (the concise name of the node).
    pub label: String,
    /// Longer accessible description, or empty when there is none.
    pub description: String,
    /// Urgency token (for example `polite`, `assertive`, `off`).
    pub urgency: String,
}

/// Attaches an `a11y` metadata map (role/label/description/urgency) to `node`.
///
/// The four values are stored as plain strings under an open `a11y` field. No
/// ARIA or terminal name is copied into the node: a surface derives its own
/// affordances from this record. An existing `a11y` field is replaced. Read it
/// back with [`a11y_of`].
pub fn with_a11y(node: Expr, role: &str, label: &str, description: &str, urgency: &str) -> Expr {
    let record = map(vec![
        ("role", text(role.to_owned())),
        ("label", text(label.to_owned())),
        ("description", text(description.to_owned())),
        ("urgency", text(urgency.to_owned())),
    ]);
    sim_value::access::set(&node, A11Y_KEY, record)
}

/// Reads the accessibility record attached by [`with_a11y`], if present and
/// well-formed.
///
/// Returns `None` when the node carries no `a11y` map or the map is missing one
/// of the four string fields, so a partial record never reads back as valid.
pub fn a11y_of(node: &Expr) -> Option<A11y> {
    let record = sim_value::access::field(node, A11Y_KEY)?;
    Some(A11y {
        role: sim_value::access::field_str(record, "role")?.to_owned(),
        label: sim_value::access::field_str(record, "label")?.to_owned(),
        description: sim_value::access::field_str(record, "description")?.to_owned(),
        urgency: sim_value::access::field_str(record, "urgency")?.to_owned(),
    })
}

/// Presents a [`Draft`]'s diagnostics as a deterministic `scene/overlay`.
///
/// A committable draft (no diagnostics) yields an affirmative overlay carrying
/// an `ok` status and a single confirmation line. A rejected draft yields one
/// `scene/badge` per diagnostic, in order, each tagged with a severity token
/// and carrying its message; a diagnostic that has a machine-readable `code` is
/// anchored to it through a `code` field. The overlay's `status` is `rejected`
/// when any diagnostic is present.
pub fn diagnostics_scene(draft: &Draft) -> Expr {
    if draft.committable && draft.diagnostics.is_empty() {
        return node(
            "overlay",
            vec![
                ("role", sym("diagnostics")),
                ("status", sym("ok")),
                ("children", list(vec![text_line("no diagnostics")])),
            ],
        );
    }
    let lines = draft.diagnostics.iter().map(diagnostic_node).collect();
    node(
        "overlay",
        vec![
            ("role", sym("diagnostics")),
            ("status", sym("rejected")),
            ("children", list(lines)),
        ],
    )
}

/// Builds one diagnostic line: a `scene/badge` tagged with the severity token
/// and carrying the message, anchored to its `code` when one is present.
fn diagnostic_node(diagnostic: &Diagnostic) -> Expr {
    let mut entries = vec![
        ("status", sym(severity_token(diagnostic.severity))),
        ("label", text(diagnostic.message.clone())),
    ];
    if let Some(code) = &diagnostic.code {
        entries.push(("code", Expr::Symbol(code.clone())));
    }
    node("badge", entries)
}

/// A `scene/text` line used for the affirmative diagnostics overlay.
fn text_line(content: &str) -> Expr {
    node("text", vec![("text", text(content.to_owned()))])
}

/// The stable severity token a diagnostic badge carries.
fn severity_token(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
        Severity::Note => "note",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_lib_intent::intent_kind_of;
    use sim_lib_scene::{build::text_node, validate_scene};

    fn commands() -> Vec<Command> {
        vec![
            Command {
                id: Symbol::new("run"),
                label: "Run validation".to_owned(),
                kind: CommandKind::Invoke,
            },
            Command {
                id: Symbol::new("ask-status"),
                label: "Ask mission status".to_owned(),
                kind: CommandKind::Ask,
            },
            Command {
                id: Symbol::new("open-readme"),
                label: "Open README".to_owned(),
                kind: CommandKind::Open,
            },
        ]
    }

    #[test]
    fn focus_next_prev_wraps_deterministically() {
        let ids = ["a", "b", "c"];
        let scene = with_focus(text_node("x"), "a");
        assert_eq!(focused_id(&scene).unwrap().name.as_ref(), "a");

        let b = move_focus(&scene, &ids, FocusDir::Next);
        assert_eq!(focused_id(&b).unwrap().name.as_ref(), "b");
        let c = move_focus(&b, &ids, FocusDir::Next);
        let wrap = move_focus(&c, &ids, FocusDir::Next);
        assert_eq!(focused_id(&wrap).unwrap().name.as_ref(), "a", "next wraps");

        let prev_wrap = move_focus(&scene, &ids, FocusDir::Prev);
        assert_eq!(
            focused_id(&prev_wrap).unwrap().name.as_ref(),
            "c",
            "prev from first wraps to last"
        );

        // Determinism: same input, same output.
        assert_eq!(move_focus(&scene, &ids, FocusDir::Next), b);
    }

    #[test]
    fn move_focus_seeds_and_tolerates_empty() {
        let ids = ["a", "b"];
        let bare = text_node("x");
        assert_eq!(
            focused_id(&move_focus(&bare, &ids, FocusDir::Next))
                .unwrap()
                .name
                .as_ref(),
            "a"
        );
        assert_eq!(
            focused_id(&move_focus(&bare, &ids, FocusDir::Prev))
                .unwrap()
                .name
                .as_ref(),
            "b"
        );
        // Empty id list leaves the scene untouched.
        assert_eq!(move_focus(&bare, &[], FocusDir::Next), bare);
    }

    #[test]
    fn palette_filters_and_orders_deterministically() {
        let commands = commands();
        let scene = palette_scene(&commands, "");
        validate_scene(&scene).expect("palette overlay validates");
        assert_eq!(
            button_labels(&scene),
            commands.iter().map(|c| c.label.clone()).collect::<Vec<_>>()
        );

        // Case-insensitive substring filter, order preserved.
        let filtered = palette_scene(&commands, "OPEN");
        assert_eq!(button_labels(&filtered), vec!["Open README".to_owned()]);

        let many = palette_scene(&commands, "i");
        assert_eq!(
            button_labels(&many),
            vec!["Run validation".to_owned(), "Ask mission status".to_owned()],
            "'i' matches 'Run validation' and 'Ask mission status' in order"
        );

        // Deterministic.
        assert_eq!(palette_scene(&commands, "i"), many);
    }

    #[test]
    fn every_command_intent_validates() {
        for command in commands() {
            let produced = palette_intent(&command, "main", 9).expect("command reduces");
            validate_intent(&produced).expect("produced intent validates");
            let kind = intent_kind_of(&produced).unwrap();
            let expected = match command.kind {
                CommandKind::Invoke => "invoke",
                CommandKind::Ask => "ask",
                CommandKind::Open => "open",
            };
            assert_eq!(kind.name.as_ref(), expected);
        }
    }

    #[test]
    fn a11y_round_trips() {
        let node = with_a11y(
            text_node("Run"),
            "button",
            "Run validation",
            "Runs the mission validation suite",
            "polite",
        );
        validate_scene(&node).expect("a11y-annotated node validates");
        let back = a11y_of(&node).expect("a11y reads back");
        assert_eq!(
            back,
            A11y {
                role: "button".to_owned(),
                label: "Run validation".to_owned(),
                description: "Runs the mission validation suite".to_owned(),
                urgency: "polite".to_owned(),
            }
        );
        // A node without an a11y field reads back as None.
        assert!(a11y_of(&text_node("plain")).is_none());
    }

    #[test]
    fn diagnostics_scene_renders_rejected_messages() {
        let base = Expr::String("x".to_owned());
        let mut draft = Draft::rejected(base.clone(), Diagnostic::error("name is required"));
        draft
            .diagnostics
            .push(Diagnostic::info("value will be truncated"));
        let scene = diagnostics_scene(&draft);
        validate_scene(&scene).expect("diagnostics overlay validates");
        let labels = badge_labels(&scene);
        assert_eq!(
            labels,
            vec![
                "name is required".to_owned(),
                "value will be truncated".to_owned()
            ],
            "diagnostics render in order"
        );
        assert_eq!(overlay_status(&scene), Some("rejected".to_owned()));

        // A committable draft yields an affirmative overlay.
        let clean = Draft::clean(base.clone(), base);
        let ok = diagnostics_scene(&clean);
        validate_scene(&ok).expect("affirmative overlay validates");
        assert_eq!(overlay_status(&ok), Some("ok".to_owned()));
    }

    fn children(scene: &Expr) -> Vec<Expr> {
        match sim_value::access::field(scene, "children") {
            Some(Expr::List(items)) => items.clone(),
            _ => Vec::new(),
        }
    }

    fn button_labels(scene: &Expr) -> Vec<String> {
        children(scene)
            .iter()
            .filter_map(|child| sim_value::access::field_str(child, "label").map(str::to_owned))
            .collect()
    }

    fn badge_labels(scene: &Expr) -> Vec<String> {
        children(scene)
            .iter()
            .filter_map(|child| sim_value::access::field_str(child, "label").map(str::to_owned))
            .collect()
    }

    fn overlay_status(scene: &Expr) -> Option<String> {
        sim_value::access::field_sym(scene, "status").map(|symbol| symbol.name.to_string())
    }
}
