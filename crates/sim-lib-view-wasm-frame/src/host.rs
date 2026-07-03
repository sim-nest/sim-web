//! Local view host for Scene/Intent facade tests.
//!
//! This Rust helper turns a value into a Scene, folds raw gestures into Intents,
//! and commits edits locally against an in-process value. It reuses the same
//! `sim-lib-view` lenses, `sim-lib-intent` gesture algebra, and `sim-lib-scene`
//! diff used by server and web-shell adapters, so facade tests exercise the same
//! data contracts without claiming a live in-page runtime.

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Result, Symbol};
use sim_lib_intent::gesture::{PointerEvent, RawGesture};
use sim_lib_intent::{Operator, Origin, intent_from_gesture};
use sim_lib_scene::diff;
use sim_lib_view::{
    LensRegistry, UNIVERSAL_EDITOR_ID, UNIVERSAL_VIEW_ID, register_universal_default,
};

use std::sync::Arc;

/// A re-rendered Scene with the diff from the previous Scene.
#[derive(Clone, Debug)]
pub struct SceneUpdate {
    /// The full new Scene.
    pub scene: Expr,
    /// The diff (a `scene/patch` value) from the previous Scene.
    pub diff: Expr,
}

/// A local host over a single value, rendering through the universal default
/// lens by default.
pub struct BrowserHost {
    cx: Cx,
    registry: LensRegistry,
    value: Expr,
    view_lens: Symbol,
    editor_lens: Symbol,
    scene: Expr,
    recognizer: sim_lib_intent::GestureRecognizer,
    operator: Operator,
    tick: u64,
}

impl BrowserHost {
    /// Build a host showing `value` through the universal default lens.
    pub fn new(value: Expr) -> Result<Self> {
        let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
        let mut registry = LensRegistry::new();
        register_universal_default(&mut registry, false);
        let view_lens = Symbol::new(UNIVERSAL_VIEW_ID);
        let editor_lens = Symbol::new(UNIVERSAL_EDITOR_ID);
        let scene = registry.render(&mut cx, &view_lens, &value)?;
        Ok(Self {
            cx,
            registry,
            value,
            view_lens,
            editor_lens,
            scene,
            recognizer: sim_lib_intent::GestureRecognizer::new(),
            operator: Operator::Human,
            tick: 0,
        })
    }

    /// The current value.
    pub fn value(&self) -> &Expr {
        &self.value
    }

    /// The current Scene.
    pub fn scene(&self) -> &Expr {
        &self.scene
    }

    fn origin(&mut self) -> Origin {
        self.tick += 1;
        Origin {
            operator: self.operator,
            at_tick: self.tick,
        }
    }

    /// Feed one raw pointer event; returns the composed Intent if a gesture
    /// completed (regardless of whether it mutates).
    pub fn feed_pointer(&mut self, event: PointerEvent) -> Result<Option<Expr>> {
        let Some(raw) = self.recognizer.pointer(event) else {
            return Ok(None);
        };
        self.compose(&raw).map(Some)
    }

    /// Compose a keyboard command into an Intent.
    pub fn key_command(&mut self, command: &str, hit: sim_lib_intent::Hit) -> Result<Expr> {
        let raw = sim_lib_intent::GestureRecognizer::key(command, hit);
        self.compose(&raw)
    }

    fn compose(&mut self, raw: &RawGesture) -> Result<Expr> {
        let origin = self.origin();
        intent_from_gesture(origin, "pane-main", raw)
            .map_err(|error| sim_kernel::Error::HostError(format!("gesture: {error}")))
    }

    /// Edit a field at `path` to `value` -- the field-entry gesture -- and apply
    /// it. Returns the Scene update on success.
    pub fn edit_field(&mut self, path: Expr, value: Expr) -> Result<Option<SceneUpdate>> {
        let origin = self.origin();
        let intent = sim_lib_intent::intent(
            "edit-field",
            origin,
            vec![
                ("target", self.value.clone()),
                ("path", path),
                ("value", value),
            ],
        );
        self.apply_intent(&intent)
    }

    /// Apply an Intent: if the editor can commit it, mutate the value and
    /// re-render; otherwise return `None`. A non-committable draft (a
    /// non-mutating Intent such as `select`, or a rejected edit whose diagnostic
    /// a web-shell adapter surfaces separately) leaves the view unchanged.
    pub fn apply_intent(&mut self, intent: &Expr) -> Result<Option<SceneUpdate>> {
        let draft = self
            .registry
            .propose(&mut self.cx, &self.editor_lens, &self.value, intent)?;
        if !draft.committable {
            return Ok(None);
        }
        let operation = self
            .registry
            .commit(&mut self.cx, &self.editor_lens, &draft)?;
        let new_value = set_value_of(&operation.form, &self.value);
        if new_value == self.value {
            return Ok(None);
        }
        self.value = new_value;
        let scene = self
            .registry
            .render(&mut self.cx, &self.view_lens, &self.value)?;
        let patch = diff(&self.scene, &scene);
        self.scene = scene.clone();
        Ok(Some(SceneUpdate { scene, diff: patch }))
    }
}

/// Read the `value` from a universal-editor `set-value` operation, falling back
/// to the current value for any other operation shape.
fn set_value_of(operation: &Expr, current: &Expr) -> Expr {
    let Expr::Map(entries) = operation else {
        return current.clone();
    };
    entries
        .iter()
        .find_map(|(key, value)| {
            matches!(key, Expr::Symbol(symbol) if &*symbol.name == "value").then_some(value)
        })
        .cloned()
        .unwrap_or_else(|| current.clone())
}
