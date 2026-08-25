use sim_kernel::{Expr, Symbol};
use sim_lib_intent::{field, intent_kind_of, validate_intent};
use sim_lib_scene::{GlanceAction, GlanceCard, GlanceMetric};
use sim_value::build;

use crate::{
    EndpointAction, EndpointCandidate, EndpointRole, OptionalEndpointRoleAdapter, RoleAuthority,
};

fn candidate(id: &str) -> EndpointCandidate {
    EndpointCandidate {
        id: Symbol::new(id),
        roles: vec![EndpointRole::Glance, EndpointRole::SemanticAction],
    }
}

fn authority(generation: u64) -> RoleAuthority {
    RoleAuthority {
        connected: true,
        session_consent: true,
        visible_focus: true,
        role_admitted: true,
        provider_evidence: true,
        route_present: true,
        root_present: true,
        route_generation: generation,
        route_expires_at_ms: 200,
        now_ms: 100,
    }
}

fn filtered_glance() -> Expr {
    GlanceCard::new(
        "Ready",
        Some(GlanceMetric::new("status", "filtered")),
        Some(GlanceAction::new("Continue", build::sym("continue"))),
        "info",
        4,
    )
    .to_scene()
}

#[test]
fn projection_is_one_glance_and_only_bounded_semantic_actions() {
    let mut adapter = OptionalEndpointRoleAdapter::default();
    let projection = adapter
        .project(&candidate("watch"), &filtered_glance(), authority(7))
        .unwrap();
    assert_eq!(projection.glance, filtered_glance());

    for action in [
        EndpointAction::Primary,
        EndpointAction::Acknowledge,
        EndpointAction::Defer,
        EndpointAction::Cancel,
    ] {
        let intent = adapter
            .intent_for(&projection, action, authority(7), 12)
            .unwrap();
        validate_intent(&intent).unwrap();
        let kind = intent_kind_of(&intent).unwrap();
        assert!(matches!(kind.name.as_ref(), "invoke" | "cancel"));
        if let Some(Expr::Symbol(op)) = field(&intent, "op") {
            assert_eq!(op.namespace.as_deref(), Some("continuity/action"));
            assert!(!op.name.contains("pointer"));
            assert!(!op.name.contains("effect"));
        }
    }
    let stop = adapter
        .intent_for(&projection, EndpointAction::Stop, authority(7), 13)
        .unwrap();
    validate_intent(&stop).unwrap();
    assert!(
        adapter
            .intent_for(&projection, EndpointAction::Acknowledge, authority(7), 14)
            .is_err()
    );
}

#[test]
fn every_authority_loss_and_prior_generation_fails_closed() {
    let mut losses: Vec<fn(&mut RoleAuthority)> = vec![
        |a| a.connected = false,
        |a| a.session_consent = false,
        |a| a.visible_focus = false,
        |a| a.role_admitted = false,
        |a| a.provider_evidence = false,
        |a| a.route_present = false,
        |a| a.root_present = false,
        |a| a.now_ms = a.route_expires_at_ms,
    ];
    for lose in losses.drain(..) {
        let mut adapter = OptionalEndpointRoleAdapter::default();
        let projection = adapter
            .project(&candidate("optional"), &filtered_glance(), authority(3))
            .unwrap();
        let mut lost = authority(3);
        lose(&mut lost);
        assert!(
            adapter
                .intent_for(&projection, EndpointAction::Acknowledge, lost, 1)
                .is_err()
        );
    }

    let mut adapter = OptionalEndpointRoleAdapter::default();
    let old = adapter
        .project(&candidate("optional"), &filtered_glance(), authority(3))
        .unwrap();
    let _new = adapter
        .project(&candidate("optional"), &filtered_glance(), authority(4))
        .unwrap();
    assert!(
        adapter
            .intent_for(&old, EndpointAction::Acknowledge, authority(3), 2)
            .is_err()
    );
}

#[test]
fn phone_watch_and_fictional_future_candidate_share_scene_and_intent_identity() {
    let scene = filtered_glance();
    let mut identities = Vec::new();
    for id in ["phone", "watch", "fictional-future-halo"] {
        let mut adapter = OptionalEndpointRoleAdapter::default();
        let projection = adapter
            .project(&candidate(id), &scene, authority(11))
            .unwrap();
        let intent = adapter
            .intent_for(&projection, EndpointAction::Primary, authority(11), 9)
            .unwrap();
        identities.push((projection.glance, intent));
    }
    assert_eq!(identities[0], identities[1]);
    assert_eq!(identities[1], identities[2]);
}

#[test]
fn adapter_retains_no_endpoint_content_or_private_truth() {
    assert_eq!(core::mem::size_of::<OptionalEndpointRoleAdapter>(), 16);
    let inadmissible = EndpointCandidate {
        id: Symbol::new("display-only"),
        roles: vec![EndpointRole::Glance],
    };
    assert!(
        OptionalEndpointRoleAdapter::default()
            .project(&inadmissible, &filtered_glance(), authority(1))
            .is_err()
    );
}
