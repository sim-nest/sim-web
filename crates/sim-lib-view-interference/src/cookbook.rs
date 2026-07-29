//! Checked end-to-end builder for the interference surface recipe.

use std::sync::Arc;

use sim_kernel::{
    Args, CapabilityName, Consistency, Cx, DefaultFactory, EagerPolicy, Error, EvalFabric,
    EvalMode, EvalReply, EvalRequest, Expr, ObjectCompat, Result, Symbol, Value,
};
use sim_lib_intent::{Origin, intent};
use sim_lib_interference_core::{
    Emitter, FieldAmplitude, Hertz, InterferenceProblem, MetresPerSecond, NepersPerMetre, Point3M,
    PositiveMetres, Radians, SamplingPlane, ScalarMedium, SourceSet, UnitVector3,
};
use sim_lib_interference_runtime::{
    InterferenceLib, InterferenceRecordsLib, ScalarProjectionDescriptor, StudyDescriptor,
};
use sim_lib_interference_solve::ReferencePhasorSolver;
use sim_lib_view::{Operation, SurfaceCodec, surface};
use sim_value::{access, build};

use crate::{
    INTERFERENCE_PROJECT_CAPABILITY, INTERFERENCE_SOLVE_CAPABILITY, InterferenceSurfaceCodec,
};

/// Runs the complete checked recipe and returns its stable report lines.
///
/// The recipe encodes a solved study, realizes one projection edit and one
/// model edit as ordinary [`Operation`] values, refreshes the Scene from the
/// realized study, then proves domain detector reduction against explicit
/// desktop, phone, and glance budgets.
pub fn interference_study_demo() -> Result<Vec<String>> {
    let mut cx = recipe_cx()?;
    let codec = InterferenceSurfaceCodec::new();
    let study = solved_study(96, 96)?;
    let base = study.as_expr(&mut cx)?;
    let initial = codec.encode(&mut cx, &base, &surface::preset("desktop").unwrap())?;
    let mut lines = vec![format!(
        "encode={} evidence=Study/{}",
        scene_kind(&initial),
        study.evidence.sampling.verdict
    )];

    let project = checked_operation(
        &codec,
        &mut cx,
        &base,
        edit(&base, &["observable"], Expr::Symbol(Symbol::new("phase"))),
    )?;
    let projected = realize(&mut cx, &project)?;
    let projection = projected
        .object()
        .downcast_ref::<ScalarProjectionDescriptor>()
        .expect("projection Operation result_shape admitted the runtime record");
    lines.push(format!(
        "project-edit={} realized=interference/Projection target={}x{}",
        operation_head(&project.form),
        projection.rows,
        projection.columns
    ));

    let model = checked_operation(
        &codec,
        &mut cx,
        &base,
        edit(&base, &["frequency"], build::float(2.0)),
    )?;
    let refreshed = realize(&mut cx, &model)?;
    let refreshed_study = refreshed
        .object()
        .downcast_ref::<StudyDescriptor>()
        .expect("model Operation result_shape admitted the runtime Study");
    let refreshed_expr = refreshed_study.as_expr(&mut cx)?;
    let refreshed_scene = codec.encode(
        &mut cx,
        &refreshed_expr,
        &surface::preset("desktop").unwrap(),
    )?;
    lines.push(format!(
        "model-edit={} frequency-hz={} refreshed={}",
        operation_head(&model.form),
        refreshed_study.problem.frequency_hz,
        scene_kind(&refreshed_scene)
    ));

    for (preset, budget) in [("desktop", 1_024_u64), ("phone", 256), ("watch", 64)] {
        let mut caps = surface::preset(preset).expect("published surface preset");
        set_display_limit(&mut caps, "heatmap-max-cells", budget);
        set_display_limit(&mut caps, "heatmap-max-bytes", 64 * 1_024);
        let scene = codec.encode(&mut cx, &refreshed_expr, &caps)?;
        let heatmap = find_kind(&scene, "heatmap").expect("interference Scene heatmap");
        let certificate =
            access::field(heatmap, "projection-certificate").expect("projection certificate");
        let target_rows = integer_field(heatmap, "rows");
        let target_columns = integer_field(heatmap, "cols");
        let cells = target_rows * target_columns;
        assert!(cells <= budget, "projected cells exceed the surface budget");
        let surface_name = if preset == "watch" { "glance" } else { preset };
        lines.push(format!(
            "surface={surface_name} source={}x{} target={target_rows}x{target_columns} cells={cells}/{budget} detector={}",
            study.field.rows,
            study.field.cols,
            access::field_sym(certificate, "detector")
                .expect("detector rule")
        ));
    }
    Ok(lines)
}

fn recipe_cx() -> Result<Cx> {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.load_lib(&InterferenceRecordsLib)?;
    cx.load_lib(&InterferenceLib)?;
    cx.grant(sim_kernel::read_construct_capability());
    cx.grant(CapabilityName::new(INTERFERENCE_PROJECT_CAPABILITY));
    cx.grant(CapabilityName::new(INTERFERENCE_SOLVE_CAPABILITY));
    Ok(cx)
}

fn solved_study(rows: usize, columns: usize) -> Result<StudyDescriptor> {
    let point = |x, y, z| Point3M::from_metres(x, y, z);
    let problem = InterferenceProblem::new(
        Hertz::new(1.0).expect("positive recipe frequency"),
        ScalarMedium::new(
            MetresPerSecond::new(100.0).expect("positive recipe wave speed"),
            NepersPerMetre::new(0.0).expect("finite recipe attenuation"),
        ),
        domain(SourceSet::new(vec![Emitter::ForwardPlane {
            id: "recipe-plane".to_owned(),
            through: domain(point(0.0, 0.0, 0.0))?,
            direction: domain(UnitVector3::new(0.0, 0.0, 1.0))?,
            amplitude: domain(FieldAmplitude::new(2.0))?,
            phase: domain(Radians::new(0.0))?,
        }]))?,
        domain(PositiveMetres::new(0.001))?,
    );
    let plane = domain(SamplingPlane::new(
        domain(point(0.0, 0.0, 1.0))?,
        domain(UnitVector3::new(1.0, 0.0, 0.0))?,
        domain(UnitVector3::new(0.0, 1.0, 0.0))?,
        domain(PositiveMetres::new(1.0))?,
        domain(PositiveMetres::new(1.0))?,
        rows,
        columns,
    ))?;
    let (field, evidence) = domain(ReferencePhasorSolver::default().solve(&problem, &plane))?;
    StudyDescriptor::from_reference(&problem, plane, field, &evidence)
}

fn domain<T, E: std::fmt::Debug>(result: std::result::Result<T, E>) -> Result<T> {
    result.map_err(|error| Error::Eval(format!("interference recipe fixture failed: {error:?}")))
}

fn checked_operation(
    codec: &InterferenceSurfaceCodec,
    cx: &mut Cx,
    base: &Expr,
    submitted: Expr,
) -> Result<Operation> {
    let draft = codec.decode(cx, base, &submitted)?;
    assert!(draft.committable, "recipe edit must be committable");
    codec.commit(cx, &draft)
}

fn realize(cx: &mut Cx, operation: &Operation) -> Result<sim_kernel::Value> {
    let reply = RecipeFabric.realize(
        cx,
        EvalRequest {
            expr: operation.form.clone(),
            result_shape: operation.result_shape.clone(),
            required_capabilities: operation.required_capabilities.clone(),
            deadline: None,
            consistency: Consistency::LocalOnly,
            mode: EvalMode::Eval,
            answer_limit: None,
            stream_buffer: None,
            stream: false,
            trace: false,
        },
    )?;
    Ok(reply.value)
}

struct RecipeFabric;

impl EvalFabric for RecipeFabric {
    fn realize(&self, cx: &mut Cx, request: EvalRequest) -> Result<EvalReply> {
        for capability in &request.required_capabilities {
            cx.require(capability)?;
        }
        let value = evaluate_operation(cx, request.expr)?;
        if let Some(shape) = request.result_shape {
            let shape = shape.object().as_shape().expect("registered Shape value");
            let matched = shape.check_value(cx, value.clone())?;
            assert!(
                matched.accepted,
                "realized result must satisfy Operation shape"
            );
        }
        Ok(EvalReply {
            value,
            diagnostics: cx.take_diagnostics(),
            trace: None,
        })
    }
}

fn evaluate_operation(cx: &mut Cx, form: Expr) -> Result<Value> {
    let Expr::Call { operator, args } = form else {
        return Err(Error::Eval(
            "interference recipe Operation must be an evaluable call".to_owned(),
        ));
    };
    let Expr::Symbol(function) = operator.as_ref() else {
        return Err(Error::Eval(
            "interference recipe Operation requires a symbol operator".to_owned(),
        ));
    };
    let args = args
        .iter()
        .map(|arg| operation_argument(cx, arg))
        .collect::<Result<Vec<_>>>()?;
    cx.call_function(function, Args::new(args))
}

fn operation_argument(cx: &mut Cx, expr: &Expr) -> Result<Value> {
    let Expr::Extension { tag, payload } = expr else {
        return cx.eval_expr(expr.clone());
    };
    if *tag != Symbol::qualified("citizen", "read-construct") {
        return cx.eval_expr(expr.clone());
    }
    let Expr::Vector(parts) = payload.as_ref() else {
        return Err(Error::Eval(
            "citizen read-construct payload must be a vector".to_owned(),
        ));
    };
    let Some((Expr::Symbol(class), args)) = parts.split_first() else {
        return Err(Error::Eval(
            "citizen read-construct must begin with a class symbol".to_owned(),
        ));
    };
    let args = args
        .iter()
        .map(|arg| sim_citizen::value_from_expr(cx, arg))
        .collect::<Result<Vec<_>>>()?;
    cx.read_construct(class, args)
}

fn edit(base: &Expr, path: &[&str], value: Expr) -> Expr {
    intent(
        "edit-field",
        Origin::human(17),
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

fn operation_head(form: &Expr) -> &str {
    let Expr::Call { operator, .. } = form else {
        panic!("recipe Operation must be an evaluable call");
    };
    let Expr::Symbol(symbol) = operator.as_ref() else {
        panic!("recipe Operation call must have a symbol operator");
    };
    symbol.name.as_ref()
}

fn scene_kind(scene: &Expr) -> String {
    sim_lib_scene::node_kind(scene)
        .expect("recipe output is a Scene")
        .to_string()
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

fn integer_field(expr: &Expr, field: &str) -> u64 {
    let Expr::Number(number) = access::field(expr, field).expect("integer field") else {
        panic!("{field} is not a number")
    };
    number.canonical.parse().expect("canonical integer")
}
