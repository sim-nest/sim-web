//! Scene value model: builders, accessors, and fail-closed validation.
//!
//! A Scene is a SIM value (an `Expr` tree) built from open maps tagged with a
//! `kind` symbol. This module never introduces a parallel data model; it only
//! provides ergonomic constructors over `Expr` and a validator that turns a
//! malformed scene into a structured [`SceneError`] (a path plus a message)
//! rather than a panic.

use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, Expr, NoopEvalPolicy, ShapeMatch, Symbol};

use crate::kinds::{KIND_KEY, is_known_kind};

/// One total budget for producing or rendering a Scene.
///
/// `nodes` and `depth` bound structural growth. `encoded_bytes` bounds the
/// whole scene value as encoded data. `face_bytes` bounds any single rendered
/// face such as a label, text run, title, or field value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneBudget {
    /// Maximum number of scene nodes.
    pub nodes: usize,
    /// Maximum nesting depth, with the root at depth 0.
    pub depth: usize,
    /// Maximum encoded bytes for the scene value.
    pub encoded_bytes: usize,
    /// Maximum bytes for one visible face.
    pub face_bytes: usize,
}

impl SceneBudget {
    /// Create a budget from explicit limits.
    pub const fn new(nodes: usize, depth: usize, encoded_bytes: usize, face_bytes: usize) -> Self {
        Self {
            nodes,
            depth,
            encoded_bytes,
            face_bytes,
        }
    }

    /// Default browser-safe budget for generic views.
    pub const fn interactive() -> Self {
        Self::new(512, 32, 256 * 1024, 8 * 1024)
    }

    /// Smaller budget used by tests and compact previews.
    pub const fn compact() -> Self {
        Self::new(64, 12, 32 * 1024, 1024)
    }
}

/// Mutable receipt for a [`SceneBudget`] as scene producers spend it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneBudgetState {
    budget: SceneBudget,
    nodes_used: usize,
    encoded_bytes_used: usize,
}

impl SceneBudgetState {
    /// Start spending `budget`.
    pub fn new(budget: SceneBudget) -> Self {
        Self {
            budget,
            nodes_used: 0,
            encoded_bytes_used: 0,
        }
    }

    /// The immutable limits this state enforces.
    pub fn budget(&self) -> &SceneBudget {
        &self.budget
    }

    /// Number of scene nodes admitted so far.
    pub fn nodes_used(&self) -> usize {
        self.nodes_used
    }

    /// Number of approximate encoded bytes admitted so far.
    pub fn encoded_bytes_used(&self) -> usize {
        self.encoded_bytes_used
    }

    /// Try to admit one node at `depth` with a visible face and encoded-size
    /// estimate. Returns a truncation reason when the budget is exhausted.
    pub fn admit(
        &mut self,
        depth: usize,
        face: Option<&str>,
        encoded_bytes: usize,
    ) -> Result<(), SceneBudgetExhausted> {
        if self.nodes_used >= self.budget.nodes {
            return Err(SceneBudgetExhausted::Nodes {
                limit: self.budget.nodes,
            });
        }
        if depth > self.budget.depth {
            return Err(SceneBudgetExhausted::Depth {
                limit: self.budget.depth,
            });
        }
        if let Some(face) = face
            && face.len() > self.budget.face_bytes
        {
            return Err(SceneBudgetExhausted::FaceBytes {
                limit: self.budget.face_bytes,
            });
        }
        if self.encoded_bytes_used.saturating_add(encoded_bytes) > self.budget.encoded_bytes {
            return Err(SceneBudgetExhausted::EncodedBytes {
                limit: self.budget.encoded_bytes,
            });
        }
        self.nodes_used += 1;
        self.encoded_bytes_used = self.encoded_bytes_used.saturating_add(encoded_bytes);
        Ok(())
    }
}

/// Reason a Scene budget refused another node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SceneBudgetExhausted {
    /// The node count was exhausted.
    Nodes {
        /// Configured node limit.
        limit: usize,
    },
    /// The depth limit was exhausted.
    Depth {
        /// Configured depth limit.
        limit: usize,
    },
    /// The total encoded-byte limit was exhausted.
    EncodedBytes {
        /// Configured encoded-byte limit.
        limit: usize,
    },
    /// A single visible face exceeded the per-face limit.
    FaceBytes {
        /// Configured per-face byte limit.
        limit: usize,
    },
}

impl SceneBudgetExhausted {
    /// Stable reason token for scene truncation metadata.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Nodes { .. } => "nodes",
            Self::Depth { .. } => "depth",
            Self::EncodedBytes { .. } => "encoded-bytes",
            Self::FaceBytes { .. } => "face-bytes",
        }
    }

    /// Configured limit that was exceeded.
    pub fn limit(&self) -> usize {
        match self {
            Self::Nodes { limit }
            | Self::Depth { limit }
            | Self::EncodedBytes { limit }
            | Self::FaceBytes { limit } => *limit,
        }
    }
}

/// A structured scene validation diagnostic: where the problem is and what it
/// is. `path` is a human-readable address into the scene tree (for example
/// `nodes[0].kind`); `message` describes the violation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SceneError {
    /// Address into the scene tree, outermost segment first.
    pub path: Vec<String>,
    /// Human-readable description of the violation.
    pub message: String,
}

impl SceneError {
    fn at(path: &[String], message: impl Into<String>) -> Self {
        Self {
            path: path.to_vec(),
            message: message.into(),
        }
    }

    /// Render the path as a dotted/indexed address, or `<root>` when empty.
    pub fn path_string(&self) -> String {
        if self.path.is_empty() {
            "<root>".to_owned()
        } else {
            self.path.join("")
        }
    }
}

impl core::fmt::Display for SceneError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {}", self.path_string(), self.message)
    }
}

/// Build a plain data map from string-keyed entries (keys become `core`-less
/// symbols). Use [`node`] to build a tagged scene node.
pub use sim_value::build::map;

/// Build a scene node: an `Expr::Map` whose first entry is `kind: scene/<name>`
/// followed by `entries`.
pub fn node(kind_name: &str, entries: Vec<(&str, Expr)>) -> Expr {
    let mut pairs = Vec::with_capacity(entries.len() + 1);
    pairs.push((
        Expr::Symbol(Symbol::new(KIND_KEY)),
        Expr::Symbol(Symbol::qualified(crate::kinds::SCENE_NAMESPACE, kind_name)),
    ));
    for (key, value) in entries {
        pairs.push((Expr::Symbol(Symbol::new(key)), value));
    }
    Expr::Map(pairs)
}

/// If `expr` is a map tagged with a symbol `kind`, return that kind symbol.
pub fn node_kind(expr: &Expr) -> Option<Symbol> {
    sim_value::access::field_sym(expr, KIND_KEY)
}

fn kind_entry(map: &Expr) -> Option<&Expr> {
    sim_value::access::field(map, KIND_KEY)
}

fn has_kind_key(map: &Expr) -> bool {
    kind_entry(map).is_some()
}

/// Validate that `expr` is a well-formed scene, failing closed with a
/// [`SceneError`] otherwise.
///
/// The root must be a scene node (a map tagged with a recognized `scene/<kind>`
/// symbol). Nested maps that carry a `kind` key are validated as scene nodes
/// too; maps without a `kind` key are treated as plain data and only recursed
/// into. This keeps the metadata open (arbitrary data may ride along) while
/// still rejecting a map that claims to be a scene node but is not one.
pub fn validate_scene(expr: &Expr) -> Result<(), SceneError> {
    let mut path = Vec::new();
    validate_node(expr, &mut path)
}

fn validate_node(expr: &Expr, path: &mut Vec<String>) -> Result<(), SceneError> {
    let shape_error = check_scene_shape(expr, path)?;
    let Expr::Map(entries) = expr else {
        return Err(SceneError::at(
            path,
            "expected a scene node map (an Expr::Map tagged with a kind)",
        ));
    };
    match kind_entry(expr) {
        None => {
            return Err(SceneError::at(path, "scene node is missing a 'kind' tag"));
        }
        Some(Expr::Symbol(kind)) => {
            if !is_known_kind(kind) {
                return Err(SceneError::at(
                    path,
                    format!(
                        "unrecognized scene kind '{kind}' -- if this is a plain data map, \
                         rename its 'kind' field (scene node maps reserve 'kind')"
                    ),
                ));
            }
        }
        Some(_) => {
            return Err(SceneError::at(path, "scene node 'kind' must be a symbol"));
        }
    }
    if let Some(message) = shape_error {
        return Err(SceneError::at(path, message));
    }
    validate_children(entries, path)
}

fn check_scene_shape(expr: &Expr, path: &[String]) -> Result<Option<String>, SceneError> {
    let mut cx = Cx::new(Arc::new(NoopEvalPolicy), Arc::new(DefaultFactory));
    let matched = crate::shapes::scene_shape()
        .check_expr(&mut cx, expr)
        .map_err(|error| SceneError::at(path, format!("scene shape check failed: {error}")))?;
    Ok((!matched.accepted)
        .then(|| rejection_message(&matched, "value is not a recognized scene node")))
}

fn rejection_message(matched: &ShapeMatch, fallback: &str) -> String {
    matched
        .diagnostics
        .first()
        .map(|diagnostic| diagnostic.message.clone())
        .unwrap_or_else(|| fallback.to_owned())
}

fn validate_children(entries: &[(Expr, Expr)], path: &mut Vec<String>) -> Result<(), SceneError> {
    for (key, value) in entries {
        let label = match key {
            Expr::Symbol(symbol) => format!(".{}", symbol.as_qualified_str()),
            other => format!(".{other:?}"),
        };
        path.push(label);
        validate_data(value, path)?;
        path.pop();
    }
    Ok(())
}

fn validate_data(expr: &Expr, path: &mut Vec<String>) -> Result<(), SceneError> {
    match expr {
        Expr::Map(_) if has_kind_key(expr) => validate_node(expr, path),
        Expr::Map(entries) => validate_children(entries, path),
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
            for (index, item) in items.iter().enumerate() {
                path.push(format!("[{index}]"));
                validate_data(item, path)?;
                path.pop();
            }
            Ok(())
        }
        _ => Ok(()),
    }
}
