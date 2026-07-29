use std::sync::Arc;

use sim_citizen::CitizenRuntime;
use sim_kernel::{CapabilityName, Cx, Expr, ObjectCompat, Symbol};
use sim_lib_intent::{Origin, intent};
use sim_lib_interference_core::{SamplingCertificate, SamplingThresholds};
use sim_lib_interference_runtime::{
    SamplingCertificateDescriptor, StudyDescriptor, install_interference_records,
};
use sim_lib_interference_solve::Observable;
use sim_lib_view::{DispatchContext, DispatchReason, LensRegistry, SurfaceCodec, surface};
use sim_value::{access, build};

use crate::scene::{ProjectionOptions, study_scene_with};
use crate::{
    INTERFERENCE_PROJECT_CAPABILITY, INTERFERENCE_SOLVE_CAPABILITY, INTERFERENCE_SURFACE_CODEC_ID,
    InterferenceSurfaceCodec, MAX_ANIMATION_FRAMES, register_interference_surface,
    surface_interference_codec_symbol,
};

fn test_cx() -> Cx {
    sim_kernel::testing::eager_cx()
}

fn study_expr(cx: &mut Cx) -> Expr {
    let study = StudyDescriptor::example();
    study.as_expr(cx).expect("Study Expr")
}

fn edit(base: &Expr, path: &[&str], value: Expr) -> Expr {
    intent(
        "edit-field",
        Origin::human(7),
        vec![
            ("target", base.clone()),
            (
                "path",
                Expr::List(
                    path.iter()
                        .map(|segment| Expr::Symbol(Symbol::new(*segment)))
                        .collect(),
                ),
            ),
            ("value", value),
        ],
    )
}

#[test]
fn registration_claims_the_exact_study_shape_and_one_codec() {
    let mut cx = test_cx();
    let mut registry = LensRegistry::new();
    register_interference_surface(&mut cx, &mut registry).unwrap();
    assert!(
        registry
            .surface_codec(&surface_interference_codec_symbol())
            .is_some()
    );
    assert_eq!(registry.lenses().len(), 1);
    assert_eq!(
        registry.lenses()[0].meta.id.as_qualified_str(),
        INTERFERENCE_SURFACE_CODEC_ID
    );
    let study_symbol = Symbol::new("ranked-study");
    let study_value = cx
        .factory()
        .opaque(Arc::new(StudyDescriptor::example()))
        .unwrap();
    cx.env_mut().define(study_symbol.clone(), study_value);
    let value = Expr::Symbol(study_symbol);
    let outcome = registry
        .dispatch_view(&mut cx, &value, &DispatchContext::permissive(&grant_all))
        .unwrap();
    assert_eq!(outcome.lens_id, surface_interference_codec_symbol());
    assert!(matches!(outcome.reason, DispatchReason::ShapeMatch(100)));
}

fn grant_all(_: &CapabilityName) -> bool {
    true
}

#[test]
fn certified_scene_contains_controls_summaries_heatmap_badges_and_cross_section() {
    let mut cx = test_cx();
    let value = study_expr(&mut cx);
    let scene = InterferenceSurfaceCodec::new()
        .encode(&mut cx, &value, &surface::preset("desktop").unwrap())
        .unwrap();
    sim_lib_scene::validate_scene(&scene).unwrap();
    for kind in ["field", "slider", "heatmap", "badge", "plot"] {
        assert!(contains_kind(&scene, kind), "missing scene/{kind}");
    }
    for text in [
        "interference-controls",
        "interference-sources",
        "interference-evidence",
        "projection-certificate",
        "sampling",
        "contrast",
    ] {
        assert!(format!("{scene:?}").contains(text), "missing {text}");
    }
}

#[test]
fn surface_budget_is_met_only_through_domain_detector_reduction() {
    let mut cx = test_cx();
    let value = study_expr(&mut cx);
    let mut caps = surface::preset("desktop").unwrap();
    set_display_limit(&mut caps, "heatmap-max-cells", 1);
    set_display_limit(&mut caps, "heatmap-max-bytes", 4096);
    let scene = InterferenceSurfaceCodec::new()
        .encode(&mut cx, &value, &caps)
        .unwrap();
    let heatmap = find_kind(&scene, "heatmap").unwrap();
    assert_eq!(number_field(heatmap, "rows"), 1);
    assert_eq!(number_field(heatmap, "cols"), 1);
    let certificate = access::field(heatmap, "projection-certificate").unwrap();
    assert_eq!(
        access::field_sym(certificate, "detector")
            .unwrap()
            .name
            .as_ref(),
        "detector-scalar-area-mean"
    );
    assert_eq!(number_field(certificate, "footprint-max-rows"), 2);
    assert!(
        access::field_str(heatmap, "advisory")
            .unwrap()
            .contains("sampling")
    );
}

#[test]
fn project_and_model_edits_preserve_real_base_proposed_and_authority() {
    let mut cx = test_cx();
    install_interference_records(&mut cx).unwrap();
    let codec = InterferenceSurfaceCodec::new();
    let base = study_expr(&mut cx);

    let project = codec
        .decode(
            &mut cx,
            &base,
            &edit(&base, &["observable"], Expr::Symbol(Symbol::new("phase"))),
        )
        .unwrap();
    assert!(project.committable);
    assert_eq!(project.base, base);
    assert_ne!(project.proposed, project.base);
    let operation = codec.commit(&mut cx, &project).unwrap();
    assert_form_head(&operation.form, "interference", "project");
    assert_eq!(
        operation.required_capabilities[0].as_str(),
        INTERFERENCE_PROJECT_CAPABILITY
    );
    assert!(operation.result_shape.is_some());

    let model = codec
        .decode(
            &mut cx,
            &base,
            &edit(&base, &["frequency"], build::float(2_000.0)),
        )
        .unwrap();
    assert!(model.committable);
    let operation = codec.commit(&mut cx, &model).unwrap();
    assert_form_head(&operation.form, "interference", "solve");
    assert_eq!(
        operation.required_capabilities[0].as_str(),
        INTERFERENCE_SOLVE_CAPABILITY
    );
    assert!(operation.result_shape.is_some());
}

#[test]
fn every_declared_edit_field_classifies_to_the_existing_domain_operation() {
    let mut cx = test_cx();
    let codec = InterferenceSurfaceCodec::new();
    let base = study_expr(&mut cx);
    let project_cases = [
        ("observable", Expr::Symbol(Symbol::new("real"))),
        ("wt", build::float(0.5)),
        ("floor", build::float(0.1)),
        ("palette", Expr::Symbol(Symbol::new("blue-red"))),
        ("cross-section", build::uint(0)),
    ];
    for (field, value) in project_cases {
        let draft = codec
            .decode(&mut cx, &base, &edit(&base, &[field], value))
            .unwrap();
        assert!(draft.committable, "{field}: {:?}", draft.diagnostics);
        assert_form_head(
            &codec.commit(&mut cx, &draft).unwrap().form,
            "interference",
            "project",
        );
    }
    let model_cases = [
        (vec!["frequency"], build::float(1_100.0)),
        (vec!["medium", "attenuation-np-m"], build::float(0.01)),
        (vec!["source", "source", "phase-rad"], build::float(0.25)),
        (vec!["plane", "extent-u-m"], build::float(0.2)),
    ];
    for (path, value) in model_cases {
        let draft = codec
            .decode(&mut cx, &base, &edit(&base, &path, value))
            .unwrap();
        assert!(draft.committable, "{path:?}: {:?}", draft.diagnostics);
        assert_form_head(
            &codec.commit(&mut cx, &draft).unwrap().form,
            "interference",
            "solve",
        );
    }
}

#[test]
fn invalid_edits_are_rejected_and_cannot_commit() {
    let mut cx = test_cx();
    let codec = InterferenceSurfaceCodec::new();
    let base = study_expr(&mut cx);
    let draft = codec
        .decode(
            &mut cx,
            &base,
            &edit(&base, &["frequency"], build::float(-1.0)),
        )
        .unwrap();
    assert!(!draft.committable);
    assert!(draft.diagnostics[0].message.contains("positive"));
    assert!(codec.commit(&mut cx, &draft).is_err());

    let stale_target = Expr::String("different Study".to_owned());
    let submitted = intent(
        "set-param",
        Origin::human(8),
        vec![
            ("target", stale_target),
            ("param", Expr::Symbol(Symbol::new("wt"))),
            ("value", build::float(0.5)),
        ],
    );
    let stale = codec.decode(&mut cx, &base, &submitted).unwrap();
    assert!(!stale.committable);
    assert!(stale.diagnostics[0].message.contains("does not match"));
}

#[test]
fn phase_masks_and_annotated_aliasing_remain_visible() {
    let mut cx = test_cx();
    let mut study = StudyDescriptor::example();
    let problem = study.problem.to_problem().unwrap();
    let plane = study.plane.to_plane().unwrap();
    let thresholds = SamplingThresholds::new(10_000.0, 9_000.0, 0.0, 0.0).unwrap();
    let sampling =
        SamplingCertificate::measure_with_thresholds(&problem, &plane, thresholds).unwrap();
    study.evidence.sampling = SamplingCertificateDescriptor::from_certificate(sampling);
    study.evidence.sampling_policy = Symbol::qualified("interference", "annotate");
    let study =
        StudyDescriptor::new(study.problem, study.plane, study.field, study.evidence).unwrap();
    let scene = study_scene_with(
        &mut cx,
        &study,
        &surface::preset("desktop").unwrap(),
        ProjectionOptions {
            observable: Observable::Phase,
            phase_floor: 0.8,
            palette: "cyclic-phase",
            cross_section: Some(0),
        },
        1,
        None,
    )
    .unwrap();
    let heatmap = find_kind(&scene, "heatmap").unwrap();
    let valid = access::field(heatmap, "valid").unwrap();
    assert!(matches!(valid, Expr::List(items) if items.contains(&Expr::Bool(false))));
    assert!(
        access::field_str(heatmap, "advisory")
            .unwrap()
            .contains("aliased")
    );
    assert!(format!("{scene:?}").contains("sampling aliased"));
}

#[test]
fn animation_is_preprojected_bounded_and_deterministic() {
    let mut cx = test_cx();
    let codec = InterferenceSurfaceCodec::new();
    let value = study_expr(&mut cx);
    let caps = surface::preset("desktop").unwrap();
    let first = codec.encode_animation(&mut cx, &value, &caps, 4).unwrap();
    let second = codec.encode_animation(&mut cx, &value, &caps, 4).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.len(), 4);
    assert!(
        codec
            .encode_animation(&mut cx, &value, &caps, MAX_ANIMATION_FRAMES + 1)
            .is_err()
    );
}

#[test]
fn identical_study_and_caps_produce_identical_scene() {
    let mut cx = test_cx();
    let value = study_expr(&mut cx);
    let caps = surface::preset("desktop").unwrap();
    let codec = InterferenceSurfaceCodec::new();
    assert_eq!(
        codec.encode(&mut cx, &value, &caps).unwrap(),
        codec.encode(&mut cx, &value, &caps).unwrap()
    );
}

fn assert_form_head(expr: &Expr, namespace: &str, name: &str) {
    let Expr::List(items) = expr else {
        panic!("operation is not a call list: {expr:?}");
    };
    assert!(
        matches!(&items[0], Expr::Symbol(symbol) if symbol.namespace.as_deref() == Some(namespace) && symbol.name.as_ref() == name),
        "unexpected operation head: {:?}",
        items[0]
    );
}

fn contains_kind(expr: &Expr, kind: &str) -> bool {
    find_kind(expr, kind).is_some()
}

fn find_kind<'a>(expr: &'a Expr, expected: &str) -> Option<&'a Expr> {
    if matches!(
        sim_lib_scene::node_kind(expr),
        Some(kind) if kind.namespace.as_deref() == Some("scene") && kind.name.as_ref() == expected
    ) {
        return Some(expr);
    }
    match expr {
        Expr::Map(entries) => entries.iter().find_map(|(key, value)| {
            find_kind(key, expected).or_else(|| find_kind(value, expected))
        }),
        Expr::List(items) | Expr::Vector(items) | Expr::Set(items) => {
            items.iter().find_map(|item| find_kind(item, expected))
        }
        _ => None,
    }
}

fn set_display_limit(caps: &mut sim_lib_view::SurfaceCaps, field: &str, value: u64) {
    let Expr::Map(entries) = &mut caps.display else {
        panic!("display caps must be a map")
    };
    if let Some((_, found)) = entries.iter_mut().find(
        |(key, _)| matches!(key, Expr::Symbol(symbol) if symbol.namespace.is_none() && symbol.name.as_ref() == field),
    ) {
        *found = build::uint(value);
    } else {
        entries.push((Expr::Symbol(Symbol::new(field)), build::uint(value)));
    }
}

fn number_field(expr: &Expr, field: &str) -> u64 {
    let Expr::Number(number) = access::field(expr, field).unwrap() else {
        panic!("{field} is not a number")
    };
    number.canonical.parse().unwrap()
}
