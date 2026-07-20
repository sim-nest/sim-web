use std::rc::Rc;

use sim_kernel::Expr;
use sim_lib_scene::{GLANCE_KIND, GlanceCard, node, validate_scene};
use sim_lib_view_device::{
    AckChannel, DeviceTier, EncodedScene, GlanceAdapter, GlanceBudget, GlanceInput, GlanceState,
    Split, reduce_scene_to_glance, tier_preset,
};
use sim_value::{access, build};

use crate::{
    WATCH_GLANCE_ACK_MS, WATCH_GLANCE_CELLS, WATCH_GLANCE_GLYPHS, WATCH_GLANCE_LARGE_CELLS,
    WATCH_GLANCE_LARGE_GLYPHS, watch_glance_adapter, watch_glance_budget, watch_glance_budget_demo,
    watch_glance_large_adapter, watch_glance_large_budget,
};

#[test]
fn watch_configures_shared_glance_adapter() {
    let budget = watch_glance_budget();
    assert_eq!(budget.cells, WATCH_GLANCE_CELLS);
    assert_eq!(budget.glyphs, WATCH_GLANCE_GLYPHS);
    assert_eq!(budget.ack, AckChannel::Haptic);

    let large_budget = watch_glance_large_budget();
    assert_eq!(large_budget.cells, WATCH_GLANCE_LARGE_CELLS);
    assert_eq!(large_budget.glyphs, WATCH_GLANCE_LARGE_GLYPHS);
    assert_eq!(large_budget.ack, AckChannel::Haptic);

    let adapter = watch_glance_adapter(false);
    assert_eq!(adapter.budget, budget);
    assert_eq!(adapter.ack_ms, WATCH_GLANCE_ACK_MS);

    let large_adapter = watch_glance_large_adapter();
    assert_eq!(large_adapter.budget, large_budget);
    assert_eq!(large_adapter.ack_ms, WATCH_GLANCE_ACK_MS);
}

#[test]
fn rich_scene_reduces_to_one_glance_card_through_shared_path() {
    let profile = tier_preset(DeviceTier::Actuator);
    let glance = reduce_scene_to_glance(&rich_scene(), &profile).expect("reduces");

    assert_eq!(scene_kind(&glance).as_deref(), Some(GLANCE_KIND));
    let card = GlanceCard::from_scene(&glance).expect("glance parses");
    assert_eq!(card.title, "Wrist status");
    assert_eq!(card.metric.expect("metric").value, "83");
    assert_eq!(card.action.expect("action").label, "Dismiss");
    validate_scene(&glance).expect("glance validates");
}

#[test]
fn same_reduced_card_feeds_hud_and_round_watch_budgets() {
    let profile = tier_preset(DeviceTier::Actuator);
    let glance = reduce_scene_to_glance(&rich_scene(), &profile).expect("reduces");
    let encoded = EncodedScene::new(glance);
    let hud = GlanceAdapter::new(GlanceBudget::mono_hud(), 60);

    let hud_frame = Split::new(hud, profile.clone())
        .adapt_one(&encoded, &GlanceState::idle(1))
        .expect("hud adapts");
    let watch_frame = Split::new(watch_glance_adapter(false), profile)
        .adapt_one(&encoded, &GlanceState::idle(1))
        .expect("watch adapts");

    assert_eq!(scene_kind(&hud_frame).as_deref(), Some(GLANCE_KIND));
    assert_eq!(scene_kind(&watch_frame).as_deref(), Some(GLANCE_KIND));
    assert_eq!(field_u64(&hud_frame, "cells"), Some(2));
    assert_eq!(
        field_u64(&watch_frame, "cells"),
        Some(u64::from(WATCH_GLANCE_CELLS))
    );
    assert_eq!(card_title(&hud_frame), Some("Wrist status".to_owned()));
    assert_eq!(card_title(&watch_frame), Some("Wrist status".to_owned()));
}

#[test]
fn tap_yields_local_haptic_ack_without_encoder_call() {
    let profile = tier_preset(DeviceTier::Actuator);
    let glance = reduce_scene_to_glance(&rich_scene(), &profile).expect("reduces");
    let encoded = EncodedScene::new(glance);

    let frame = Split::new(watch_glance_adapter(false), profile)
        .adapt_one(&encoded, &GlanceState::with_input(GlanceInput::Tap, 7))
        .expect("tap adapts");

    assert_eq!(
        access::field_sym(&frame, "ack-channel")
            .expect("ack channel")
            .name
            .as_ref(),
        "haptic"
    );
    assert_eq!(
        access::field_sym(&frame, "ack-input")
            .expect("ack input")
            .name
            .as_ref(),
        "tap"
    );
    assert_eq!(field_u64(&frame, "ack-ms"), Some(WATCH_GLANCE_ACK_MS));
    assert_eq!(field_u64(&frame, "ack-tick"), Some(7));
}

#[test]
fn budget_demo_names_round_watch_budget() {
    let demo = watch_glance_budget_demo();
    let kind = access::field_sym(&demo, "kind").expect("kind");
    assert_eq!(kind.namespace.as_deref(), Some("view-wrist"));
    assert_eq!(kind.name.as_ref(), "glance-budget");
    assert_eq!(
        access::field_str(&demo, "model"),
        Some("amazfit-t-rex-3-pro-44")
    );
    assert_eq!(
        field_u64(&demo, "cells"),
        Some(u64::from(WATCH_GLANCE_CELLS))
    );
    assert_eq!(
        field_u64(&demo, "glyphs"),
        Some(u64::from(WATCH_GLANCE_GLYPHS))
    );
    let ack = access::field_sym(&demo, "ack").expect("ack");
    assert_eq!(ack.name.as_ref(), "haptic");
    assert_eq!(field_u64(&demo, "ack-ms"), Some(WATCH_GLANCE_ACK_MS));
}

fn rich_scene() -> Expr {
    node(
        "stack",
        vec![
            ("title", Expr::String("Wrist status".to_owned())),
            (
                "children",
                build::list(vec![
                    node(
                        "meter",
                        vec![
                            ("label", Expr::String("battery".to_owned())),
                            ("value", build::uint(83)),
                        ],
                    ),
                    node(
                        "button",
                        vec![
                            ("label", Expr::String("Dismiss".to_owned())),
                            ("target", build::sym("dismiss")),
                        ],
                    ),
                    node("text", vec![("text", Expr::String("connected".to_owned()))]),
                ]),
            ),
        ],
    )
}

fn scene_kind(expr: &Expr) -> Option<String> {
    sim_lib_scene::node_kind(expr).and_then(|symbol| {
        (symbol.namespace.as_deref() == Some(sim_lib_scene::kinds::SCENE_NAMESPACE))
            .then(|| symbol.name.to_string())
    })
}

fn field_u64(expr: &Expr, field: &str) -> Option<u64> {
    let Expr::Number(number) = access::field(expr, field)? else {
        return None;
    };
    number.canonical.parse().ok()
}

fn card_title(frame: &Rc<Expr>) -> Option<String> {
    GlanceCard::from_scene(frame).ok().map(|card| card.title)
}
