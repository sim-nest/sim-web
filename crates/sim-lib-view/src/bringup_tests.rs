use sim_kernel::{Error, Expr, Symbol};
use sim_value::access;

use crate::{
    BringUpLedger, GLASSES_BRINGUP_FIXTURE, GLASSES_BRINGUP_FIXTURE_TEXT, GLASSES_BRINGUP_KIND,
    GLASSES_BRINGUP_LANES, GLASSES_BRINGUP_NAMESPACE, VITURE_CARINA_LANE,
    default_glasses_bringup_fixture, glasses_bringup_fixture, glasses_bringup_fixture_names,
};

#[test]
fn bringup_lanes_default_unverified_and_gate_enable() {
    assert_eq!(glasses_bringup_fixture_names(), [GLASSES_BRINGUP_FIXTURE]);
    assert_eq!(
        glasses_bringup_fixture(GLASSES_BRINGUP_FIXTURE),
        Some(default_glasses_bringup_fixture())
    );
    assert!(glasses_bringup_fixture("unknown").is_none());

    let fixture = default_glasses_bringup_fixture();
    assert_eq!(
        access::field(&fixture, "kind"),
        Some(&Expr::Symbol(Symbol::qualified(
            GLASSES_BRINGUP_NAMESPACE,
            GLASSES_BRINGUP_KIND
        )))
    );
    assert_eq!(
        access::field(&fixture, "fixture"),
        Some(&Expr::String(GLASSES_BRINGUP_FIXTURE.to_owned()))
    );
    let lanes = access::field(&fixture, "lanes").expect("lanes map");
    let Expr::Map(lane_entries) = lanes else {
        panic!("fixture lanes must be a map");
    };
    assert_eq!(lane_entries.len(), GLASSES_BRINGUP_LANES.len());
    for lane in GLASSES_BRINGUP_LANES {
        assert!(
            GLASSES_BRINGUP_FIXTURE_TEXT.contains(&format!(":{lane}")),
            "committed fixture names {lane}"
        );
    }
    assert_eq!(
        GLASSES_BRINGUP_FIXTURE_TEXT
            .matches(":verified false")
            .count(),
        GLASSES_BRINGUP_LANES.len()
    );

    let mut ledger = BringUpLedger::from_expr(&fixture).expect("fixture parses");
    for lane in GLASSES_BRINGUP_LANES {
        let entry = ledger.entry(lane).expect("lane entry exists");
        assert!(!entry.verified, "{lane} defaults to unverified");
        assert!(entry.firmware.is_none(), "{lane} has no default firmware");
        assert!(entry.version.is_none(), "{lane} has no default version");
        assert!(matches!(entry.claims, Expr::Map(_)), "{lane} has claims");
        assert_lane_not_verified(&ledger, lane);
    }

    let verified = ledger
        .entry_mut(VITURE_CARINA_LANE)
        .expect("Viture Carina lane exists");
    verified.verified = true;
    verified.firmware = Some("carina-verified".to_owned());
    verified.version = Some("bringup-1".to_owned());

    ledger
        .enable_lane(VITURE_CARINA_LANE)
        .expect("verified lane enables");
    for lane in GLASSES_BRINGUP_LANES
        .iter()
        .copied()
        .filter(|lane| *lane != VITURE_CARINA_LANE)
    {
        assert_lane_not_verified(&ledger, lane);
    }
}

#[test]
fn bringup_fixture_round_trips_and_requires_all_lanes() {
    let ledger = BringUpLedger::from_expr(&default_glasses_bringup_fixture()).unwrap();
    assert_eq!(
        BringUpLedger::from_expr(&ledger.to_expr()).unwrap(),
        ledger,
        "ledger expression round-trips"
    );

    let mut incomplete = ledger.clone();
    incomplete.entries.pop();
    let error = BringUpLedger::from_expr(&incomplete.to_expr()).unwrap_err();
    assert!(
        error.to_string().contains("missing lane"),
        "unexpected error: {error}"
    );
}

fn assert_lane_not_verified(ledger: &BringUpLedger, lane: &str) {
    match ledger.enable_lane(lane) {
        Err(Error::HostError(message)) => assert_eq!(message, format!("lane {lane} not verified")),
        other => panic!("expected lane-not-verified error for {lane}, got {other:?}"),
    }
}
