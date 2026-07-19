use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Error, Expr, Result};
use sim_lib_scene::{GLANCE_KIND, GlanceCard, node, validate_scene};
use sim_lib_view::{Draft, Operation, SurfaceCaps, SurfaceCodec};
use sim_value::{access, build};

use crate::{
    AckChannel, EncodedScene, GlanceAdapter, GlanceBudget, GlanceInput, GlanceReducer, GlanceState,
    Split, tier_preset,
};

#[test]
fn rich_scene_reduces_to_one_glance_card() {
    let profile = tier_preset(crate::DeviceTier::Actuator);
    let scene = rich_scene();

    let glance = GlanceReducer.reduce(&scene, &profile).expect("reduces");

    assert_eq!(scene_kind(&glance).as_deref(), Some(GLANCE_KIND));
    let card = GlanceCard::from_scene(&glance).expect("glance parses");
    assert_eq!(card.title, "Motor bay");
    assert_eq!(card.metric.expect("metric").value, "72");
    assert_eq!(card.action.expect("action").label, "Ack");
    validate_scene(&glance).expect("glance validates");
}

#[test]
fn one_reducer_two_budgets_drive_one_adapter() {
    let profile = tier_preset(crate::DeviceTier::Actuator);
    let glance = GlanceReducer
        .reduce(&rich_scene(), &profile)
        .expect("reduces");
    let hud = GlanceAdapter::new(GlanceBudget::mono_hud(), 60);
    let watch = GlanceAdapter::new(GlanceBudget::round_watch(), 120);
    let encoded = EncodedScene::new(glance);

    let hud_frame = Split::new(hud, profile.clone())
        .adapt_one(&encoded, &GlanceState::idle(1))
        .expect("hud adapts");
    let watch_frame = Split::new(watch, profile)
        .adapt_one(&encoded, &GlanceState::idle(1))
        .expect("watch adapts");

    assert_eq!(scene_kind(&hud_frame).as_deref(), Some(GLANCE_KIND));
    assert_eq!(scene_kind(&watch_frame).as_deref(), Some(GLANCE_KIND));
    assert_eq!(access::field_i64(&hud_frame, "cells"), Some(2));
    assert_eq!(access::field_i64(&watch_frame, "cells"), Some(4));
}

#[test]
fn tap_yields_configured_ack_without_encoder_call() {
    let encode_count = Arc::new(AtomicUsize::new(0));
    let codec = CountingEncoder {
        encode_count: Arc::clone(&encode_count),
    };
    let profile = tier_preset(crate::DeviceTier::Actuator);
    let adapter = GlanceAdapter::new(
        GlanceBudget {
            cells: 3,
            glyphs: 18,
            ack: AckChannel::Tone,
        },
        90,
    );
    let split = Split::new(adapter, profile);
    let caps = SurfaceCaps::from_preset("watch", "watch.local.glance").expect("watch caps");
    let mut cx = test_cx();

    let run = split
        .run(
            &codec,
            &mut cx,
            &rich_scene(),
            &caps,
            &[GlanceState::with_input(GlanceInput::Tap, 7)],
        )
        .expect("split run");

    assert_eq!(encode_count.load(Ordering::SeqCst), 1);
    let frame = run.frames.first().expect("one frame");
    assert_eq!(
        access::field_sym(frame, "ack-channel")
            .unwrap()
            .name
            .as_ref(),
        "tone"
    );
    assert_eq!(
        access::field_sym(frame, "ack-input").unwrap().name.as_ref(),
        "tap"
    );
    assert_eq!(access::field_i64(frame, "ack-ms"), Some(90));
    assert_eq!(access::field_i64(frame, "ack-tick"), Some(7));
}

#[derive(Clone)]
struct CountingEncoder {
    encode_count: Arc<AtomicUsize>,
}

impl SurfaceCodec for CountingEncoder {
    fn encode(&self, _cx: &mut Cx, value: &Expr, _caps: &SurfaceCaps) -> Result<Expr> {
        self.encode_count.fetch_add(1, Ordering::SeqCst);
        GlanceReducer.reduce(value, &tier_preset(crate::DeviceTier::Actuator))
    }

    fn decode(&self, _cx: &mut Cx, _value: &Expr, _intent: &Expr) -> Result<Draft> {
        Err(Error::HostError(
            "counting encoder only supports encode".to_owned(),
        ))
    }

    fn commit(&self, _cx: &mut Cx, _draft: &Draft) -> Result<Operation> {
        Err(Error::HostError(
            "counting encoder only supports encode".to_owned(),
        ))
    }
}

fn rich_scene() -> Expr {
    node(
        "stack",
        vec![
            ("title", Expr::String("Motor bay".to_owned())),
            (
                "children",
                build::list(vec![
                    node(
                        "meter",
                        vec![
                            ("label", Expr::String("load".to_owned())),
                            ("value", build::uint(72)),
                        ],
                    ),
                    node(
                        "button",
                        vec![
                            ("label", Expr::String("Ack".to_owned())),
                            ("target", build::sym("ack")),
                        ],
                    ),
                    node("text", vec![("text", Expr::String("extra".to_owned()))]),
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

fn test_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}
