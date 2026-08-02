//! Certified domain projection into general heatmap and plot Scene nodes.

use std::f64::consts::TAU;

use sim_kernel::{Cx, Error, Expr, Result};
use sim_lib_interference_core::{SamplingVerdict, WorkEstimate};
use sim_lib_interference_runtime::StudyDescriptor;
use sim_lib_interference_solve::{
    LossClass, Observable, ProjectionCertificate, ReductionRule, ScalarProjection, ScalarSample,
    reduce_for_view,
};
use sim_lib_scene::{badge, box_, data_map, node, sym, text_node, validate_scene};
use sim_lib_view::SurfaceCaps;
use sim_lib_view_math::{
    BLUE_RED_PALETTE, CYCLIC_PHASE_PALETTE, HeatmapData, VIRIDIS_PALETTE, heatmap_budget,
    heatmap_view, number, plot_view,
};
use sim_value::build::{list, text, uint};

use crate::surface::MAX_ANIMATION_FRAMES;

const HEATMAP_CELL_BYTES: u64 = 9;
const METADATA_BYTE_RESERVE: u64 = 1024;

#[derive(Clone, Copy)]
pub(crate) struct ProjectionOptions {
    pub(crate) observable: Observable,
    pub(crate) phase_floor: f64,
    pub(crate) palette: &'static str,
    pub(crate) cross_section: Option<usize>,
}

impl Default for ProjectionOptions {
    fn default() -> Self {
        Self {
            observable: Observable::Amplitude,
            phase_floor: 0.0,
            palette: VIRIDIS_PALETTE,
            cross_section: None,
        }
    }
}

pub(crate) fn study_scene(
    cx: &mut Cx,
    study: &StudyDescriptor,
    caps: &SurfaceCaps,
) -> Result<Expr> {
    study_scene_with(cx, study, caps, ProjectionOptions::default(), 1, None)
}

pub(crate) fn animation_scenes(
    cx: &mut Cx,
    study: &StudyDescriptor,
    caps: &SurfaceCaps,
    frames: usize,
) -> Result<Vec<Expr>> {
    if frames == 0 || frames > MAX_ANIMATION_FRAMES {
        return Err(Error::Eval(format!(
            "interference animation frames must be in 1..={MAX_ANIMATION_FRAMES}"
        )));
    }
    let budget = heatmap_budget(caps)?;
    let total_cells = budget.max_cells().min(bytes_to_cells(budget.max_bytes()));
    if total_cells < frames {
        return Err(Error::Eval(format!(
            "surface heatmap budget admits {total_cells} animation cells, fewer than {frames} frames"
        )));
    }
    (0..frames)
        .map(|index| {
            let wt = TAU * index as f64 / frames as f64;
            study_scene_with(
                cx,
                study,
                caps,
                ProjectionOptions {
                    observable: Observable::Instant { wt },
                    palette: BLUE_RED_PALETTE,
                    ..ProjectionOptions::default()
                },
                frames,
                Some((index, frames)),
            )
        })
        .collect()
}

pub(crate) fn study_scene_with(
    cx: &mut Cx,
    study: &StudyDescriptor,
    caps: &SurfaceCaps,
    options: ProjectionOptions,
    budget_divisor: usize,
    frame: Option<(usize, usize)>,
) -> Result<Expr> {
    let projection = project_for_surface(cx, study, caps, options, budget_divisor)?;
    let certificate = projection.certificate();
    let (values, valid, range) = scalar_cells(&projection);
    let detector = detector_label(certificate);
    let advisory = projection_advisory(certificate);
    let label = observable_label(options.observable);
    let mut heatmap = heatmap_view(
        HeatmapData {
            rows: projection.rows(),
            cols: projection.columns(),
            values: &values,
            valid: &valid,
            range,
            palette: options.palette,
            label,
            detector: &detector,
            advisory: Some(&advisory),
        },
        caps,
    )?;
    attach_certificate(&mut heatmap, certificate, &advisory);

    let row = options
        .cross_section
        .unwrap_or(projection.rows() / 2)
        .min(projection.rows() - 1);
    let cross_section = projection
        .samples()
        .chunks(projection.columns())
        .nth(row)
        .expect("validated projection has its requested row")
        .iter()
        .enumerate()
        .filter_map(|(column, sample)| match sample {
            ScalarSample::Value(value) => Some((column as f64, *value)),
            ScalarSample::Masked => None,
        })
        .collect::<Vec<_>>();

    let sampling = certificate.source_sampling_certificate();
    let mut root_entries = vec![
        ("role", sym("interference-study")),
        ("dir", sym("column")),
        (
            "children",
            list(vec![
                controls(options, study, row),
                badge(
                    sampling_status(sampling.verdict),
                    &format!("sampling {}", sampling_status(sampling.verdict)),
                ),
                source_summary(study),
                evidence_summary(study),
                badge("contrast", &contrast_label(range)),
                heatmap,
                plot_view(&format!("cross-section-row-{row}"), &cross_section),
            ]),
        ),
    ];
    if let Some((index, frames)) = frame {
        root_entries.push(("animation-frame", uint(index as u64)));
        root_entries.push(("animation-frames", uint(frames as u64)));
    }
    let scene = node("stack", root_entries);
    validate_scene(&scene)
        .map_err(|error| Error::HostError(format!("invalid interference Scene: {error}")))?;
    Ok(scene)
}

fn project_for_surface(
    cx: &mut Cx,
    study: &StudyDescriptor,
    caps: &SurfaceCaps,
    options: ProjectionOptions,
    budget_divisor: usize,
) -> Result<ScalarProjection> {
    let budget = heatmap_budget(caps)?;
    let frame_cells = budget.max_cells() / budget_divisor;
    let frame_bytes = budget.max_bytes() / budget_divisor as u64;
    let limit = frame_cells.min(bytes_to_cells(frame_bytes));
    if limit == 0 {
        return Err(Error::Eval(
            "surface heatmap budget cannot admit one interference cell".to_owned(),
        ));
    }
    let (rows, columns) = fit_grid(study.field.rows, study.field.cols, limit);
    let rule = if rows == study.field.rows && columns == study.field.cols {
        ReductionRule::Detail
    } else {
        detector_for(options.observable)
    };
    let field = study.field.materialize_host(cx)?;
    reduce_for_view(
        &field,
        study.evidence.sampling.to_certificate()?,
        options.observable,
        options.phase_floor,
        rows,
        columns,
        rule,
    )
    .map_err(|error| Error::Eval(format!("interference view projection failed: {error}")))
}

fn bytes_to_cells(bytes: u64) -> usize {
    usize::try_from(bytes.saturating_sub(METADATA_BYTE_RESERVE) / HEATMAP_CELL_BYTES)
        .unwrap_or(usize::MAX)
}

fn fit_grid(rows: usize, columns: usize, limit: usize) -> (usize, usize) {
    if rows.saturating_mul(columns) <= limit {
        return (rows, columns);
    }
    let ratio = rows as f64 / columns as f64;
    let mut target_rows = ((limit as f64 * ratio).sqrt().floor() as usize).clamp(1, rows);
    let mut target_columns = (limit / target_rows).clamp(1, columns);
    while target_rows.saturating_mul(target_columns) > limit {
        if target_columns > 1 {
            target_columns -= 1;
        } else {
            target_rows -= 1;
        }
    }
    (target_rows, target_columns)
}

fn detector_for(observable: Observable) -> ReductionRule {
    match observable {
        Observable::Amplitude => ReductionRule::DetectorScalarAreaMean,
        Observable::MagnitudeSquared => ReductionRule::DetectorMagnitudeSquaredAreaMean,
        Observable::Real
        | Observable::Imaginary
        | Observable::Phase
        | Observable::Instant { .. } => ReductionRule::DetectorComplexMean,
    }
}

fn scalar_cells(projection: &ScalarProjection) -> (Vec<f64>, Vec<bool>, (f64, f64)) {
    let mut values = Vec::with_capacity(projection.samples().len());
    let mut valid = Vec::with_capacity(projection.samples().len());
    let mut range: Option<(f64, f64)> = None;
    for sample in projection.samples() {
        match sample {
            ScalarSample::Value(value) => {
                values.push(*value);
                valid.push(true);
                range = Some(range.map_or((*value, *value), |(min, max)| {
                    (min.min(*value), max.max(*value))
                }));
            }
            ScalarSample::Masked => {
                values.push(0.0);
                valid.push(false);
            }
        }
    }
    (values, valid, range.unwrap_or((0.0, 0.0)))
}

fn controls(options: ProjectionOptions, study: &StudyDescriptor, row: usize) -> Expr {
    box_(
        "interference-controls",
        vec![
            selector(
                "observable",
                observable_token(options.observable),
                &[
                    "real",
                    "imaginary",
                    "amplitude",
                    "phase",
                    "magnitude-squared",
                    "instant",
                ],
            ),
            selector(
                "detector",
                reduction_token(detector_for(options.observable)),
                &[
                    "detail",
                    "detector-complex-mean",
                    "detector-scalar-area-mean",
                    "detector-magnitude-squared-area-mean",
                ],
            ),
            selector(
                "palette",
                options.palette,
                &[VIRIDIS_PALETTE, BLUE_RED_PALETTE, CYCLIC_PHASE_PALETTE],
            ),
            slider("wt", -TAU, TAU, observable_wt(options.observable)),
            slider("floor", 0.0, 1.0, options.phase_floor),
            slider(
                "cross-section",
                0.0,
                (study.plane.rows - 1) as f64,
                row as f64,
            ),
        ],
    )
}

fn selector(param: &str, value: &str, options: &[&str]) -> Expr {
    node(
        "field",
        vec![
            ("control", sym("select")),
            ("param", sym(param)),
            ("value", sym(value)),
            (
                "options",
                list(options.iter().map(|option| sym(option)).collect()),
            ),
        ],
    )
}

fn slider(param: &str, min: f64, max: f64, value: f64) -> Expr {
    node(
        "slider",
        vec![
            ("param", sym(param)),
            ("min", number(min)),
            ("max", number(max)),
            ("value", number(value)),
        ],
    )
}

fn source_summary(study: &StudyDescriptor) -> Expr {
    box_(
        "interference-sources",
        std::iter::once(text_node(format!(
            "frequency {} Hz in medium {} m/s, attenuation {} Np/m",
            study.problem.frequency_hz,
            study.problem.medium.speed_m_s,
            study.problem.medium.attenuation_np_m
        )))
        .chain(study.problem.emitters.iter().map(|source| {
            text_node(format!(
                "source {} {} amplitude {} phase {} rad",
                source.id, source.kind, source.amplitude, source.phase_rad
            ))
        }))
        .collect(),
    )
}

fn evidence_summary(study: &StudyDescriptor) -> Expr {
    let evidence = &study.evidence;
    let WorkEstimate {
        cells,
        emitter_evaluations,
        host_bytes,
        result_bytes,
        ..
    } = evidence
        .work
        .to_estimate()
        .expect("validated Study work evidence");
    box_(
        "interference-evidence",
        vec![
            text_node(format!(
                "provider {} adapter {} dtype {}",
                evidence.provider, evidence.adapter, evidence.dtype
            )),
            text_node(format!(
                "cells {cells}, source evaluations {emitter_evaluations}, host bytes {host_bytes}, result bytes {result_bytes}"
            )),
            text_node(format!(
                "component tolerance {}, squared-magnitude tolerance {}",
                evidence.component_absolute_tolerance,
                evidence.squared_magnitude_absolute_tolerance
            )),
        ],
    )
}

fn attach_certificate(scene: &mut Expr, certificate: ProjectionCertificate, advisory: &str) {
    let footprint = certificate.footprint();
    let source = certificate.source_dimensions();
    let target = certificate.target_dimensions();
    let Expr::Map(entries) = scene else {
        unreachable!("heatmap_view always returns a map")
    };
    entries.push((
        sym("projection-certificate"),
        data_map(vec![
            ("source-rows", uint(source.rows() as u64)),
            ("source-columns", uint(source.columns() as u64)),
            ("target-rows", uint(target.rows() as u64)),
            ("target-columns", uint(target.columns() as u64)),
            ("footprint-min-rows", uint(footprint.min_rows() as u64)),
            ("footprint-max-rows", uint(footprint.max_rows() as u64)),
            (
                "footprint-min-columns",
                uint(footprint.min_columns() as u64),
            ),
            (
                "footprint-max-columns",
                uint(footprint.max_columns() as u64),
            ),
            ("detector", sym(reduction_token(certificate.rule()))),
            ("loss", sym(loss_token(certificate.loss_class()))),
            ("masked", uint(certificate.mask_count() as u64)),
            ("advisory", text(advisory)),
        ]),
    ));
}

fn detector_label(certificate: ProjectionCertificate) -> String {
    let footprint = certificate.footprint();
    format!(
        "{}; footprint rows {}..{}, columns {}..{}",
        reduction_token(certificate.rule()),
        footprint.min_rows(),
        footprint.max_rows(),
        footprint.min_columns(),
        footprint.max_columns()
    )
}

fn projection_advisory(certificate: ProjectionCertificate) -> String {
    format!(
        "sampling {}; {}; {} masked cell(s)",
        sampling_status(certificate.source_sampling_certificate().verdict),
        loss_token(certificate.loss_class()),
        certificate.mask_count()
    )
}

fn contrast_label(range: (f64, f64)) -> String {
    format!(
        "min {}, max {}, span {}",
        range.0,
        range.1,
        range.1 - range.0
    )
}

fn observable_label(observable: Observable) -> &'static str {
    match observable {
        Observable::Real => "real component",
        Observable::Imaginary => "imaginary component",
        Observable::Amplitude => "field amplitude",
        Observable::Phase => "wrapped phase",
        Observable::MagnitudeSquared => "normalized squared magnitude",
        Observable::Instant { .. } => "instantaneous field",
    }
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

fn observable_wt(observable: Observable) -> f64 {
    match observable {
        Observable::Instant { wt } => wt,
        _ => 0.0,
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

fn loss_token(loss: LossClass) -> &'static str {
    match loss {
        LossClass::Lossless => "lossless",
        LossClass::DetectorIntegration => "detector-integration",
    }
}

fn sampling_status(verdict: SamplingVerdict) -> &'static str {
    match verdict {
        SamplingVerdict::Resolved => "resolved",
        SamplingVerdict::Marginal => "marginal",
        SamplingVerdict::Aliased => "aliased",
    }
}
