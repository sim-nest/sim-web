use super::*;
#[test]
fn all_scenes_are_valid() {
    for kind in [
        SceneKind::Overview,
        SceneKind::Discovery,
        SceneKind::PlanDiff,
        SceneKind::Review,
        SceneKind::LiveEvents,
        SceneKind::Verification,
        SceneKind::History,
        SceneKind::Unknown,
        SceneKind::Quarantine,
        SceneKind::Reconciliation,
    ] {
        assert!(
            sim_lib_scene::validate_scene(&EstateSurfaceCodec::encode(kind, None, vec![])).is_ok()
        );
    }
}
#[test]
fn stale_operation_refuses() {
    let old = Key("old".into());
    let new = Key("new".into());
    assert!(EstateSurfaceCodec::decode(&old, &new, Edit::Review).is_err());
}
#[test]
fn operation_contains_no_command_text() {
    let key = Key("plan".into());
    let call = EstateSurfaceCodec::decode(
        &key,
        &key,
        Edit::Apply {
            approval: Key("approval".into()),
        },
    )
    .unwrap();
    assert_eq!(
        call,
        Call::Apply {
            plan: key,
            approval: Key("approval".into())
        }
    );
}
