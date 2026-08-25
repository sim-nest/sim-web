use super::*;
use sim_kernel::testing::bare_cx as cx;
use sim_lib_continuity::RoleDemand;

fn plan() -> ContinuityPlan {
    ContinuityPlan {
        retention_turns: 64,
        max_freshness: 10,
        disclosure: vec![Symbol::new("transcript"), Symbol::new("result")],
        roles: vec![RoleDemand {
            role: Symbol::new("root"),
            root: true,
            required_services: vec![],
            fallbacks: vec![],
        }],
        ..ContinuityPlan::default()
    }
}

#[test]
fn minimal_flow_rebuilds_every_view_and_never_needs_sound() {
    let mut controller = PhoneController::new(
        plan(),
        vec![Symbol::new("transcript"), Symbol::new("result")],
    )
    .unwrap();
    controller
        .apply_action(PhoneAction::HoldToTalk, 0, 1)
        .unwrap();
    let generation = controller.capture_generation().unwrap();
    assert_eq!(controller.accept_pcm(generation, &[1, 2, 3]), 3);
    controller.apply_action(PhoneAction::Release, 1, 2).unwrap();
    assert_eq!(controller.accept_pcm(generation, &[4, 5]), 0);
    controller
        .accept_transcript("quiet answer".into(), 2, 3)
        .unwrap();
    let first = controller.projection().unwrap();
    let rebuilt = controller.projection().unwrap();
    assert_eq!(first, rebuilt);
    assert_eq!(rebuilt.transcript.as_deref(), Some("quiet answer"));
    assert!(!rebuilt.sound_available);
    let scene = PhoneSurfaceCodec
        .encode(
            &mut cx(),
            &rebuilt.to_expr(),
            &sim_lib_view::surface::preset("phone").unwrap(),
        )
        .unwrap();
    validate_scene(&scene).unwrap();
    controller.apply_action(PhoneAction::Submit, 3, 4).unwrap();
    controller
        .accept_result("visible result".into(), 4, 5)
        .unwrap();
    let result = controller.projection().unwrap();
    assert_eq!(result.state.name.as_ref(), "result-review");
    assert_eq!(result.result.as_deref(), Some("visible result"));
    assert!(matches!(
        controller.apply_action(PhoneAction::Submit, 3, 5),
        Err(JournalError::FenceConflict)
    ));
}

#[test]
fn passive_events_never_arm_and_every_stop_boundary_rejects_pcm() {
    for passive in [
        PassiveEvent::Focus,
        PassiveEvent::PageOpen,
        PassiveEvent::Notification,
        PassiveEvent::Connection,
        PassiveEvent::Stale,
        PassiveEvent::Rotation,
    ] {
        let mut controller = PhoneController::new(plan(), vec![]).unwrap();
        controller.observe(passive);
        assert_eq!(controller.capture_generation(), None);
    }
    for stop in [
        PhoneAction::Release,
        PhoneAction::Discard,
        PhoneAction::Defer,
        PhoneAction::Cancel,
        PhoneAction::Stop,
    ] {
        let mut controller = PhoneController::new(plan(), vec![]).unwrap();
        controller
            .apply_action(PhoneAction::HoldToTalk, 0, 1)
            .unwrap();
        let generation = controller.capture_generation().unwrap();
        controller.apply_action(stop, 1, 2).unwrap();
        assert_eq!(controller.accept_pcm(generation, &[1]), 0);
    }
}

#[test]
fn actions_are_ordinary_intents_and_hostile_text_is_scene_data() {
    let mut controller = PhoneController::new(plan(), vec![Symbol::new("transcript")]).unwrap();
    let actions = [
        PhoneAction::HoldToTalk,
        PhoneAction::Release,
        PhoneAction::Review,
        PhoneAction::Submit,
        PhoneAction::Discard,
        PhoneAction::Type("</script><b>not markup</b>".into()),
        PhoneAction::Replay,
        PhoneAction::Defer,
        PhoneAction::Cancel,
        PhoneAction::Stop,
        PhoneAction::StrongerPlacement,
    ];
    for action in &actions {
        sim_lib_intent::validate_intent(&controller.intent_for(action, 1)).unwrap();
    }
    controller.apply_action(actions[5].clone(), 0, 1).unwrap();
    let projection = controller.projection().unwrap();
    assert_eq!(
        projection.transcript.as_deref(),
        Some("</script><b>not markup</b>")
    );
    let rendered = PhoneSurfaceCodec
        .encode(
            &mut cx(),
            &projection.to_expr(),
            &sim_lib_view::surface::preset("phone").unwrap(),
        )
        .unwrap();
    assert!(format!("{rendered:?}").contains("not markup"));
}

#[test]
fn disclosure_ceiling_is_applied_before_projection_and_manual_fallback_survives_rotation() {
    let mut controller = PhoneController::new(plan(), vec![]).unwrap();
    controller
        .apply_action(PhoneAction::Type("private".into()), 0, 1)
        .unwrap();
    controller.observe(PassiveEvent::Rotation);
    assert_eq!(
        controller.projection().unwrap().transcript.as_deref(),
        Some("[withheld]")
    );
    assert!(matches!(
        controller.apply_action(PhoneAction::Review, 0, 2),
        Err(JournalError::FenceConflict)
    ));
}
