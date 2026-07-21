use sim_kernel::{Expr, Symbol};
use sim_lib_intent::{Origin, field, intent_kind_of, validate_intent};
use sim_value::build;

use crate::{
    AssignedWornRole, FleetSensorQuorum, FleetSensorSample, FleetWristInput, SwipeDirection,
    TwoHandedTiming, WatchFleetMember, WornActivity, WornRole, WristSide, assign_worn_roles,
    fleet_sensor_quorum, two_handed_intent, watch_acknowledge_op, watch_palette_symbol,
};

#[test]
fn role_assignment_prefers_charged_watch_for_field() {
    let left = WatchFleetMember::new(WristSide::Left, 92, false, WornActivity::Active).unwrap();
    let right = WatchFleetMember::new(WristSide::Right, 41, true, WornActivity::Active).unwrap();

    let roles = assign_worn_roles(&left, &right).unwrap();

    assert_eq!(roles.field_side(), Some(WristSide::Right));
    assert_eq!(
        roles.left,
        AssignedWornRole {
            side_role: WornRole::LeftWrist,
            duty_role: WornRole::Backup,
        }
    );
    assert_eq!(
        roles.right,
        AssignedWornRole {
            side_role: WornRole::RightWrist,
            duty_role: WornRole::Field,
        }
    );
}

#[test]
fn role_assignment_preserves_sleep_quiet_and_off_body_roles() {
    let left = WatchFleetMember::new(WristSide::Left, 80, false, WornActivity::Sleep).unwrap();
    let right = WatchFleetMember::new(WristSide::Right, 55, false, WornActivity::OffBody).unwrap();

    let roles = assign_worn_roles(&left, &right).unwrap();

    assert_eq!(roles.field_side(), None);
    assert_eq!(roles.left.duty_role, WornRole::Sleep);
    assert_eq!(roles.right.duty_role, WornRole::OffBodyBeacon);
}

#[test]
fn divergent_hr_lowers_confidence() {
    let sensor = Symbol::qualified("stream/worn-sensor", "heart-rate");
    let left = FleetSensorSample::new(WristSide::Left, sensor.clone(), 72, 9_600).unwrap();
    let right = FleetSensorSample::new(WristSide::Right, sensor.clone(), 94, 8_800).unwrap();

    let quorum = fleet_sensor_quorum(&left, &right, 5).unwrap();

    assert_eq!(
        quorum,
        FleetSensorQuorum::LowConfidence {
            sensor,
            prefer: WristSide::Left,
            value: 72,
            delta: 22,
            confidence: 4_400,
        }
    );
    assert!(quorum.confidence() < left.confidence());
}

#[test]
fn two_handed_ack_and_hr_quorum() {
    let sensor = Symbol::qualified("stream/worn-sensor", "heart-rate");
    let left_hr = FleetSensorSample::new(WristSide::Left, sensor.clone(), 72, 9_600).unwrap();
    let right_hr = FleetSensorSample::new(WristSide::Right, sensor.clone(), 94, 8_800).unwrap();

    assert_eq!(
        fleet_sensor_quorum(&left_hr, &right_hr, 5).unwrap(),
        FleetSensorQuorum::LowConfidence {
            sensor,
            prefer: WristSide::Left,
            value: 72,
            delta: 22,
            confidence: 4_400,
        }
    );

    let intent = two_handed_intent(
        &FleetWristInput::tap(1_000),
        &FleetWristInput::tap(1_180),
        Origin::human(7),
        build::sym("watch-pane"),
        build::sym("rich-surface"),
        TwoHandedTiming::default(),
    )
    .expect("two taps acknowledge");

    assert_eq!(kind_name(&intent), "invoke");
    assert_eq!(
        field(&intent, "op"),
        Some(&Expr::Symbol(watch_acknowledge_op()))
    );
    validate_intent(&intent).expect("acknowledge is a valid Intent");
}

#[test]
fn long_press_plus_swipe_cancels_and_dual_raise_opens_palette() {
    let cancel = two_handed_intent(
        &FleetWristInput::swipe(SwipeDirection::Left, 2_000),
        &FleetWristInput::long_press(900, 2_160),
        Origin::human(8),
        build::sym("watch-pane"),
        build::sym("rich-surface"),
        TwoHandedTiming::default(),
    )
    .expect("swipe and long press cancel");
    assert_eq!(kind_name(&cancel), "cancel");
    assert_eq!(field(&cancel, "pane"), Some(&build::sym("watch-pane")));
    validate_intent(&cancel).expect("cancel is a valid Intent");

    let open = two_handed_intent(
        &FleetWristInput::raised(220, 3_000),
        &FleetWristInput::raised(240, 3_120),
        Origin::human(9),
        build::sym("watch-pane"),
        build::sym("rich-surface"),
        TwoHandedTiming::default(),
    )
    .expect("dual raise opens palette");
    assert_eq!(kind_name(&open), "open");
    assert_eq!(
        field(&open, "value"),
        Some(&Expr::Symbol(watch_palette_symbol()))
    );
    assert_eq!(field(&open, "pane"), Some(&build::sym("rich-surface")));
    validate_intent(&open).expect("open palette is a valid Intent");
}

fn kind_name(intent: &Expr) -> String {
    intent_kind_of(intent)
        .expect("intent kind")
        .name
        .to_string()
}
