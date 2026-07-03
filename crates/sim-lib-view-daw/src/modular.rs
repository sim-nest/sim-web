//! Modular component builder scenes for synth, patch, and poly workflows.

use sim_kernel::{Expr, Symbol};
use sim_lib_scene::node;
use sim_value::build::{int, list, map, sym, text, vector};

use crate::modular_fixture::{
    fixture_palette, fixture_patch, fixture_sections, invalid_patch_validations,
};
use crate::poly::{POLY_SECTION_VIEW_ID, poly_section_view};

/// Stable lens id for the modular component builder.
pub const COMPONENT_BUILDER_VIEW_ID: &str = "view:component-builder";
/// Stable lens id for the component palette panel.
pub const COMPONENT_PALETTE_VIEW_ID: &str = "view:component-palette";
/// Stable lens id for the component graph panel.
pub const COMPONENT_GRAPH_VIEW_ID: &str = "view:component-graph";
/// Stable lens id for the component cord editor.
pub const COMPONENT_CORD_VIEW_ID: &str = "view:component-cord-editor";
/// Patch serialization format tag emitted by the builder.
pub const COMPONENT_BUILDER_PATCH_FORMAT: &str = "component-builder-patch-v1";

/// Action names the component builder accepts.
pub const COMPONENT_BUILDER_ACTIONS: [&str; 14] = [
    "connect",
    "disconnect",
    "add-module",
    "duplicate",
    "delete",
    "bypass",
    "reset",
    "inspect",
    "route-matrix",
    "enable-section",
    "disable-section",
    "save",
    "load",
    "live-preview",
];

/// Validation codes the component builder can report.
pub const COMPONENT_BUILDER_VALIDATION_CODES: [&str; 6] = [
    "missing-input",
    "cycle",
    "feedback-delay",
    "clock-mismatch",
    "rate-mismatch",
    "gate-s-trigger-mismatch",
];

/// Fixture name for a graph-edit builder snapshot.
pub const COMPONENT_BUILDER_GRAPH_EDIT_FIXTURE: &str = "graph-edit";
/// Fixture name for a cord-edit builder snapshot.
pub const COMPONENT_BUILDER_CORD_EDIT_FIXTURE: &str = "cord-edit";
/// Fixture name for a section-edit builder snapshot.
pub const COMPONENT_BUILDER_SECTION_EDIT_FIXTURE: &str = "section-edit";
/// Fixture name for an invalid-patch builder snapshot.
pub const COMPONENT_BUILDER_INVALID_PATCH_FIXTURE: &str = "invalid-patch";

/// A single validation finding reported by the component builder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuilderValidation {
    /// Qualified validation code symbol.
    pub code: Symbol,
    /// Module or cord the finding applies to, if any.
    pub target: Option<Symbol>,
    /// Human-readable description of the finding.
    pub message: String,
}

impl BuilderValidation {
    /// Builds a validation finding from a code, optional target, and message.
    pub fn new(code: &str, target: Option<Symbol>, message: impl Into<String>) -> Self {
        Self {
            code: Symbol::qualified("component-builder/validation", code),
            target,
            message: message.into(),
        }
    }
}

/// Returns the names of the built-in component builder fixtures.
pub fn component_builder_fixture_names() -> [&'static str; 4] {
    [
        COMPONENT_BUILDER_GRAPH_EDIT_FIXTURE,
        COMPONENT_BUILDER_CORD_EDIT_FIXTURE,
        COMPONENT_BUILDER_SECTION_EDIT_FIXTURE,
        COMPONENT_BUILDER_INVALID_PATCH_FIXTURE,
    ]
}

/// Renders the named fixture into a component builder Scene snapshot.
pub fn component_builder_snapshot(name: &str) -> Option<Expr> {
    let palette = fixture_palette();
    let patch = fixture_patch();
    let sections = fixture_sections();
    let validation = match name {
        COMPONENT_BUILDER_GRAPH_EDIT_FIXTURE => Vec::new(),
        COMPONENT_BUILDER_CORD_EDIT_FIXTURE => Vec::new(),
        COMPONENT_BUILDER_SECTION_EDIT_FIXTURE => Vec::new(),
        COMPONENT_BUILDER_INVALID_PATCH_FIXTURE => invalid_patch_validations(),
        _ => return None,
    };
    Some(component_builder_view(
        &patch,
        &palette,
        &sections,
        &validation,
    ))
}

/// Builds the full component builder Scene from a patch, palette, sections,
/// and validation findings.
pub fn component_builder_view(
    patch: &Expr,
    palette: &Expr,
    sections: &Expr,
    validation: &[BuilderValidation],
) -> Expr {
    let category_filter = field_symbol_named(palette, "category-filter");
    let capability_filter = field_symbol_named(palette, "capability-filter");
    let stable_ids = stable_component_ids(patch);
    node(
        "stack",
        vec![
            ("lens", sym(COMPONENT_BUILDER_VIEW_ID)),
            ("role", sym("component-builder")),
            (
                "patch-format",
                text(COMPONENT_BUILDER_PATCH_FORMAT.to_owned()),
            ),
            ("view-ids", builder_view_ids()),
            ("action-names", vector(action_names())),
            ("stable-component-ids", vector(stable_ids.clone())),
            (
                "children",
                list(vec![
                    action_toolbar(),
                    component_palette_view(
                        palette,
                        category_filter.as_ref(),
                        capability_filter.as_ref(),
                    ),
                    component_graph_view(patch),
                    component_cord_view(patch),
                    poly_section_view(sections),
                    validation_display(validation),
                    persistence_view(&stable_ids),
                ]),
            ),
        ],
    )
}

/// Builds the component palette table Scene, filtered by category and
/// capability.
pub fn component_palette_view(
    inventory: &Expr,
    category_filter: Option<&Symbol>,
    capability_filter: Option<&Symbol>,
) -> Expr {
    let rows = palette_items(inventory)
        .into_iter()
        .filter(|item| item_matches_filters(item, category_filter, capability_filter))
        .map(|item| palette_row(&item))
        .collect();
    node(
        "table",
        vec![
            ("lens", sym(COMPONENT_PALETTE_VIEW_ID)),
            ("role", sym("component-palette")),
            (
                "category-filter",
                category_filter
                    .cloned()
                    .map(Expr::Symbol)
                    .unwrap_or(Expr::Nil),
            ),
            (
                "capability-filter",
                capability_filter
                    .cloned()
                    .map(Expr::Symbol)
                    .unwrap_or(Expr::Nil),
            ),
            ("rows", vector(rows)),
        ],
    )
}

/// Builds the component graph Scene of modules and cords from a patch.
pub fn component_graph_view(patch: &Expr) -> Expr {
    let graph_nodes = patch_modules(patch)
        .into_iter()
        .map(|module| {
            node(
                "node",
                vec![
                    ("id", module.id),
                    ("title", text(module.label)),
                    ("module-kind", module.kind),
                    ("inputs", vector(module.inputs)),
                    ("outputs", vector(module.outputs)),
                    (
                        "actions",
                        vector(vec![
                            sym("inspect"),
                            sym("duplicate"),
                            sym("bypass"),
                            sym("delete"),
                        ]),
                    ),
                ],
            )
        })
        .collect();
    let edges = patch_cords(patch)
        .into_iter()
        .map(|cord| {
            node(
                "edge",
                vec![
                    ("from", text(cord.from)),
                    ("to", text(cord.to)),
                    ("action", sym("disconnect")),
                ],
            )
        })
        .collect();
    node(
        "graph",
        vec![
            ("lens", sym(COMPONENT_GRAPH_VIEW_ID)),
            ("role", sym("component-graph")),
            (
                "patch-format",
                text(COMPONENT_BUILDER_PATCH_FORMAT.to_owned()),
            ),
            ("nodes", list(graph_nodes)),
            ("edges", list(edges)),
        ],
    )
}

/// Builds the cord editor table Scene from a patch.
pub fn component_cord_view(patch: &Expr) -> Expr {
    let rows = patch_cords(patch)
        .into_iter()
        .map(|cord| {
            map(vec![
                ("from", text(cord.from)),
                ("to", text(cord.to)),
                (
                    "actions",
                    vector(vec![sym("connect"), sym("disconnect"), sym("route-matrix")]),
                ),
            ])
        })
        .collect();
    node(
        "table",
        vec![
            ("lens", sym(COMPONENT_CORD_VIEW_ID)),
            ("role", sym("component-cord-editor")),
            ("rows", vector(rows)),
            (
                "actions",
                vector(vec![sym("connect"), sym("disconnect"), sym("route-matrix")]),
            ),
        ],
    )
}

/// Builds the validation display Scene from a set of builder findings.
pub fn validation_display(validation: &[BuilderValidation]) -> Expr {
    let rows = validation.iter().map(validation_row).collect();
    let children = validation
        .iter()
        .map(|entry| {
            node(
                "field",
                vec![
                    ("role", sym("builder-validation-entry")),
                    ("validation", Expr::Symbol(entry.code.clone())),
                    (
                        "target",
                        entry.target.clone().map(Expr::Symbol).unwrap_or(Expr::Nil),
                    ),
                    ("message", text(entry.message.clone())),
                ],
            )
        })
        .collect();
    node(
        "box",
        vec![
            ("role", sym("builder-validation")),
            ("rows", vector(rows)),
            ("children", list(children)),
        ],
    )
}

fn validation_row(entry: &BuilderValidation) -> Expr {
    map(vec![
        ("validation", Expr::Symbol(entry.code.clone())),
        (
            "target",
            entry.target.clone().map(Expr::Symbol).unwrap_or(Expr::Nil),
        ),
        ("message", text(entry.message.clone())),
    ])
}

fn builder_view_ids() -> Expr {
    map(vec![
        ("builder", sym(COMPONENT_BUILDER_VIEW_ID)),
        ("palette", sym(COMPONENT_PALETTE_VIEW_ID)),
        ("graph", sym(COMPONENT_GRAPH_VIEW_ID)),
        ("cord", sym(COMPONENT_CORD_VIEW_ID)),
        ("poly", sym(POLY_SECTION_VIEW_ID)),
    ])
}

fn action_toolbar() -> Expr {
    node(
        "stack",
        vec![
            ("role", sym("component-builder-actions")),
            ("dir", sym("row")),
            (
                "children",
                list(
                    COMPONENT_BUILDER_ACTIONS
                        .iter()
                        .map(|action| action_button(action))
                        .collect(),
                ),
            ),
        ],
    )
}

fn action_button(action: &str) -> Expr {
    node(
        "button",
        vec![
            ("role", sym("builder-action")),
            ("action", sym(action)),
            ("label", text(action.replace('-', " "))),
        ],
    )
}

fn persistence_view(stable_ids: &[Expr]) -> Expr {
    node(
        "box",
        vec![
            ("role", sym("builder-persistence")),
            ("stable-component-ids", vector(stable_ids.to_vec())),
            (
                "children",
                list(vec![
                    action_button("save"),
                    action_button("load"),
                    action_button("live-preview"),
                ]),
            ),
        ],
    )
}

fn palette_row(item: &Expr) -> Expr {
    let id = field_named(item, "id")
        .cloned()
        .unwrap_or_else(|| sym("component"));
    let label = field_str_named(item, "label")
        .map(text)
        .unwrap_or_else(|| text(expr_label(&id)));
    map(vec![
        ("id", id),
        ("label", label),
        (
            "category",
            field_named(item, "category")
                .cloned()
                .unwrap_or_else(|| sym("unknown")),
        ),
        ("capabilities", vector(item_capabilities(item))),
        (
            "implemented",
            field_named(item, "implemented")
                .cloned()
                .unwrap_or(Expr::Bool(false)),
        ),
        ("actions", vector(vec![sym("add-module"), sym("inspect")])),
    ])
}

fn item_matches_filters(
    item: &Expr,
    category_filter: Option<&Symbol>,
    capability_filter: Option<&Symbol>,
) -> bool {
    let category_matches = match category_filter {
        Some(filter) => field_named(item, "category")
            .map(|value| expr_matches_symbol(value, filter))
            .unwrap_or(false),
        None => true,
    };
    let capability_matches = match capability_filter {
        Some(filter) => item_capabilities(item)
            .iter()
            .any(|value| expr_matches_symbol(value, filter)),
        None => true,
    };
    category_matches && capability_matches
}

fn palette_items(inventory: &Expr) -> Vec<Expr> {
    field_named(inventory, "items")
        .map(sequence)
        .unwrap_or_else(|| sequence(inventory))
}

fn patch_modules(patch: &Expr) -> Vec<ModuleRecord> {
    if let Some(modules) = field_named(patch, "modules") {
        return sequence(modules)
            .into_iter()
            .enumerate()
            .map(|(index, module)| module_record(&module, index))
            .collect();
    }
    field_named(patch, "nodes")
        .map(sequence)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, module)| audio_graph_node_record(&module, index))
        .collect()
}

fn module_record(module: &Expr, index: usize) -> ModuleRecord {
    let id = field_named(module, "id")
        .cloned()
        .unwrap_or_else(|| Expr::Symbol(Symbol::new(format!("module-{index}"))));
    let kind = field_named(module, "module-kind")
        .or_else(|| field_named(module, "kind"))
        .cloned()
        .unwrap_or_else(|| sym("component"));
    ModuleRecord {
        label: expr_label(&id),
        id,
        kind,
        inputs: jack_names(field_named(module, "inputs")),
        outputs: jack_names(field_named(module, "outputs")),
    }
}

fn audio_graph_node_record(module: &Expr, index: usize) -> ModuleRecord {
    let id = field_named(module, "id")
        .cloned()
        .unwrap_or_else(|| Expr::String(format!("node-{index}")));
    ModuleRecord {
        label: expr_label(&id),
        id,
        kind: sym("audio-graph-node"),
        inputs: vec![
            field_named(module, "in-channels")
                .cloned()
                .unwrap_or_else(|| int(0)),
        ],
        outputs: vec![
            field_named(module, "out-channels")
                .cloned()
                .unwrap_or_else(|| int(0)),
        ],
    }
}

fn patch_cords(patch: &Expr) -> Vec<CordRecord> {
    if let Some(cords) = field_named(patch, "cords") {
        return sequence(cords)
            .into_iter()
            .map(|cord| CordRecord {
                from: endpoint_label(field_named(&cord, "from")),
                to: endpoint_label(field_named(&cord, "to")),
            })
            .collect();
    }
    field_named(patch, "cables")
        .map(sequence)
        .unwrap_or_default()
        .into_iter()
        .map(|cable| CordRecord {
            from: field_named(&cable, "from")
                .map(expr_label)
                .unwrap_or_else(|| "out".to_owned()),
            to: field_named(&cable, "to")
                .map(expr_label)
                .unwrap_or_else(|| "in".to_owned()),
        })
        .collect()
}

fn stable_component_ids(patch: &Expr) -> Vec<Expr> {
    patch_modules(patch)
        .into_iter()
        .map(|module| module.id)
        .collect()
}

fn endpoint_label(endpoint: Option<&Expr>) -> String {
    let Some(endpoint) = endpoint else {
        return "unpatched".to_owned();
    };
    match endpoint {
        Expr::Map(_) => {
            let module = field_named(endpoint, "module")
                .map(expr_label)
                .unwrap_or_else(|| "module".to_owned());
            let jack = field_named(endpoint, "jack")
                .map(expr_label)
                .unwrap_or_else(|| "jack".to_owned());
            format!("{module}:{jack}")
        }
        other => expr_label(other),
    }
}

fn jack_names(jacks: Option<&Expr>) -> Vec<Expr> {
    jacks
        .map(sequence)
        .unwrap_or_default()
        .into_iter()
        .map(|jack| {
            field_named(&jack, "name")
                .cloned()
                .unwrap_or_else(|| sym("jack"))
        })
        .collect()
}

fn sequence(value: &Expr) -> Vec<Expr> {
    match value {
        Expr::List(items) | Expr::Vector(items) => items.clone(),
        Expr::Nil => Vec::new(),
        other => vec![other.clone()],
    }
}

fn item_capabilities(item: &Expr) -> Vec<Expr> {
    field_named(item, "capabilities")
        .map(sequence)
        .unwrap_or_default()
}

fn field_named<'a>(expr: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = expr else {
        return None;
    };
    entries
        .iter()
        .find_map(|(key, value)| (key_name(key) == Some(name)).then_some(value))
}

fn field_symbol_named(expr: &Expr, name: &str) -> Option<Symbol> {
    match field_named(expr, name) {
        Some(Expr::Symbol(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

fn field_str_named<'a>(expr: &'a Expr, name: &str) -> Option<&'a str> {
    match field_named(expr, name) {
        Some(Expr::String(text)) => Some(text),
        _ => None,
    }
}

fn key_name(key: &Expr) -> Option<&str> {
    match key {
        Expr::Symbol(symbol) => Some(symbol.name.as_ref()),
        Expr::String(text) => Some(text),
        _ => None,
    }
}

fn expr_matches_symbol(value: &Expr, filter: &Symbol) -> bool {
    match value {
        Expr::Symbol(symbol) => {
            symbol == filter || symbol.as_qualified_str() == filter.as_qualified_str()
        }
        Expr::String(text) => text == &filter.as_qualified_str() || text == filter.name.as_ref(),
        _ => false,
    }
}

fn expr_label(expr: &Expr) -> String {
    match expr {
        Expr::Symbol(symbol) => symbol.as_qualified_str(),
        Expr::String(text) => text.clone(),
        other => format!("{other:?}"),
    }
}

fn action_names() -> Vec<Expr> {
    COMPONENT_BUILDER_ACTIONS
        .iter()
        .map(|action| sym(action))
        .collect()
}

struct ModuleRecord {
    id: Expr,
    kind: Expr,
    label: String,
    inputs: Vec<Expr>,
    outputs: Vec<Expr>,
}

struct CordRecord {
    from: String,
    to: String,
}
