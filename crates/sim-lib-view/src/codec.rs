//! The unified, reversible surface codec.
//!
//! VIEW_4 collapses the separate [`View`] (encode: `Value -> Scene`) and
//! [`Editor`] (decode: `(Value, Intent) -> Draft`, then `Draft -> Operation`)
//! halves into ONE contract, [`SurfaceCodec`], so a lens renders and parses from
//! one object and the two directions cannot drift. Rendering runs the codec
//! forward (capability-aware projection); editing runs it backward.
//!
//! The headline invariant is a CHECKABLE roundtrip property: decoding a REAL
//! `edit-field` that sets the value to a distinct sentinel yields a committable
//! [`Draft`] that actually proposes that sentinel, and the draft commits. A
//! lossy editor -- one that drops the edit and re-proposes the base -- fails the
//! property. [`roundtrip_holds`] verifies it for any codec + value;
//! [`noop_roundtrip_holds`] keeps the weaker no-op check (a cancel proposes no
//! change).
//!
//! Projection ([`SurfaceCodec::encode`]) is deterministic for a given
//! `(value, caps)`: [`reduce_for_caps`] fits the rendered Scene to the surface's
//! display density (glance/compact/regular/dense) by a fixed strategy, so the
//! same inputs always yield the same Scene -- the basis for replay and tests.
//!
//! # Example
//!
//! ```
//! use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr};
//! use sim_lib_view::{codec::{PairCodec, SurfaceCodec, roundtrip_holds}, surface, UniversalView, UniversalEditor};
//! use std::sync::Arc;
//!
//! let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
//! let codec = PairCodec::new(Arc::new(UniversalView), Arc::new(UniversalEditor::writable()));
//! let value = Expr::String("hello".to_owned());
//! // A real edit applied through the codec is faithfully reproduced -- the
//! // reversibility property (a lossy editor would make this false).
//! assert!(roundtrip_holds(&mut cx, &codec, &value).unwrap());
//! // Projection to a glance surface still yields a valid Scene.
//! let watch = surface::preset("watch").unwrap();
//! let scene = codec.encode(&mut cx, &value, &watch).unwrap();
//! assert!(sim_lib_scene::validate_scene(&scene).is_ok());
//! ```

use std::sync::Arc;

use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_intent::Origin;

use crate::contract::{Draft, Editor, Operation, View};
use crate::surface::SurfaceCaps;

/// One reversible surface codec: encode a value to a projected Scene, decode an
/// Intent to a Draft, and commit a Draft to a checked Operation.
///
/// Implementors back both directions with one definition. [`PairCodec`] adapts a
/// view/editor pair to this contract.
pub trait SurfaceCodec: Send + Sync {
    /// Renders `value` to a Scene projected for `caps` (deterministic).
    fn encode(&self, cx: &mut Cx, value: &Expr, caps: &SurfaceCaps) -> Result<Expr>;
    /// Decodes `intent` against `value` into a previewable [`Draft`].
    fn decode(&self, cx: &mut Cx, value: &Expr, intent: &Expr) -> Result<Draft>;
    /// Commits a committable [`Draft`] into a checked [`Operation`].
    fn commit(&self, cx: &mut Cx, draft: &Draft) -> Result<Operation>;
}

/// Adapts a ([`View`], [`Editor`]) pair to the unified [`SurfaceCodec`].
///
/// `encode` runs the view forward then projects to the surface via
/// [`reduce_for_caps`]; `decode`/`commit` delegate to the editor.
pub struct PairCodec {
    view: Arc<dyn View>,
    editor: Arc<dyn Editor>,
}

impl PairCodec {
    /// Pairs a view and an editor into one reversible codec.
    pub fn new(view: Arc<dyn View>, editor: Arc<dyn Editor>) -> Self {
        Self { view, editor }
    }
}

impl SurfaceCodec for PairCodec {
    fn encode(&self, cx: &mut Cx, value: &Expr, caps: &SurfaceCaps) -> Result<Expr> {
        let scene = self.view.encode(cx, value)?;
        sim_lib_scene::validate_scene(&scene)
            .map_err(|error| Error::HostError(format!("invalid scene: {error}")))?;
        let projected = reduce_for_caps(&scene, caps);
        sim_lib_scene::validate_scene(&projected).map_err(|error| {
            Error::HostError(format!("projection produced an invalid scene: {error}"))
        })?;
        Ok(projected)
    }

    fn decode(&self, cx: &mut Cx, value: &Expr, intent: &Expr) -> Result<Draft> {
        sim_lib_intent::validate_intent(intent)
            .map_err(|error| Error::HostError(format!("invalid intent: {error}")))?;
        self.editor.decode(cx, value, intent)
    }

    fn commit(&self, cx: &mut Cx, draft: &Draft) -> Result<Operation> {
        self.editor.commit(cx, draft)
    }
}

/// Builds the canonical no-op Intent (`intent/cancel`): it proposes no change.
///
/// `intent/cancel` carries the pane it cancels; a synthetic `"roundtrip"` pane
/// is used for the conformance check.
pub fn noop_intent() -> Expr {
    sim_lib_intent::intent(
        "cancel",
        Origin::human(0),
        vec![("pane", Expr::String("roundtrip".to_owned()))],
    )
}

/// A sentinel value guaranteed to differ from `value`: it embeds `value`, so it
/// can never be structurally equal to it.
fn roundtrip_sentinel(value: &Expr) -> Expr {
    Expr::List(vec![
        Expr::Symbol(Symbol::new("roundtrip-edit")),
        value.clone(),
    ])
}

/// A real `edit-field` Intent that sets the whole value (root path) to `target`.
fn roundtrip_edit(value: &Expr, target: Expr) -> Expr {
    sim_lib_intent::intent(
        "edit-field",
        Origin::human(0),
        vec![
            ("target", value.clone()),
            ("path", Expr::List(Vec::new())),
            ("value", target),
        ],
    )
}

/// Verifies the reversibility property for `codec` and `value` with a REAL edit.
///
/// Decodes an `edit-field` that sets the value to a distinct sentinel; the
/// property holds when the resulting [`Draft`] is committable, actually proposes
/// that sentinel (so an editor that drops the edit fails), and commits to an
/// [`Operation`]. This is not a tautology: a lossy editor that re-proposes the
/// base returns `false`. [`noop_roundtrip_holds`] keeps the weaker no-op check.
pub fn roundtrip_holds(cx: &mut Cx, codec: &dyn SurfaceCodec, value: &Expr) -> Result<bool> {
    let target = roundtrip_sentinel(value);
    let intent = roundtrip_edit(value, target.clone());
    let draft = codec.decode(cx, value, &intent)?;
    if !draft.committable || draft.proposed != target {
        return Ok(false);
    }
    // The committed operation must be producible and carry the edited value
    // (the universal operation sets the resource to `draft.proposed == target`).
    codec.commit(cx, &draft)?;
    Ok(true)
}

/// The weaker no-op reversibility check: decoding a [`noop_intent`] proposes no
/// change. True for any editor that honors cancel, so it cannot stand in for
/// [`roundtrip_holds`] -- keep both.
pub fn noop_roundtrip_holds(cx: &mut Cx, codec: &dyn SurfaceCodec, value: &Expr) -> Result<bool> {
    let draft = codec.decode(cx, value, &noop_intent())?;
    Ok(draft.committable && &draft.proposed == value)
}

/// Deterministically fits a Scene to a surface's display density.
///
/// Reads `caps.display_density()` and reduces child lists / table rows:
/// `glance` keeps the first item, `compact` keeps the first three, and
/// `regular`/`dense`/absent keep everything. The reduction is shallow and total
/// -- the same `(scene, caps)` always yields the same Scene.
pub fn reduce_for_caps(scene: &Expr, caps: &SurfaceCaps) -> Expr {
    let limit = match caps.display_density().as_ref().map(|d| &*d.name) {
        Some("glance") => Some(1),
        Some("compact") => Some(3),
        _ => None,
    };
    match limit {
        Some(n) => truncate_collections(scene, n),
        None => scene.clone(),
    }
}

/// Truncates a Scene node's `children` (stacks/boxes) and `rows` (tables) to at
/// most `n`, recursing into kept children. Non-Scene shapes pass through.
fn truncate_collections(scene: &Expr, n: usize) -> Expr {
    let Expr::Map(entries) = scene else {
        return scene.clone();
    };
    let reduced = entries
        .iter()
        .map(|(key, value)| {
            let collection = match key {
                Expr::Symbol(symbol) if symbol.namespace.is_none() => {
                    matches!(&*symbol.name, "children" | "rows")
                }
                _ => false,
            };
            match value {
                Expr::List(items) if collection => {
                    let kept: Vec<Expr> = items
                        .iter()
                        .take(n)
                        .map(|item| truncate_collections(item, n))
                        .collect();
                    (key.clone(), Expr::List(kept))
                }
                _ => (key.clone(), value.clone()),
            }
        })
        .collect();
    Expr::Map(reduced)
}

/// The id under which the universal default surface codec is registered.
pub const UNIVERSAL_SURFACE_CODEC_ID: &str = "surface:default";

/// Symbol form of [`UNIVERSAL_SURFACE_CODEC_ID`].
pub fn universal_surface_codec_symbol() -> Symbol {
    Symbol::new(UNIVERSAL_SURFACE_CODEC_ID)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface;
    use crate::{UniversalEditor, UniversalView};

    use sim_kernel::testing::eager_cx as cx;

    fn codec() -> PairCodec {
        PairCodec::new(
            Arc::new(UniversalView),
            Arc::new(UniversalEditor::writable()),
        )
    }

    #[test]
    fn roundtrip_holds_for_values() {
        let mut cx = cx();
        let codec = codec();
        for value in [
            Expr::Nil,
            Expr::String("text".to_owned()),
            Expr::List(vec![Expr::Nil, Expr::Bool(true)]),
        ] {
            assert!(
                roundtrip_holds(&mut cx, &codec, &value).unwrap(),
                "no-op edit must preserve {value:?}"
            );
        }
    }

    /// A deliberately lossy editor: it ignores every edit and re-proposes the
    /// base. The real-edit roundtrip must reject it; the no-op check still
    /// passes, which is exactly why the no-op check alone is not enough.
    struct LossyEditor;

    impl Editor for LossyEditor {
        fn decode(&self, _cx: &mut Cx, value: &Expr, _intent: &Expr) -> Result<Draft> {
            Ok(Draft::clean(value.clone(), value.clone()))
        }
        fn commit(&self, _cx: &mut Cx, draft: &Draft) -> Result<Operation> {
            Ok(Operation::new(draft.proposed.clone()))
        }
    }

    #[test]
    fn a_lossy_editor_fails_the_reversibility_property() {
        let mut cx = cx();
        let codec = PairCodec::new(Arc::new(UniversalView), Arc::new(LossyEditor));
        let value = Expr::String("hello".to_owned());
        assert!(
            !roundtrip_holds(&mut cx, &codec, &value).unwrap(),
            "an editor that drops edits must fail the reversibility property"
        );
        // The weaker no-op check still passes for the SAME lossy editor, proving
        // it cannot substitute for the real-edit roundtrip.
        assert!(noop_roundtrip_holds(&mut cx, &codec, &value).unwrap());
    }

    #[test]
    fn projection_is_deterministic_per_caps() {
        let mut cx = cx();
        let codec = codec();
        let value = Expr::List(vec![Expr::String("a".into()), Expr::String("b".into())]);
        for name in surface::SURFACE_PRESETS {
            let caps = surface::preset(name).unwrap();
            let first = codec.encode(&mut cx, &value, &caps).unwrap();
            let second = codec.encode(&mut cx, &value, &caps).unwrap();
            assert_eq!(first, second, "{name} projection must be deterministic");
            assert!(sim_lib_scene::validate_scene(&first).is_ok());
        }
    }

    #[test]
    fn glance_reduces_more_than_dense() {
        let glance = surface::preset("watch").unwrap(); // glance density
        let dense = surface::preset("desktop").unwrap(); // dense density
        let scene = sim_lib_scene::build::stack(
            "column",
            vec![
                sim_lib_scene::build::text_node("one"),
                sim_lib_scene::build::text_node("two"),
                sim_lib_scene::build::text_node("three"),
                sim_lib_scene::build::text_node("four"),
            ],
        );
        let reduced = reduce_for_caps(&scene, &glance);
        let kept = reduce_for_caps(&scene, &dense);
        assert!(sim_lib_scene::validate_scene(&reduced).is_ok());
        assert_eq!(child_count(&kept), 4, "dense keeps all children");
        assert_eq!(child_count(&reduced), 1, "glance keeps one child");
    }

    fn child_count(scene: &Expr) -> usize {
        let Expr::Map(entries) = scene else {
            return 0;
        };
        for (key, value) in entries {
            match (key, value) {
                (Expr::Symbol(symbol), Expr::List(items)) if &*symbol.name == "children" => {
                    return items.len();
                }
                _ => {}
            }
        }
        0
    }
}
