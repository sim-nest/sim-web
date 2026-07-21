//! Multi-surface synchronized edit sessions: broadcast, handoff, and replay.
//!
//! One resource can be open in MANY surfaces at once. The [`SurfaceHub`] owns
//! the single CANONICAL value for every resource and is the one coordination
//! point: a committed edit on any surface is applied to the canonical store and
//! then BROADCAST -- as a Scene plus a Scene diff -- to every surface/pane
//! viewing that resource, including the surface that issued the edit. This
//! avoids trying to make N independent transports share events; the hub is the
//! shared state.
//!
//! Edits flow through the universal default lens: an Intent is proposed and
//! committed through `edit:default`, yielding the universal `{op: set-value,
//! value: <proposed>}` operation, which is applied to the canonical store and
//! recorded in an append-only [`EditRow`] ledger carrying the issuing
//! operator and logical tick. Two surfaces editing the same resource therefore
//! apply in submit order (last write wins), and the ledger is replayable:
//! [`replay`] re-applies it to a seed state and reproduces the same canonical
//! state, proving the edit log is auditable.
//!
//! Handoff ([`SurfaceHub::handoff`]) opens an already-held resource on a second
//! surface so subsequent edits broadcast to both.

use std::collections::BTreeMap;
use std::sync::Arc;

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Error, Expr, Result, Symbol};
use sim_lib_view::codec::reduce_for_caps;
use sim_lib_view::{
    LensRegistry, SurfaceCaps, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
};

/// The role a surface holds inside a synchronized hub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    /// Primary focus surface for a shared session.
    Main,
    /// Secondary peer surface for the same canonical session.
    Peer,
}

/// One re-rendered Scene pushed to a surface/pane after a canonical edit.
///
/// `diff` is the Scene patch from the pane's cached Scene to `scene`; applying
/// it with [`sim_lib_scene::apply`] reconstructs `scene`.
#[derive(Clone, Debug)]
pub struct Broadcast {
    /// The surface that receives this update.
    pub surface: Symbol,
    /// The pane on that surface.
    pub pane: Symbol,
    /// The full new Scene for the pane.
    pub scene: Expr,
    /// The Scene patch from the pane's prior Scene to `scene`.
    pub diff: Expr,
}

/// One append-only ledger row: a committed edit, attributed and replayable.
///
/// Rows are appended in submit order. Replaying them in order through
/// [`replay`] reproduces the final canonical state.
#[derive(Clone, Debug)]
pub struct EditRow {
    /// The resource that was edited.
    pub resource: Symbol,
    /// The issuing operator (from the Intent origin, e.g. `human`/`agent`).
    pub operator: Symbol,
    /// The issuing logical tick (from the Intent origin `at-tick`).
    pub tick: u64,
    /// The committed `{op: set-value, value: <proposed>}` operation.
    pub operation: Expr,
}

/// Public snapshot of one live `(surface, pane)` binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceBinding {
    /// The surface that owns the binding.
    pub surface: Symbol,
    /// The pane on that surface.
    pub pane: Symbol,
    /// The canonical resource shown by the binding.
    pub resource: Symbol,
}

/// A live binding of a `(surface, pane)` to a resource, with the last Scene
/// shown there so the next broadcast can be diffed against it.
struct Binding {
    surface: Symbol,
    pane: Symbol,
    resource: Symbol,
    last_scene: Expr,
}

impl Binding {
    fn snapshot(&self) -> SurfaceBinding {
        SurfaceBinding {
            surface: self.surface.clone(),
            pane: self.pane.clone(),
            resource: self.resource.clone(),
        }
    }
}

/// The canonical multi-surface coordination point.
///
/// Holds the single canonical value per resource, the universal [`LensRegistry`]
/// that renders and edits, an owned [`Cx`], the registered surfaces and their
/// [`SurfaceCaps`], the live `(surface, pane)` bindings, and the append-only
/// [`EditRow`] ledger.
pub struct SurfaceHub {
    canonical: BTreeMap<Symbol, Expr>,
    registry: LensRegistry,
    cx: Cx,
    surfaces: BTreeMap<Symbol, SurfaceCaps>,
    roles: BTreeMap<Symbol, SurfaceRole>,
    bindings: Vec<Binding>,
    ledger: Vec<EditRow>,
}

impl Default for SurfaceHub {
    fn default() -> Self {
        Self::new()
    }
}

impl SurfaceHub {
    /// A new hub with the universal default lens registered (writable) and no
    /// resources, surfaces, bindings, or ledger rows.
    pub fn new() -> Self {
        let mut registry = LensRegistry::new();
        register_universal_default(&mut registry, false);
        Self {
            canonical: BTreeMap::new(),
            registry,
            cx: Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory)),
            surfaces: BTreeMap::new(),
            roles: BTreeMap::new(),
            bindings: Vec::new(),
            ledger: Vec::new(),
        }
    }

    /// Set (or replace) the canonical value of `resource`.
    pub fn seed(&mut self, resource: Symbol, value: Expr) {
        self.canonical.insert(resource, value);
    }

    /// Register a surface (identified by `surface`) with its capabilities.
    /// Re-registering replaces the stored caps.
    pub fn register_surface(&mut self, surface: Symbol, caps: SurfaceCaps) {
        self.register_surface_with_role(surface, caps, SurfaceRole::Main);
    }

    /// Register a surface with an explicit role inside the hub.
    pub fn register_surface_with_role(
        &mut self,
        surface: Symbol,
        caps: SurfaceCaps,
        role: SurfaceRole,
    ) {
        self.roles.insert(surface.clone(), role);
        self.surfaces.insert(surface, caps);
    }

    /// Returns the role recorded for `surface`.
    pub fn surface_role(&self, surface: &Symbol) -> Option<SurfaceRole> {
        self.roles.get(surface).copied()
    }

    /// Bind `(surface, pane)` to `resource`, render the canonical value through
    /// the universal view (projected to the surface caps via
    /// [`reduce_for_caps`]), cache that Scene for the pane, and return it.
    ///
    /// An existing binding for the same `(surface, pane)` is replaced. Fails if
    /// the surface is not registered or the resource has no canonical value.
    pub fn open(&mut self, surface: &Symbol, pane: Symbol, resource: Symbol) -> Result<Expr> {
        let caps = self.caps_of(surface)?;
        let value = self.value_of(&resource)?;
        let scene = render_for_surface(&mut self.cx, &self.registry, &caps, &value)?;
        self.bindings
            .retain(|binding| !(binding.surface == *surface && binding.pane == pane));
        self.bindings.push(Binding {
            surface: surface.clone(),
            pane,
            resource,
            last_scene: scene.clone(),
        });
        Ok(scene)
    }

    /// Submit an Intent against the resource shown in `(surface, pane)`.
    ///
    /// The Intent is proposed and committed through the universal editor against
    /// the CURRENT canonical value; the resulting `set-value` operation is
    /// applied to the canonical store and appended to the ledger (attributed to
    /// the Intent origin's operator and tick). Then, for EVERY `(surface, pane)`
    /// viewing that resource -- including other surfaces -- the new value is
    /// re-rendered, diffed against the pane's cached Scene, the cache updated,
    /// and a [`Broadcast`] emitted. Returns all broadcasts.
    ///
    /// Fails closed (returns an error, never panics) if the pane is not open,
    /// the resource is missing, the Intent is invalid, or the draft is not
    /// committable.
    pub fn submit(
        &mut self,
        surface: &Symbol,
        pane: &Symbol,
        intent: &Expr,
    ) -> Result<Vec<Broadcast>> {
        let caps = self.caps_of(surface)?;
        require_surface_input(&caps, intent)?;
        let resource = self
            .bindings
            .iter()
            .find(|binding| binding.surface == *surface && binding.pane == *pane)
            .map(|binding| binding.resource.clone())
            .ok_or_else(|| Error::HostError(format!("({surface}, {pane}) is not open")))?;
        let value = self.value_of(&resource)?;

        let editor = Symbol::new(UNIVERSAL_EDITOR_ID);
        let draft = self
            .registry
            .propose(&mut self.cx, &editor, &value, intent)?;
        let operation = self.registry.commit(&mut self.cx, &editor, &draft)?;
        let new_value = apply_set_value(&operation.form)?;
        self.commit_change(surface, pane, intent, new_value, operation.form)
    }

    /// Commit an already decoded value update from `surface`/`pane`.
    ///
    /// Device coordinators use this when a physical Intent has already been
    /// reduced to a canonical value update. The hub still validates the Intent,
    /// enforces the source surface input capability, records exactly one
    /// append-only ledger row, and broadcasts through the same atomic path as
    /// [`Self::submit`].
    pub fn commit_value_from(
        &mut self,
        surface: &Symbol,
        pane: &Symbol,
        intent: &Expr,
        new_value: Expr,
    ) -> Result<Vec<Broadcast>> {
        sim_lib_intent::validate_intent(intent)
            .map_err(|error| Error::HostError(format!("invalid intent: {error}")))?;
        let caps = self.caps_of(surface)?;
        require_surface_input(&caps, intent)?;
        let resource = self
            .bindings
            .iter()
            .find(|binding| binding.surface == *surface && binding.pane == *pane)
            .map(|binding| binding.resource.clone())
            .ok_or_else(|| Error::HostError(format!("({surface}, {pane}) is not open")))?;
        self.commit_resource_change(resource, intent, new_value)
    }

    /// Detach a surface from this hub without changing canonical resources or
    /// ledger history.
    pub fn detach_surface(&mut self, surface: &Symbol) -> Vec<SurfaceBinding> {
        self.surfaces.remove(surface);
        self.roles.remove(surface);
        let mut removed = Vec::new();
        self.bindings.retain(|binding| {
            if binding.surface == *surface {
                removed.push(binding.snapshot());
                false
            } else {
                true
            }
        });
        removed
    }

    /// Returns live bindings currently showing `resource`.
    pub fn bindings_for_resource(&self, resource: &Symbol) -> Vec<SurfaceBinding> {
        self.bindings
            .iter()
            .filter(|binding| binding.resource == *resource)
            .map(Binding::snapshot)
            .collect()
    }

    fn commit_change(
        &mut self,
        surface: &Symbol,
        pane: &Symbol,
        intent: &Expr,
        new_value: Expr,
        operation: Expr,
    ) -> Result<Vec<Broadcast>> {
        let resource = self
            .bindings
            .iter()
            .find(|binding| binding.surface == *surface && binding.pane == *pane)
            .map(|binding| binding.resource.clone())
            .ok_or_else(|| Error::HostError(format!("({surface}, {pane}) is not open")))?;
        self.commit_resource_change_with_operation(resource, intent, new_value, operation)
    }

    fn commit_resource_change(
        &mut self,
        resource: Symbol,
        intent: &Expr,
        new_value: Expr,
    ) -> Result<Vec<Broadcast>> {
        let operation = set_value_operation(new_value.clone());
        self.commit_resource_change_with_operation(resource, intent, new_value, operation)
    }

    fn commit_resource_change_with_operation(
        &mut self,
        resource: Symbol,
        intent: &Expr,
        new_value: Expr,
        operation: Expr,
    ) -> Result<Vec<Broadcast>> {
        // Render EVERY per-surface broadcast into a staging buffer FIRST. A
        // render (or a surface that lost its capabilities) can fail mid-iteration;
        // if it does we must mutate nothing -- otherwise canonical/ledger move
        // forward while some caches advance and no broadcast is delivered, an
        // unrecoverable replay divergence. We commit only after all succeed.
        let mut staged: Vec<(usize, Broadcast)> = Vec::new();
        {
            let Self {
                registry,
                cx,
                surfaces,
                bindings,
                ..
            } = self;
            for (index, binding) in bindings.iter().enumerate() {
                if binding.resource != resource {
                    continue;
                }
                let caps = surfaces.get(&binding.surface).ok_or_else(|| {
                    Error::HostError(format!(
                        "surface '{}' lost its capabilities",
                        binding.surface
                    ))
                })?;
                let scene = render_for_surface(cx, registry, caps, &new_value)?;
                let diff = sim_lib_scene::diff(&binding.last_scene, &scene);
                staged.push((
                    index,
                    Broadcast {
                        surface: binding.surface.clone(),
                        pane: binding.pane.clone(),
                        scene,
                        diff,
                    },
                ));
            }
        }

        // All broadcasts rendered: commit atomically -- canonical, then ledger,
        // then swap in each surface's advanced last_scene cache.
        self.canonical.insert(resource.clone(), new_value);
        let (operator, tick) = origin_of(intent);
        self.ledger.push(EditRow {
            resource,
            operator,
            tick,
            operation,
        });
        let mut broadcasts = Vec::with_capacity(staged.len());
        for (index, broadcast) in staged {
            self.bindings[index].last_scene = broadcast.scene.clone();
            broadcasts.push(broadcast);
        }
        Ok(broadcasts)
    }

    /// Hand `resource` off from `from` to `to`: open it on `to` in a new `pane`
    /// and return its Scene. The `from` surface keeps its binding, so the
    /// resource is now open on both and subsequent edits broadcast to both.
    ///
    /// Fails if `from` does not currently hold `resource`.
    pub fn handoff(
        &mut self,
        from: &Symbol,
        to: &Symbol,
        resource: Symbol,
        pane: Symbol,
    ) -> Result<Expr> {
        let held = self
            .bindings
            .iter()
            .any(|binding| binding.surface == *from && binding.resource == resource);
        if !held {
            return Err(Error::HostError(format!(
                "surface '{from}' does not hold resource '{resource}' to hand off"
            )));
        }
        self.open(to, pane, resource)
    }

    /// The append-only edit ledger, in submit order.
    pub fn ledger(&self) -> &[EditRow] {
        &self.ledger
    }

    /// The current canonical value of `resource`, if any.
    pub fn canonical(&self, resource: &Symbol) -> Option<&Expr> {
        self.canonical.get(resource)
    }

    fn caps_of(&self, surface: &Symbol) -> Result<SurfaceCaps> {
        self.surfaces
            .get(surface)
            .cloned()
            .ok_or_else(|| Error::HostError(format!("surface '{surface}' is not registered")))
    }

    fn value_of(&self, resource: &Symbol) -> Result<Expr> {
        self.canonical.get(resource).cloned().ok_or_else(|| {
            Error::HostError(format!("resource '{resource}' has no canonical value"))
        })
    }
}

/// Re-apply a ledger to a seed canonical state, yielding the final state.
///
/// Rows are applied in order; for a resource, the last `set-value` wins. This is
/// the replay surface that proves the ledger is auditable: feeding the rows
/// produced by a run of edits back over the original seed reproduces the final
/// canonical state of the hub.
///
/// Every committed row carries the universal `{op: set-value, ...}` operation,
/// so replay fails closed if a row's operation is not a `set-value`: a foreign
/// or corrupted ledger row is surfaced as an error rather than silently dropped,
/// which would otherwise reproduce a state that never existed.
pub fn replay(rows: &[EditRow], seed: BTreeMap<Symbol, Expr>) -> Result<BTreeMap<Symbol, Expr>> {
    let mut state = seed;
    for row in rows {
        let value = apply_set_value(&row.operation)?;
        state.insert(row.resource.clone(), value);
    }
    Ok(state)
}

/// Render `value` through the universal view, projected to `caps`.
fn render_for_surface(
    cx: &mut Cx,
    registry: &LensRegistry,
    caps: &SurfaceCaps,
    value: &Expr,
) -> Result<Expr> {
    let scene = registry.render(cx, &Symbol::new(UNIVERSAL_VIEW_ID), value)?;
    Ok(reduce_for_caps(&scene, caps))
}

fn require_surface_input(caps: &SurfaceCaps, intent: &Expr) -> Result<()> {
    let required = input_capabilities_for_intent(intent)?;
    if required
        .iter()
        .any(|capability| caps.input_flag(capability))
    {
        return Ok(());
    }
    Err(Error::HostError(format!(
        "surface '{}' does not accept any required input for this Intent: {}",
        caps.client_id,
        required.join(", ")
    )))
}

fn input_capabilities_for_intent(intent: &Expr) -> Result<&'static [&'static str]> {
    let kind = match sim_value::access::field(intent, "kind") {
        Some(Expr::Symbol(kind)) if kind.namespace.as_deref() == Some("intent") => {
            kind.name.as_ref()
        }
        Some(Expr::Symbol(_)) => {
            return Err(Error::HostError(
                "Intent kind must be in the intent namespace".to_owned(),
            ));
        }
        _ => return Err(Error::HostError("submit input is not an Intent".to_owned())),
    };
    match kind {
        "tap" | "dismiss" | "commit" | "cancel" | "approve" | "reject" | "pause-agent"
        | "rerun-validation" | "replay-cassette" => Ok(&["tap", "pointer", "touch", "keyboard"]),
        "select" | "move" | "wire" | "unwire" | "create" | "delete" | "scrub"
        | "piano-roll-edit" | "player-rack-edit" | "arranger-edit" => Ok(&["pointer", "touch"]),
        "invoke" => Ok(&[
            "pointer",
            "touch",
            "tap",
            "button",
            "gaze",
            "head",
            "hand",
            "controller",
            "voice",
        ]),
        "edit" | "edit-field" | "set-param" | "set-lens" | "set-mode" | "open" | "ask"
        | "split-mission" | "open-source" => Ok(&["keyboard", "touch", "voice"]),
        "performance-event" => Ok(&["keyboard", "touch", "camera"]),
        other => Err(Error::HostError(format!(
            "no surface input capability mapping for intent/{other}"
        ))),
    }
}

fn set_value_operation(value: Expr) -> Expr {
    Expr::Map(vec![
        (
            Expr::Symbol(Symbol::new("op")),
            Expr::Symbol(Symbol::new("set-value")),
        ),
        (Expr::Symbol(Symbol::new("value")), value),
    ])
}

/// Interpret the universal `{op: set-value, value: <v>}` operation, returning
/// `<v>`. Any other shape fails closed.
fn apply_set_value(operation: &Expr) -> Result<Expr> {
    let Expr::Map(entries) = operation else {
        return Err(Error::HostError("operation is not a map".to_owned()));
    };
    let is_set_value = matches!(
        sim_value::access::entry_field(entries, "op"),
        Some(Expr::Symbol(symbol)) if &*symbol.name == "set-value"
    );
    if !is_set_value {
        return Err(Error::HostError(
            "operation is not a set-value op".to_owned(),
        ));
    }
    sim_value::access::entry_field(entries, "value")
        .cloned()
        .ok_or_else(|| Error::HostError("set-value operation is missing a 'value'".to_owned()))
}

/// Read the operator symbol and logical tick from an Intent origin, defaulting
/// to `unknown`/`0` if absent (the Intent is validated before this is called).
fn origin_of(intent: &Expr) -> (Symbol, u64) {
    let origin = sim_value::access::field(intent, "origin");
    let operator = origin
        .and_then(|origin| sim_value::access::field_sym(origin, "operator"))
        .unwrap_or_else(|| Symbol::new("unknown"));
    let tick = origin
        .and_then(|origin| sim_value::access::field_any(origin, "at-tick"))
        .and_then(|tick| match tick {
            Expr::Number(number) => number.canonical.parse::<u64>().ok(),
            _ => None,
        })
        .unwrap_or(0);
    (operator, tick)
}

#[cfg(test)]
mod tests;
