//! The command palette: the keyboard spine of the workspace.
//!
//! One entry point searches everything an operator can reach -- open resources,
//! registry exports, browse/help/test cards, lenses, commands, recent objects,
//! and saved workspaces -- and opens any result into the appropriate lens.
//! Opening a card targets its value through the dispatcher (the best lens),
//! never only the generic card renderer.

use sim_kernel::{Expr, Symbol};
use sim_lib_intent::{Origin, intent};
use sim_lib_scene::{node, sym};

use crate::value::panes;

/// What a palette entry points at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    /// A resource open in the workspace.
    Resource,
    /// A registry export (class, codec, shape, ...).
    Export,
    /// A browse/help/test card.
    Card,
    /// A lens.
    Lens,
    /// A runnable command.
    Command,
    /// A recently touched object.
    Recent,
    /// A saved workspace.
    Workspace,
}

impl EntryKind {
    fn token(self) -> &'static str {
        match self {
            EntryKind::Resource => "resource",
            EntryKind::Export => "export",
            EntryKind::Card => "card",
            EntryKind::Lens => "lens",
            EntryKind::Command => "command",
            EntryKind::Recent => "recent",
            EntryKind::Workspace => "workspace",
        }
    }
}

/// One searchable palette entry.
#[derive(Clone, Debug)]
pub struct PaletteEntry {
    /// Stable entry id.
    pub id: Symbol,
    /// Human-readable label, matched against the query.
    pub label: String,
    /// What kind of thing this entry is.
    pub kind: EntryKind,
    /// The target value (a resource ref, lens id, card, command, ...).
    pub target: Expr,
}

impl PaletteEntry {
    /// Build an entry.
    pub fn new(id: &str, label: &str, kind: EntryKind, target: Expr) -> Self {
        Self {
            id: Symbol::new(id),
            label: label.to_owned(),
            kind,
            target,
        }
    }
}

/// A searchable command palette.
#[derive(Default)]
pub struct Palette {
    entries: Vec<PaletteEntry>,
}

impl Palette {
    /// An empty palette.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an entry.
    pub fn add(&mut self, entry: PaletteEntry) {
        self.entries.push(entry);
    }

    /// Add a resource entry per open pane in a workspace.
    pub fn add_workspace_resources(&mut self, workspace: &Expr) {
        for pane in panes(workspace) {
            if let Some(Expr::Symbol(id)) = crate::value::get(&pane, "id") {
                let resource = crate::value::get(&pane, "resource")
                    .cloned()
                    .unwrap_or(Expr::Nil);
                self.add(PaletteEntry::new(
                    &id.name,
                    &id.name,
                    EntryKind::Resource,
                    resource,
                ));
            }
        }
    }

    /// All entries.
    pub fn entries(&self) -> &[PaletteEntry] {
        &self.entries
    }

    /// Search the palette, returning matching entries best first.
    pub fn search(&self, query: &str) -> Vec<&PaletteEntry> {
        let query = query.to_lowercase();
        let mut scored: Vec<(i32, usize, &PaletteEntry)> = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| {
                match_score(&query, &entry.label.to_lowercase()).map(|score| (score, index, entry))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, entry)| entry).collect()
    }

    /// Render the palette (query field plus the result list) as a Scene.
    pub fn scene(&self, query: &str) -> Expr {
        let results = self
            .search(query)
            .into_iter()
            .map(|entry| {
                node(
                    "button",
                    vec![
                        ("control", Expr::Symbol(entry.id.clone())),
                        (
                            "label",
                            Expr::String(format!("{}  [{}]", entry.label, entry.kind.token())),
                        ),
                        ("target", entry.target.clone()),
                    ],
                )
            })
            .collect();
        node(
            "box",
            vec![
                ("role", sym("palette")),
                (
                    "children",
                    Expr::List(vec![
                        node(
                            "field",
                            vec![
                                ("datatype", sym("search")),
                                ("value", Expr::String(query.to_owned())),
                            ],
                        ),
                        node(
                            "stack",
                            vec![("dir", sym("column")), ("children", Expr::List(results))],
                        ),
                    ]),
                ),
            ],
        )
    }

    /// Build the Intent that opens an entry into a pane: resources, cards, and
    /// recents open the value (the dispatcher picks the best lens); a lens entry
    /// switches the active lens; a command invokes.
    pub fn open_entry(&self, entry: &PaletteEntry, pane: &str) -> Expr {
        match entry.kind {
            EntryKind::Lens => intent(
                "set-lens",
                Origin::human(0),
                vec![("pane", sym(pane)), ("lens", entry.target.clone())],
            ),
            EntryKind::Command => intent(
                "invoke",
                Origin::human(0),
                vec![
                    ("target", entry.target.clone()),
                    ("op", Expr::Symbol(entry.id.clone())),
                    ("args", Expr::Map(Vec::new())),
                ],
            ),
            _ => intent(
                "open",
                Origin::human(0),
                vec![("value", entry.target.clone()), ("pane", sym(pane))],
            ),
        }
    }
}

/// The target value a browse/help/test card points at, if any.
pub fn card_target(card: &Expr) -> Option<Expr> {
    for name in ["target", "subject", "ref", "value"] {
        if let Some(value) = crate::value::get(card, name) {
            return Some(value.clone());
        }
    }
    None
}

/// Open a card into the best lens for its target: an `intent/open` of the card's
/// target, dispatched to the right lens rather than the generic card renderer.
pub fn open_card(card: &Expr, pane: &str) -> Option<Expr> {
    let target = card_target(card)?;
    Some(intent(
        "open",
        Origin::human(0),
        vec![("value", target), ("pane", sym(pane))],
    ))
}

/// Score a query against a label: higher is better, `None` excludes.
fn match_score(query: &str, label: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }
    if let Some(index) = label.find(query) {
        // Contiguous substring: best, earlier is better.
        return Some(1000 - index as i32);
    }
    // Subsequence fallback.
    if is_subsequence(query, label) {
        Some(100)
    } else {
        None
    }
}

fn is_subsequence(query: &str, label: &str) -> bool {
    let mut chars = label.chars();
    query.chars().all(|needle| chars.any(|c| c == needle))
}
