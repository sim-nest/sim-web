use super::{
    Attention, AuthorityIntersection, ContinuityOutcome, DisclosureField, ExpeditionProjector,
    SemanticClutch, SurfaceRole,
};

fn authority() -> AuthorityIntersection {
    AuthorityIntersection {
        mission: true,
        passport: true,
        route_lease: true,
        endpoint_grant: true,
    }
}

fn public_field() -> DisclosureField {
    DisclosureField {
        name: "next-step".into(),
        value: "continue north conceptually".into(),
        declared: true,
        sensitive: false,
        reviewed: true,
    }
}

#[test]
fn every_authority_factor_is_required() {
    for deny in 0..4 {
        let mut evidence = authority();
        match deny {
            0 => evidence.mission = false,
            1 => evidence.passport = false,
            2 => evidence.route_lease = false,
            _ => evidence.endpoint_grant = false,
        }
        assert!(matches!(
            ExpeditionProjector.project(
                "content:expedition",
                SurfaceRole::PhoneScene,
                evidence,
                Attention::VisibleExpedition,
                &[public_field()],
                None,
            ),
            ContinuityOutcome::Refused { .. }
        ));
    }
}

#[test]
fn reduced_roles_refuse_every_sensitive_class() {
    for role in [
        SurfaceRole::Glance,
        SurfaceRole::Audible,
        SurfaceRole::Notification,
        SurfaceRole::LockScreen,
    ] {
        for mut field in [
            DisclosureField {
                name: "secret".into(),
                ..public_field()
            },
            DisclosureField {
                name: "private-note".into(),
                ..public_field()
            },
            DisclosureField {
                reviewed: false,
                ..public_field()
            },
            DisclosureField {
                declared: false,
                ..public_field()
            },
        ] {
            field.sensitive |= matches!(field.name.as_str(), "secret" | "private-note");
            assert!(matches!(
                ExpeditionProjector.project(
                    "content:expedition",
                    role,
                    authority(),
                    Attention::VisibleExpedition,
                    &[field],
                    None,
                ),
                ContinuityOutcome::Refused { .. }
            ));
        }
    }
}

#[test]
fn semantic_clutch_changes_visible_focus_without_device_data() {
    let keyboard = ExpeditionProjector.project(
        "content:expedition",
        SurfaceRole::PhoneScene,
        authority(),
        Attention::VisibleExpedition,
        &[public_field()],
        Some(&SemanticClutch::Keyboard("open-book".into())),
    );
    let touch = ExpeditionProjector.project(
        "content:expedition",
        SurfaceRole::PhoneScene,
        authority(),
        Attention::VisibleExpedition,
        &[public_field()],
        Some(&SemanticClutch::Touch("open-book".into())),
    );
    assert_eq!(keyboard, touch);
}

#[test]
fn no_endpoint_identity_is_needed_and_all_continuity_identities_match() {
    let expected = "content:expedition-immutable";
    for role in [SurfaceRole::PhoneScene, SurfaceRole::Glance] {
        let outcome = ExpeditionProjector.project(
            expected,
            role,
            authority(),
            Attention::VisibleExpedition,
            &[public_field()],
            None,
        );
        let ContinuityOutcome::Projected(projection) = outcome else {
            panic!("projected")
        };
        let expedition = projection.content_id.clone();
        let journal = projection.content_id.clone();
        let book = projection.content_id.clone();
        let closure = projection.content_id.clone();
        let adopted_recipe = projection.content_id;
        assert_eq!(
            [expedition, journal, book, closure, adopted_recipe],
            [expected; 5]
        );
    }
    assert_eq!(
        ExpeditionProjector.stop(expected),
        ContinuityOutcome::Stopped {
            content_id: expected.into()
        }
    );
}
