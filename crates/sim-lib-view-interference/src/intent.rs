//! Intent classification and compilation to existing interference operations.

use sim_citizen::CitizenRuntime;
use sim_kernel::{Cx, Error, Expr, Result, Symbol};
use sim_lib_interference_runtime::{
    EmitterDescriptor, MediumDescriptor, PlaneDescriptor, ProblemDescriptor,
    ProjectionRequestDescriptor, StudyDescriptor, project_function_symbol, solve_function_symbol,
};
use sim_lib_interference_solve::{Observable, ReductionRule};
use sim_lib_view_math::HEATMAP_PALETTES;
use sim_value::{access, build};

#[derive(Clone)]
pub(crate) enum EditClass {
    Projection(ProjectionEdit),
    Model(ModelEdit),
}

#[derive(Clone)]
pub(crate) struct ProjectionEdit {
    observable: Observable,
    phase_floor: f64,
    target_rows: usize,
    target_columns: usize,
    reduction: ReductionRule,
    palette: String,
    cross_section: usize,
    frames: usize,
}

#[derive(Clone)]
pub(crate) struct ModelEdit {
    problem: ProblemDescriptor,
    plane: PlaneDescriptor,
    sampling: Symbol,
}

pub(crate) fn decode_study(cx: &mut Cx, expr: &Expr) -> Result<StudyDescriptor> {
    decode_citizen(cx, expr, "interference surface Study")
}

pub(crate) fn classify_edit(
    _cx: &mut Cx,
    study: &StudyDescriptor,
    base: &Expr,
    intent: &Expr,
) -> Result<EditClass> {
    let (path, value) = intent_path_and_value(intent)?;
    let target = access::field(intent, "target")
        .ok_or_else(|| Error::Eval("interference edit is missing target".to_owned()))?;
    if target != base {
        return Err(Error::Eval(
            "interference edit target does not match the rendered Study".to_owned(),
        ));
    }
    let root = path
        .first()
        .ok_or_else(|| Error::Eval("interference edit path cannot be empty".to_owned()))?;
    match root.as_str() {
        "observable" | "wt" | "floor" | "palette" | "cross-section" | "detector" | "frames" => {
            projection_edit(study, root, value)
        }
        "frequency" | "medium" | "source" | "plane" => model_edit(study, &path, value),
        _ => Err(Error::Eval(format!(
            "unsupported interference edit path {}",
            path.join(".")
        ))),
    }
}

fn projection_edit(study: &StudyDescriptor, field: &str, value: &Expr) -> Result<EditClass> {
    let mut edit = ProjectionEdit {
        observable: Observable::Amplitude,
        phase_floor: 0.0,
        target_rows: study.plane.rows,
        target_columns: study.plane.columns,
        reduction: ReductionRule::Detail,
        palette: "viridis".to_owned(),
        cross_section: study.plane.rows / 2,
        frames: 1,
    };
    match field {
        "observable" => {
            edit.observable = observable(value, None)?;
            edit.palette = default_palette(edit.observable).to_owned();
        }
        "wt" => {
            edit.observable = Observable::Instant {
                wt: finite_number(value, "wt")?,
            };
            edit.palette = "blue-red".to_owned();
        }
        "floor" => {
            edit.observable = Observable::Phase;
            edit.phase_floor = nonnegative_number(value, "floor")?;
            edit.palette = "cyclic-phase".to_owned();
        }
        "palette" => {
            edit.palette = unqualified_name(value, "palette")?;
            if !HEATMAP_PALETTES.contains(&edit.palette.as_str()) {
                return Err(Error::Eval(format!(
                    "unknown interference palette {}",
                    edit.palette
                )));
            }
        }
        "cross-section" => {
            edit.cross_section = bounded_index(value, "cross-section", study.plane.rows)?;
        }
        "detector" => {
            edit.reduction = reduction(value)?;
            if edit.reduction != ReductionRule::Detail {
                edit.target_rows = study.plane.rows.div_ceil(2);
                edit.target_columns = study.plane.columns.div_ceil(2);
            }
        }
        "frames" => {
            edit.frames = positive_usize(value, "frames")?;
            if edit.frames > crate::MAX_ANIMATION_FRAMES {
                return Err(Error::Eval(format!(
                    "interference animation frames exceed {}",
                    crate::MAX_ANIMATION_FRAMES
                )));
            }
            edit.observable = Observable::Instant { wt: 0.0 };
            edit.palette = "blue-red".to_owned();
        }
        _ => unreachable!("projection field matched above"),
    }
    validate_projection(&edit, study)?;
    Ok(EditClass::Projection(edit))
}

fn model_edit(study: &StudyDescriptor, path: &[String], value: &Expr) -> Result<EditClass> {
    let mut problem = study.problem.clone();
    let mut plane = study.plane.clone();
    match path {
        [field] if field == "frequency" => {
            problem.frequency_hz = positive_number(value, "frequency")?;
        }
        [root, field] if root == "medium" => edit_medium(&mut problem.medium, field, value)?,
        [root, id, field] if root == "source" => {
            let source = problem
                .emitters
                .iter_mut()
                .find(|source| source.id == *id)
                .ok_or_else(|| Error::Eval(format!("unknown interference source {id}")))?;
            edit_source(source, field, value)?;
        }
        [root, field] if root == "plane" => edit_plane(&mut plane, field, value)?,
        _ => {
            return Err(Error::Eval(format!(
                "unsupported interference model edit path {}",
                path.join(".")
            )));
        }
    }
    problem.to_problem()?;
    plane.to_plane()?;
    Ok(EditClass::Model(ModelEdit {
        problem,
        plane,
        sampling: study.evidence.sampling_policy.clone(),
    }))
}

fn edit_medium(medium: &mut MediumDescriptor, field: &str, value: &Expr) -> Result<()> {
    match field {
        "speed-m-s" => medium.speed_m_s = positive_number(value, field)?,
        "attenuation-np-m" => medium.attenuation_np_m = nonnegative_number(value, field)?,
        _ => return Err(Error::Eval(format!("unsupported medium field {field}"))),
    }
    MediumDescriptor::new(medium.speed_m_s, medium.attenuation_np_m)?;
    Ok(())
}

fn edit_source(source: &mut EmitterDescriptor, field: &str, value: &Expr) -> Result<()> {
    match field {
        "amplitude" => source.amplitude = nonnegative_number(value, field)?,
        "phase-rad" => source.phase_rad = finite_number(value, field)?,
        "anchor-m" => source.anchor_m = number_vector(value, field)?,
        "direction" => source.direction = Some(number_vector(value, field)?),
        _ => return Err(Error::Eval(format!("unsupported source field {field}"))),
    }
    source.to_emitter()?;
    Ok(())
}

fn edit_plane(plane: &mut PlaneDescriptor, field: &str, value: &Expr) -> Result<()> {
    match field {
        "origin-m" => plane.origin_m = number_vector(value, field)?,
        "u-axis" => plane.u_axis = number_vector(value, field)?,
        "v-axis" => plane.v_axis = number_vector(value, field)?,
        "extent-u-m" => plane.extent_u_m = positive_number(value, field)?,
        "extent-v-m" => plane.extent_v_m = positive_number(value, field)?,
        "rows" => plane.rows = positive_usize(value, field)?,
        "columns" | "cols" => plane.columns = positive_usize(value, field)?,
        _ => return Err(Error::Eval(format!("unsupported plane field {field}"))),
    }
    plane.to_plane()?;
    Ok(())
}

pub(crate) fn encode_edit(cx: &mut Cx, edit: &EditClass) -> Result<Expr> {
    Ok(match edit {
        EditClass::Projection(edit) => build::map(vec![
            (
                "class",
                Expr::Symbol(Symbol::qualified("interference-surface", "project")),
            ),
            (
                "observable",
                Expr::Symbol(Symbol::new(observable_token(edit.observable))),
            ),
            (
                "wt",
                observable_wt(edit.observable)
                    .map(build::float)
                    .unwrap_or(Expr::Nil),
            ),
            ("phase-floor", build::float(edit.phase_floor)),
            ("target-rows", build::uint(edit.target_rows as u64)),
            ("target-columns", build::uint(edit.target_columns as u64)),
            (
                "reduction",
                Expr::Symbol(Symbol::new(reduction_token(edit.reduction))),
            ),
            ("palette", Expr::Symbol(Symbol::new(edit.palette.as_str()))),
            ("cross-section", build::uint(edit.cross_section as u64)),
            ("frames", build::uint(edit.frames as u64)),
        ]),
        EditClass::Model(edit) => build::map(vec![
            (
                "class",
                Expr::Symbol(Symbol::qualified("interference-surface", "solve")),
            ),
            ("problem", sim_citizen::constructor_expr(cx, &edit.problem)?),
            ("plane", sim_citizen::constructor_expr(cx, &edit.plane)?),
            ("sampling", Expr::Symbol(edit.sampling.clone())),
        ]),
    })
}

pub(crate) fn decode_edit(cx: &mut Cx, study: &StudyDescriptor, expr: &Expr) -> Result<EditClass> {
    let class = access::field_sym(expr, "class")
        .ok_or_else(|| Error::Eval("interference proposed edit is missing class".to_owned()))?;
    match (class.namespace.as_deref(), class.name.as_ref()) {
        (Some("interference-surface"), "project") => {
            reject_unknown(
                expr,
                &[
                    "class",
                    "observable",
                    "wt",
                    "phase-floor",
                    "target-rows",
                    "target-columns",
                    "reduction",
                    "palette",
                    "cross-section",
                    "frames",
                ],
            )?;
            let wt = optional_number(expr, "wt")?;
            let edit = ProjectionEdit {
                observable: observable(required(expr, "observable")?, wt)?,
                phase_floor: required_number(expr, "phase-floor")?,
                target_rows: required_usize(expr, "target-rows")?,
                target_columns: required_usize(expr, "target-columns")?,
                reduction: reduction(required(expr, "reduction")?)?,
                palette: unqualified_name(required(expr, "palette")?, "palette")?,
                cross_section: required_usize(expr, "cross-section")?,
                frames: required_usize(expr, "frames")?,
            };
            validate_projection(&edit, study)?;
            Ok(EditClass::Projection(edit))
        }
        (Some("interference-surface"), "solve") => {
            reject_unknown(expr, &["class", "problem", "plane", "sampling"])?;
            let edit = ModelEdit {
                problem: decode_citizen(cx, required(expr, "problem")?, "proposed problem")?,
                plane: decode_citizen(cx, required(expr, "plane")?, "proposed plane")?,
                sampling: access::field_sym(expr, "sampling")
                    .ok_or_else(|| Error::Eval("proposed solve is missing sampling".to_owned()))?,
            };
            edit.problem.to_problem()?;
            edit.plane.to_plane()?;
            Ok(EditClass::Model(edit))
        }
        _ => Err(Error::Eval(format!(
            "unknown interference proposed edit class {class}"
        ))),
    }
}

pub(crate) fn project_form(base: &Expr, edit: &ProjectionEdit) -> Expr {
    let mut request = vec![
        (
            "observable",
            Expr::Symbol(Symbol::new(observable_token(edit.observable))),
        ),
        ("phase-floor", build::float(edit.phase_floor)),
        ("target-rows", build::uint(edit.target_rows as u64)),
        ("target-cols", build::uint(edit.target_columns as u64)),
        (
            "reduction",
            Expr::Symbol(Symbol::new(reduction_token(edit.reduction))),
        ),
    ];
    if let Some(wt) = observable_wt(edit.observable) {
        request.push(("wt", build::float(wt)));
    }
    Expr::List(vec![
        Expr::Symbol(project_function_symbol()),
        base.clone(),
        build::map(request),
    ])
}

pub(crate) fn solve_form(cx: &mut Cx, edit: &ModelEdit) -> Result<Expr> {
    Ok(Expr::List(vec![
        Expr::Symbol(solve_function_symbol()),
        sim_citizen::constructor_expr(cx, &edit.problem)?,
        sim_citizen::constructor_expr(cx, &edit.plane)?,
        build::map(vec![
            ("sampling", Expr::Symbol(edit.sampling.clone())),
            ("work-budget", Expr::Symbol(Symbol::new("default"))),
        ]),
    ]))
}

fn validate_projection(edit: &ProjectionEdit, study: &StudyDescriptor) -> Result<()> {
    if edit.target_rows > study.plane.rows || edit.target_columns > study.plane.columns {
        return Err(Error::Eval(
            "interference projection target exceeds the Study plane".to_owned(),
        ));
    }
    ProjectionRequestDescriptor::new(
        edit.observable,
        edit.phase_floor,
        edit.target_rows,
        edit.target_columns,
        edit.reduction,
    )?;
    if !HEATMAP_PALETTES.contains(&edit.palette.as_str()) {
        return Err(Error::Eval(format!(
            "unknown interference palette {}",
            edit.palette
        )));
    }
    if edit.cross_section >= edit.target_rows {
        return Err(Error::Eval(format!(
            "cross-section {} is outside {} projected rows",
            edit.cross_section, edit.target_rows
        )));
    }
    if edit.frames == 0 || edit.frames > crate::MAX_ANIMATION_FRAMES {
        return Err(Error::Eval(format!(
            "interference animation frames must be in 1..={}",
            crate::MAX_ANIMATION_FRAMES
        )));
    }
    Ok(())
}

fn intent_path_and_value(intent: &Expr) -> Result<(Vec<String>, &Expr)> {
    let kind = sim_lib_intent::intent_kind_of(intent)
        .ok_or_else(|| Error::Eval("interference editor input is not an Intent".to_owned()))?;
    match kind.name.as_ref() {
        "edit-field" => {
            let path = access::field(intent, "path")
                .ok_or_else(|| Error::Eval("edit-field is missing path".to_owned()))?;
            Ok((path_segments(path)?, required(intent, "value")?))
        }
        "set-param" => {
            let param = unqualified_name(required(intent, "param")?, "param")?;
            Ok((vec![param], required(intent, "value")?))
        }
        other => Err(Error::Eval(format!(
            "interference surface does not handle intent/{other}"
        ))),
    }
}

fn path_segments(expr: &Expr) -> Result<Vec<String>> {
    let Expr::List(items) = expr else {
        return Err(Error::Eval(
            "interference edit path must be a list".to_owned(),
        ));
    };
    items
        .iter()
        .map(|item| match item {
            Expr::Symbol(symbol) if symbol.namespace.is_none() => Ok(symbol.name.to_string()),
            Expr::String(text) if !text.is_empty() => Ok(text.clone()),
            _ => Err(Error::Eval(
                "interference edit path segments must be unqualified symbols or text".to_owned(),
            )),
        })
        .collect()
}

fn decode_citizen<T>(cx: &mut Cx, expr: &Expr, context: &'static str) -> Result<T>
where
    T: CitizenRuntime,
{
    let Expr::Extension { tag, payload } = expr else {
        return Err(Error::Eval(format!(
            "{context} must be a Citizen read-construct"
        )));
    };
    if *tag != Symbol::qualified("citizen", "read-construct") {
        return Err(Error::Eval(format!(
            "{context} has unexpected extension {tag}"
        )));
    }
    let Expr::Vector(items) = payload.as_ref() else {
        return Err(Error::Eval(format!(
            "{context} read-construct must be a vector"
        )));
    };
    let Some((Expr::Symbol(class), args)) = items.split_first() else {
        return Err(Error::Eval(format!(
            "{context} read-construct is missing its class"
        )));
    };
    if class != &T::citizen_symbol() {
        return Err(Error::Eval(format!(
            "{context} expected {}, found {class}",
            T::citizen_symbol()
        )));
    }
    let values = args
        .iter()
        .map(|arg| sim_citizen::value_from_expr(cx, arg))
        .collect::<Result<Vec<_>>>()?;
    T::construct_from_values(cx, values)
}

fn observable(expr: &Expr, wt: Option<f64>) -> Result<Observable> {
    Ok(match unqualified_name(expr, "observable")?.as_str() {
        "real" => Observable::Real,
        "imaginary" => Observable::Imaginary,
        "amplitude" => Observable::Amplitude,
        "phase" => Observable::Phase,
        "magnitude-squared" => Observable::MagnitudeSquared,
        "instant" => Observable::Instant {
            wt: wt.ok_or_else(|| Error::Eval("instant observable requires wt".to_owned()))?,
        },
        other => return Err(Error::Eval(format!("unknown observable {other}"))),
    })
}

fn reduction(expr: &Expr) -> Result<ReductionRule> {
    Ok(match unqualified_name(expr, "detector")?.as_str() {
        "detail" => ReductionRule::Detail,
        "detector-complex-mean" => ReductionRule::DetectorComplexMean,
        "detector-scalar-area-mean" => ReductionRule::DetectorScalarAreaMean,
        "detector-magnitude-squared-area-mean" => ReductionRule::DetectorMagnitudeSquaredAreaMean,
        other => return Err(Error::Eval(format!("unknown detector {other}"))),
    })
}

fn observable_token(observable: Observable) -> &'static str {
    match observable {
        Observable::Real => "real",
        Observable::Imaginary => "imaginary",
        Observable::Amplitude => "amplitude",
        Observable::Phase => "phase",
        Observable::MagnitudeSquared => "magnitude-squared",
        Observable::Instant { .. } => "instant",
    }
}

fn observable_wt(observable: Observable) -> Option<f64> {
    match observable {
        Observable::Instant { wt } => Some(wt),
        _ => None,
    }
}

fn reduction_token(rule: ReductionRule) -> &'static str {
    match rule {
        ReductionRule::Detail => "detail",
        ReductionRule::DetectorComplexMean => "detector-complex-mean",
        ReductionRule::DetectorScalarAreaMean => "detector-scalar-area-mean",
        ReductionRule::DetectorMagnitudeSquaredAreaMean => "detector-magnitude-squared-area-mean",
    }
}

fn default_palette(observable: Observable) -> &'static str {
    match observable {
        Observable::Phase => "cyclic-phase",
        Observable::Real | Observable::Imaginary | Observable::Instant { .. } => "blue-red",
        Observable::Amplitude | Observable::MagnitudeSquared => "viridis",
    }
}

fn reject_unknown(expr: &Expr, allowed: &[&str]) -> Result<()> {
    let Expr::Map(entries) = expr else {
        return Err(Error::Eval(
            "interference proposed edit must be a map".to_owned(),
        ));
    };
    for (key, _) in entries {
        let Expr::Symbol(key) = key else {
            return Err(Error::Eval(
                "interference proposed edit keys must be symbols".to_owned(),
            ));
        };
        if key.namespace.is_some() || !allowed.contains(&key.name.as_ref()) {
            return Err(Error::Eval(format!(
                "unknown interference proposed edit field {key}"
            )));
        }
    }
    Ok(())
}

fn required<'a>(expr: &'a Expr, name: &str) -> Result<&'a Expr> {
    access::field(expr, name)
        .ok_or_else(|| Error::Eval(format!("interference edit is missing {name}")))
}

fn unqualified_name(expr: &Expr, field: &str) -> Result<String> {
    match expr {
        Expr::Symbol(symbol) if symbol.namespace.is_none() => Ok(symbol.name.to_string()),
        Expr::String(text) if !text.is_empty() => Ok(text.clone()),
        _ => Err(Error::Eval(format!(
            "interference {field} must be an unqualified symbol or text"
        ))),
    }
}

fn finite_number(expr: &Expr, field: &str) -> Result<f64> {
    let value = sim_value::access::as_f64(expr)
        .ok_or_else(|| Error::Eval(format!("interference {field} must be a number")))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(Error::Eval(format!("interference {field} must be finite")))
    }
}

fn positive_number(expr: &Expr, field: &str) -> Result<f64> {
    let value = finite_number(expr, field)?;
    if value > 0.0 {
        Ok(value)
    } else {
        Err(Error::Eval(format!(
            "interference {field} must be positive"
        )))
    }
}

fn nonnegative_number(expr: &Expr, field: &str) -> Result<f64> {
    let value = finite_number(expr, field)?;
    if value >= 0.0 {
        Ok(value)
    } else {
        Err(Error::Eval(format!(
            "interference {field} must be non-negative"
        )))
    }
}

fn required_number(expr: &Expr, field: &str) -> Result<f64> {
    finite_number(required(expr, field)?, field)
}

fn optional_number(expr: &Expr, field: &str) -> Result<Option<f64>> {
    match access::field(expr, field) {
        None | Some(Expr::Nil) => Ok(None),
        Some(value) => finite_number(value, field).map(Some),
    }
}

fn positive_usize(expr: &Expr, field: &str) -> Result<usize> {
    let value = expr_usize(expr, field)?;
    if value > 0 {
        Ok(value)
    } else {
        Err(Error::Eval(format!(
            "interference {field} must be positive"
        )))
    }
}

fn required_usize(expr: &Expr, field: &str) -> Result<usize> {
    expr_usize(required(expr, field)?, field)
}

fn bounded_index(expr: &Expr, field: &str, upper: usize) -> Result<usize> {
    let value = expr_usize(expr, field)?;
    if value < upper {
        Ok(value)
    } else {
        Err(Error::Eval(format!(
            "interference {field} must be below {upper}"
        )))
    }
}

fn expr_usize(expr: &Expr, field: &str) -> Result<usize> {
    let Expr::Number(number) = expr else {
        return Err(Error::Eval(format!(
            "interference {field} must be an integer"
        )));
    };
    number.canonical.parse().map_err(|_| {
        Error::Eval(format!(
            "interference {field} must be a non-negative integer"
        ))
    })
}

fn number_vector(expr: &Expr, field: &str) -> Result<Vec<f64>> {
    let (Expr::List(values) | Expr::Vector(values)) = expr else {
        return Err(Error::Eval(format!(
            "interference {field} must be a number list"
        )));
    };
    values
        .iter()
        .map(|value| finite_number(value, field))
        .collect()
}
