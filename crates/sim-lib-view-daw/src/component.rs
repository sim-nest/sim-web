//! Generic component editor scenes.

use sim_kernel::{Expr, NumberLiteral, Symbol};
use sim_lib_scene::node;
use sim_value::{
    access::{field, field_str, field_sym},
    build::{float, int, list, map, sym, text, vector},
};

/// Stable lens id for the generic component editor.
pub const COMPONENT_EDITOR_VIEW_ID: &str = "view:component-editor";
/// Fixture name for a component descriptor carrying many parameters.
pub const COMPONENT_EDITOR_MANY_PARAM_FIXTURE: &str = "many-param";
/// Fixture name for a component descriptor with no parameters.
pub const COMPONENT_EDITOR_NO_PARAM_FIXTURE: &str = "no-param";
/// Fixture name for a component descriptor holding an invalid value.
pub const COMPONENT_EDITOR_INVALID_VALUE_FIXTURE: &str = "invalid-value";
/// Fixture name for a component descriptor that only carries a trace.
pub const COMPONENT_EDITOR_TRACE_ONLY_FIXTURE: &str = "trace-only";

/// Returns the names of the built-in component editor fixtures.
pub fn component_editor_fixture_names() -> [&'static str; 4] {
    [
        COMPONENT_EDITOR_MANY_PARAM_FIXTURE,
        COMPONENT_EDITOR_NO_PARAM_FIXTURE,
        COMPONENT_EDITOR_INVALID_VALUE_FIXTURE,
        COMPONENT_EDITOR_TRACE_ONLY_FIXTURE,
    ]
}

/// Builds the component editor Scene from a component descriptor.
pub fn component_editor_view(descriptor: &Expr) -> Expr {
    let mut children = vec![
        summary_view(descriptor),
        ports_view(descriptor),
        parameter_groups_view(descriptor),
    ];
    if let Some(errors) = validation_errors_view(descriptor) {
        children.push(errors);
    }
    if let Some(trace) = trace_view(descriptor) {
        children.push(trace);
    }
    if let Some(route) = specialized_route_view(descriptor) {
        children.push(route);
    }
    node(
        "stack",
        vec![
            ("lens", sym(COMPONENT_EDITOR_VIEW_ID)),
            ("role", sym("component-editor")),
            ("component", component_expr(descriptor)),
            ("name", text(component_name(descriptor))),
            ("category", category_expr(descriptor)),
            ("trace-available", Expr::Bool(trace_available(descriptor))),
            ("children", list(children)),
        ],
    )
}

/// Returns the descriptor for the named editor fixture, if it exists.
pub fn component_editor_fixture(name: &str) -> Option<Expr> {
    match name {
        COMPONENT_EDITOR_MANY_PARAM_FIXTURE => Some(many_param_descriptor()),
        COMPONENT_EDITOR_NO_PARAM_FIXTURE => Some(no_param_descriptor()),
        COMPONENT_EDITOR_INVALID_VALUE_FIXTURE => Some(invalid_value_descriptor()),
        COMPONENT_EDITOR_TRACE_ONLY_FIXTURE => Some(trace_only_descriptor()),
        _ => None,
    }
}

/// Renders the named fixture into a component editor Scene snapshot.
pub fn component_editor_snapshot(name: &str) -> Option<Expr> {
    component_editor_fixture(name).map(|descriptor| component_editor_view(&descriptor))
}

fn summary_view(descriptor: &Expr) -> Expr {
    node(
        "box",
        vec![
            ("role", sym("component-summary")),
            ("component", component_expr(descriptor)),
            ("name", text(component_name(descriptor))),
            ("category", category_expr(descriptor)),
            ("trace-available", Expr::Bool(trace_available(descriptor))),
            (
                "children",
                list(vec![node(
                    "badge",
                    vec![
                        (
                            "status",
                            sym(if trace_available(descriptor) {
                                "trace"
                            } else {
                                "plain"
                            }),
                        ),
                        ("label", text(component_name(descriptor))),
                    ],
                )]),
            ),
        ],
    )
}

fn ports_view(descriptor: &Expr) -> Expr {
    let rows = sequence(field(descriptor, "ports"))
        .iter()
        .map(|port| {
            map(vec![
                (
                    "id",
                    field(port, "id").cloned().unwrap_or_else(|| sym("port")),
                ),
                (
                    "media",
                    field(port, "media")
                        .cloned()
                        .unwrap_or_else(|| sym("unknown")),
                ),
                (
                    "direction",
                    field(port, "direction")
                        .cloned()
                        .unwrap_or_else(|| sym("unknown")),
                ),
                (
                    "channels",
                    field(port, "channels").cloned().unwrap_or_else(|| int(1)),
                ),
                (
                    "required",
                    field(port, "required").cloned().unwrap_or(Expr::Bool(true)),
                ),
            ])
        })
        .collect();
    node(
        "table",
        vec![("role", sym("component-ports")), ("rows", vector(rows))],
    )
}

fn parameter_groups_view(descriptor: &Expr) -> Expr {
    let groups = sequence(field(descriptor, "parameter-groups"));
    if groups.is_empty() {
        return node(
            "box",
            vec![
                ("role", sym("parameter-empty")),
                (
                    "children",
                    list(vec![node("text", vec![("text", text("No parameters"))])]),
                ),
            ],
        );
    }
    let children = groups
        .iter()
        .map(|group| parameter_group_view(group, descriptor))
        .collect();
    node(
        "stack",
        vec![
            ("role", sym("parameter-groups")),
            ("dir", sym("column")),
            ("children", list(children)),
        ],
    )
}

fn parameter_group_view(group: &Expr, descriptor: &Expr) -> Expr {
    let params = sequence(field(group, "params"))
        .iter()
        .map(|param| parameter_editor_view(param, descriptor))
        .collect();
    node(
        "box",
        vec![
            ("role", sym("parameter-group")),
            (
                "group",
                field(group, "name").cloned().unwrap_or_else(|| sym("main")),
            ),
            (
                "label",
                text(field_str(group, "label").unwrap_or("Parameters")),
            ),
            ("children", list(params)),
        ],
    )
}

fn parameter_editor_view(param: &Expr, descriptor: &Expr) -> Expr {
    let editor = editor_symbol(param);
    let id = field(param, "id")
        .cloned()
        .unwrap_or_else(|| Expr::Symbol(Symbol::new("param")));
    node(
        "field",
        vec![
            ("role", sym("component-param-editor")),
            ("editor", Expr::Symbol(editor.clone())),
            ("param", id.clone()),
            (
                "label",
                text(field_str(param, "label").unwrap_or("Parameter")),
            ),
            (
                "unit",
                field(param, "unit")
                    .cloned()
                    .unwrap_or_else(|| sym("unitless")),
            ),
            ("value", value_for_param(param, descriptor, &id)),
            (
                "normalized-value",
                field(param, "normalized-value")
                    .or_else(|| field(param, "normalized-default"))
                    .cloned()
                    .unwrap_or(Expr::Nil),
            ),
            ("range", field(param, "range").cloned().unwrap_or(Expr::Nil)),
            (
                "enum-values",
                field(param, "enum-values")
                    .cloned()
                    .unwrap_or_else(|| vector(Vec::new())),
            ),
            ("read-only", Expr::Bool(editor_is_readonly(&editor, param))),
            ("errors", vector(errors_for_param(descriptor, &id))),
        ],
    )
}

fn validation_errors_view(descriptor: &Expr) -> Option<Expr> {
    let errors = sequence(field(descriptor, "validation-errors"));
    if errors.is_empty() {
        return None;
    }
    Some(node(
        "box",
        vec![
            ("role", sym("validation-errors")),
            (
                "children",
                list(
                    errors
                        .iter()
                        .map(|error| {
                            node(
                                "field",
                                vec![
                                    ("role", sym("param-error")),
                                    (
                                        "param",
                                        field(error, "param")
                                            .cloned()
                                            .unwrap_or_else(|| sym("component")),
                                    ),
                                    (
                                        "message",
                                        text(
                                            field_str(error, "message").unwrap_or("invalid value"),
                                        ),
                                    ),
                                ],
                            )
                        })
                        .collect(),
                ),
            ),
        ],
    ))
}

fn trace_view(descriptor: &Expr) -> Option<Expr> {
    let fields = sequence(field(descriptor, "trace-fields"));
    if fields.is_empty() && !trace_available(descriptor) {
        return None;
    }
    let children = if fields.is_empty() {
        vec![node(
            "badge",
            vec![("status", sym("trace")), ("label", text("Trace available"))],
        )]
    } else {
        fields
            .iter()
            .map(|field_expr| trace_field_view(field_expr))
            .collect()
    };
    Some(node(
        "box",
        vec![
            ("role", sym("trace-fields")),
            ("trace-available", Expr::Bool(trace_available(descriptor))),
            ("children", list(children)),
        ],
    ))
}

fn trace_field_view(field_expr: &Expr) -> Expr {
    node(
        "field",
        vec![
            ("role", sym("component-trace-field")),
            (
                "editor",
                field(field_expr, "editor").cloned().unwrap_or_else(|| {
                    Expr::Symbol(Symbol::qualified(
                        "component-editor/editor",
                        "trace-readonly",
                    ))
                }),
            ),
            (
                "param",
                field(field_expr, "id")
                    .cloned()
                    .unwrap_or_else(|| sym("trace")),
            ),
            (
                "label",
                text(field_str(field_expr, "label").unwrap_or("Trace")),
            ),
            (
                "value",
                field(field_expr, "value").cloned().unwrap_or(Expr::Nil),
            ),
            ("read-only", Expr::Bool(true)),
        ],
    )
}

fn specialized_route_view(descriptor: &Expr) -> Option<Expr> {
    let target = field_sym(descriptor, "specialized-view")?;
    Some(node(
        "button",
        vec![
            ("role", sym("specialized-route")),
            ("action", sym("route-specialized-view")),
            ("target", Expr::Symbol(target)),
            ("label", text("Open specialized view")),
        ],
    ))
}

fn component_expr(descriptor: &Expr) -> Expr {
    field(descriptor, "component")
        .cloned()
        .unwrap_or_else(|| Expr::Symbol(Symbol::new("component")))
}

fn component_name(descriptor: &Expr) -> String {
    field_str(descriptor, "name")
        .map(str::to_owned)
        .unwrap_or_else(|| symbol_text(&component_expr(descriptor)))
}

fn category_expr(descriptor: &Expr) -> Expr {
    field(descriptor, "category")
        .cloned()
        .unwrap_or_else(|| Expr::Symbol(Symbol::new("generic")))
}

fn trace_available(descriptor: &Expr) -> bool {
    matches!(field(descriptor, "trace-available"), Some(Expr::Bool(true)))
}

fn value_for_param(param: &Expr, descriptor: &Expr, id: &Expr) -> Expr {
    field(descriptor, "current-values")
        .and_then(|values| value_by_key(values, id))
        .cloned()
        .or_else(|| field(param, "value").cloned())
        .or_else(|| field(param, "raw-default").cloned())
        .or_else(|| field(param, "normalized-default").cloned())
        .unwrap_or(Expr::Nil)
}

fn value_by_key<'a>(map_expr: &'a Expr, needle: &Expr) -> Option<&'a Expr> {
    let Expr::Map(entries) = map_expr else {
        return None;
    };
    entries
        .iter()
        .find_map(|(key, value)| (key == needle).then_some(value))
}

fn errors_for_param(descriptor: &Expr, id: &Expr) -> Vec<Expr> {
    sequence(field(descriptor, "validation-errors"))
        .into_iter()
        .filter(|error| field(error, "param") == Some(id))
        .cloned()
        .collect()
}

fn editor_symbol(param: &Expr) -> Symbol {
    if let Some(symbol) = field_sym(param, "editor") {
        return symbol;
    }
    if !sequence(field(param, "enum-values")).is_empty() {
        return Symbol::qualified("component-editor/editor", "enum");
    }
    if matches!(field(param, "read-only"), Some(Expr::Bool(true))) {
        return Symbol::qualified("component-editor/editor", "fixed-point");
    }
    Symbol::qualified("component-editor/editor", "normalized")
}

fn editor_is_readonly(editor: &Symbol, param: &Expr) -> bool {
    editor.name.as_ref() == "fixed-point"
        || editor.name.as_ref() == "trace-readonly"
        || matches!(field(param, "read-only"), Some(Expr::Bool(true)))
}

fn sequence(value: Option<&Expr>) -> Vec<&Expr> {
    match value {
        Some(Expr::List(items)) | Some(Expr::Vector(items)) => items.iter().collect(),
        _ => Vec::new(),
    }
}

fn symbol_text(expr: &Expr) -> String {
    match expr {
        Expr::Symbol(symbol) => symbol.to_string(),
        Expr::String(text) => text.clone(),
        _ => "component".to_owned(),
    }
}

fn many_param_descriptor() -> Expr {
    descriptor(
        ("component-fixture/many-param", "Many Param Component"),
        vec![
            port("input", "audio-rate", "input"),
            port("output", "audio-rate", "output"),
        ],
        vec![group(
            "main",
            vec![
                param(
                    "gain-steps",
                    "Gain Steps",
                    "integer-range",
                    number("i64", "64"),
                    Some(range(0.0, 127.0, 64.0)),
                    Vec::new(),
                    false,
                ),
                param(
                    "waveform",
                    "Waveform",
                    "enum",
                    Expr::Symbol(Symbol::qualified("component-fixture/waveform", "sine")),
                    None,
                    vec!["sine", "square"],
                    false,
                ),
                param(
                    "bypass",
                    "Bypass",
                    "toggle",
                    Expr::Bool(false),
                    None,
                    Vec::new(),
                    false,
                ),
                param(
                    "cutoff-readout",
                    "Cutoff Readout",
                    "fixed-point",
                    text("440.00 Hz"),
                    None,
                    Vec::new(),
                    true,
                ),
                param(
                    "mix",
                    "Mix",
                    "normalized",
                    float(0.75),
                    Some(range(0.0, 1.0, 0.75)),
                    Vec::new(),
                    false,
                ),
            ],
        )],
        vector(Vec::new()),
        (
            true,
            vector(vec![trace_field(
                "last-output",
                "Last Output",
                text("0.25"),
            )]),
        ),
        Expr::Symbol(Symbol::qualified("view/component", "many-param")),
    )
}

fn no_param_descriptor() -> Expr {
    descriptor(
        ("component-fixture/no-param", "No Param Component"),
        vec![port("output", "metadata", "output")],
        Vec::new(),
        vector(Vec::new()),
        (false, vector(Vec::new())),
        Expr::Nil,
    )
}

fn invalid_value_descriptor() -> Expr {
    let id = Expr::Symbol(Symbol::qualified("component-fixture/param", "gain"));
    descriptor(
        ("component-fixture/invalid-value", "Invalid Value Component"),
        vec![port("output", "control-rate", "output")],
        vec![group(
            "main",
            vec![param(
                "gain",
                "Gain",
                "normalized",
                float(2.0),
                Some(range(0.0, 1.0, 0.5)),
                Vec::new(),
                false,
            )],
        )],
        vector(vec![map(vec![
            ("param", id),
            ("message", text("value must be between 0 and 1")),
        ])]),
        (false, vector(Vec::new())),
        Expr::Nil,
    )
}

fn trace_only_descriptor() -> Expr {
    descriptor(
        ("component-fixture/trace-only", "Trace Only Component"),
        vec![port("trace", "trace", "output")],
        Vec::new(),
        vector(Vec::new()),
        (
            true,
            vector(vec![
                trace_field("frame", "Frame", number("i64", "128")),
                trace_field("state", "State", text("held")),
            ]),
        ),
        Expr::Nil,
    )
}

fn descriptor(
    identity: (&str, &str),
    ports: Vec<Expr>,
    groups: Vec<Expr>,
    validation_errors: Expr,
    trace: (bool, Expr),
    specialized_view: Expr,
) -> Expr {
    map(vec![
        (
            "tag",
            Expr::Symbol(Symbol::qualified("component-editor", "descriptor")),
        ),
        ("component", Expr::Symbol(Symbol::new(identity.0))),
        ("name", text(identity.1)),
        (
            "category",
            Expr::Symbol(Symbol::qualified("component-editor/category", "fixture")),
        ),
        ("ports", vector(ports)),
        ("parameter-groups", vector(groups)),
        ("current-values", current_values_from_groups(&[])),
        ("validation-errors", validation_errors),
        ("trace-available", Expr::Bool(trace.0)),
        ("trace-fields", trace.1),
        ("specialized-view", specialized_view),
    ])
}

fn group(name: &str, params: Vec<Expr>) -> Expr {
    map(vec![
        (
            "name",
            Expr::Symbol(Symbol::qualified("component-fixture/group", name)),
        ),
        ("label", text("Parameters")),
        ("params", vector(params)),
    ])
}

fn port(id: &str, media: &str, direction: &str) -> Expr {
    map(vec![
        (
            "id",
            Expr::Symbol(Symbol::qualified("component-fixture/port", id)),
        ),
        (
            "media",
            Expr::Symbol(Symbol::qualified("audio-synth/port-media", media)),
        ),
        (
            "direction",
            Expr::Symbol(Symbol::qualified("audio-synth/port-direction", direction)),
        ),
        ("channels", int(1)),
        ("required", Expr::Bool(true)),
    ])
}

fn param(
    id: &str,
    label: &str,
    editor: &str,
    value: Expr,
    range: Option<Expr>,
    enum_values: Vec<&str>,
    read_only: bool,
) -> Expr {
    let id_expr = Expr::Symbol(Symbol::qualified("component-fixture/param", id));
    map(vec![
        ("id", id_expr),
        ("label", text(label)),
        (
            "unit",
            Expr::Symbol(Symbol::qualified("component-fixture/unit", "unitless")),
        ),
        (
            "editor",
            Expr::Symbol(Symbol::qualified("component-editor/editor", editor)),
        ),
        ("value", value),
        ("normalized-value", float(0.5)),
        ("range", range.unwrap_or(Expr::Nil)),
        (
            "enum-values",
            vector(
                enum_values
                    .into_iter()
                    .map(|name| Expr::Symbol(Symbol::qualified("component-fixture/enum", name)))
                    .collect(),
            ),
        ),
        ("read-only", Expr::Bool(read_only)),
    ])
}

fn range(min: f64, max: f64, default: f64) -> Expr {
    map(vec![
        ("min", float(min)),
        ("max", float(max)),
        ("default", float(default)),
    ])
}

fn trace_field(id: &str, label: &str, value: Expr) -> Expr {
    map(vec![
        (
            "id",
            Expr::Symbol(Symbol::qualified("component-fixture/trace", id)),
        ),
        ("label", text(label)),
        (
            "editor",
            Expr::Symbol(Symbol::qualified(
                "component-editor/editor",
                "trace-readonly",
            )),
        ),
        ("value", value),
        ("read-only", Expr::Bool(true)),
    ])
}

fn current_values_from_groups(_groups: &[Expr]) -> Expr {
    Expr::Map(Vec::new())
}

fn number(domain: &str, canonical: &str) -> Expr {
    Expr::Number(NumberLiteral {
        domain: Symbol::new(domain),
        canonical: canonical.to_owned(),
    })
}
