//! Registration of the universal default view and editor.
//!
//! Both are registered as the lowest-quality, always-matching lens of their
//! kind, so dispatch ends here when nothing specialized claims a value.

use std::sync::Arc;

use sim_kernel::Symbol;
use sim_shape::{AnyShape, shape_value};

use crate::contract::{Lens, LensKind, LensMeta};
use crate::dispatch::LensRegistry;
use crate::universal_editor::UniversalEditor;
use crate::universal_view::UniversalView;

/// The universal default view lens id.
pub const UNIVERSAL_VIEW_ID: &str = "view:default";

/// The universal default editor lens id.
pub const UNIVERSAL_EDITOR_ID: &str = "edit:default";

/// The lowest quality, so the universal default loses every ranked tie.
const LOWEST_QUALITY: i32 = -1_000_000;

fn any_shape() -> sim_kernel::Value {
    shape_value(Symbol::qualified("core", "Any"), Arc::new(AnyShape))
}

/// Register the universal default view and editor into `registry`. When
/// `readonly` is set, the universal editor renders but never commits.
pub fn register_universal_default(registry: &mut LensRegistry, readonly: bool) {
    registry.register(Lens::view(
        LensMeta::new(Symbol::new(UNIVERSAL_VIEW_ID), LensKind::View)
            .claiming_shape(any_shape())
            .with_quality_cost(LOWEST_QUALITY, 0)
            .as_universal_default(),
        Arc::new(UniversalView),
    ));
    let editor = if readonly {
        UniversalEditor::readonly()
    } else {
        UniversalEditor::writable()
    };
    registry.register(Lens::editor(
        LensMeta::new(Symbol::new(UNIVERSAL_EDITOR_ID), LensKind::Editor)
            .claiming_shape(any_shape())
            .with_quality_cost(LOWEST_QUALITY, 0)
            .as_universal_default(),
        Arc::new(editor),
    ));
}
