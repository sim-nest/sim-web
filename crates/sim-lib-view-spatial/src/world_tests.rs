use sim_kernel::{Expr, Symbol};
use sim_lib_scene::{AnchorSpace, Transform3};

use crate::{
    PanelPlacement, VioTrackingStatus, WorldAnchorObservation, WorldAnchorResolver,
    resolve_world_anchor,
};

#[test]
fn world_anchor_falls_back_when_vio_unstable() {
    let placement = panel_placement();
    let resolver = resolver();

    let stable = resolve_world_anchor(&placement, VioTrackingStatus::Stable6Dof, &resolver);
    assert_eq!(stable.anchor_space(), AnchorSpace::World);
    assert_eq!(stable.anchor(), &Symbol::new("desk-plane"));
    assert_transform_near(stable.transform(), [0.2, 1.1, -1.7]);

    let unstable = resolve_world_anchor(&placement, VioTrackingStatus::Limited, &resolver);
    assert_eq!(unstable.anchor_space(), AnchorSpace::Head);
    assert_eq!(unstable.transform(), &placement.transform);
    assert_eq!(
        unstable.reason(),
        Some(&Symbol::qualified("world-anchor", "unstable-vio"))
    );

    let restored = resolve_world_anchor(&placement, VioTrackingStatus::Stable6Dof, &resolver);
    assert_eq!(restored, stable);
}

#[test]
fn missing_world_anchor_degrades_to_head_locked() {
    let placement = panel_placement();
    let resolver = WorldAnchorResolver::default();

    let resolution = resolve_world_anchor(&placement, VioTrackingStatus::Stable6Dof, &resolver);

    assert_eq!(resolution.anchor_space(), AnchorSpace::Head);
    assert_eq!(
        resolution.reason(),
        Some(&Symbol::qualified("world-anchor", "missing-world-anchor"))
    );
}

#[test]
fn tracking_status_reads_shared_xr_symbols() {
    assert_eq!(
        VioTrackingStatus::from_expr(&Expr::Symbol(Symbol::qualified(
            "stream/xr-tracking",
            "tracked"
        )))
        .unwrap(),
        VioTrackingStatus::Stable6Dof
    );
    assert_eq!(
        VioTrackingStatus::Limited.to_expr(),
        Expr::Symbol(Symbol::qualified("stream/xr-tracking", "limited"))
    );
}

fn panel_placement() -> PanelPlacement {
    PanelPlacement::new(
        Symbol::new("main"),
        AnchorSpace::World,
        Transform3::new([0.2, 0.1, -0.1], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
    )
    .with_world_anchor(Symbol::new("desk-plane"))
}

fn resolver() -> WorldAnchorResolver {
    WorldAnchorResolver::new([WorldAnchorObservation::new(
        Symbol::new("desk-plane"),
        Transform3::new([0.0, 1.0, -1.6], [0.0, 0.0, 0.0, 1.0], [1.0, 1.0, 1.0]),
    )])
}

fn assert_transform_near(transform: &Transform3, expected: [f64; 3]) {
    for (actual, expected) in transform.translate_m.into_iter().zip(expected) {
        assert!((actual - expected).abs() < 0.000_001);
    }
}
