//! Bounded scalar-grid projection as a domain-neutral `scene/heatmap`.
//!
//! The caller owns detector semantics and prepares the exact values and mask
//! that may be displayed. This module derives a display budget from open
//! [`SurfaceCaps`] metadata, refuses data that exceeds it, and never
//! downsamples or otherwise changes the caller's grid.

use sim_kernel::{Error, Expr, Result};
use sim_lib_scene::{HEATMAP_PALETTES, data_map, heatmap_payload_bytes, node, sym, validate_scene};
use sim_lib_view::SurfaceCaps;
use sim_value::{access, build};

use crate::num::number;

/// Named sequential palette for monotonically ordered scalar data.
pub const VIRIDIS_PALETTE: &str = "viridis";

/// Named diverging palette for signed or reference-centred scalar data.
pub const BLUE_RED_PALETTE: &str = "blue-red";

/// Named cyclic palette for phase-like scalar data whose endpoints meet.
pub const CYCLIC_PHASE_PALETTE: &str = "cyclic-phase";

/// Absolute cell ceiling for one heatmap Scene, independent of surface claims.
pub const MAX_HEATMAP_CELLS: usize = 1024 * 1024;

/// Absolute scalar-payload ceiling for one heatmap Scene.
pub const MAX_HEATMAP_BYTES: u64 = 32 * 1024 * 1024;

const DEFAULT_CELLS: usize = 64 * 1024;
const DEFAULT_BYTES: u64 = 1024 * 1024;

/// Caller-prepared scalar grid projected by [`heatmap_view`].
///
/// `values` and `valid` are row-major and must each contain exactly
/// `rows * cols` entries. The helper validates them but never resamples them:
/// only the caller's domain knows whether a smaller grid requires point
/// sampling, area integration, complex averaging, or another detector rule.
#[derive(Clone, Copy, Debug)]
pub struct HeatmapData<'a> {
    /// Number of non-zero grid rows.
    pub rows: usize,
    /// Number of non-zero grid columns.
    pub cols: usize,
    /// Finite row-major scalar values.
    pub values: &'a [f64],
    /// Row-major validity mask, parallel to `values`.
    pub valid: &'a [bool],
    /// Finite inclusive display range `(min, max)`.
    pub range: (f64, f64),
    /// One of [`sim_lib_scene::HEATMAP_PALETTES`].
    pub palette: &'a str,
    /// Non-empty accessible label for the represented quantity.
    pub label: &'a str,
    /// Non-empty caller-supplied detector or sampling description.
    pub detector: &'a str,
    /// Optional non-empty warning or qualification rendered with the grid.
    pub advisory: Option<&'a str>,
}

/// Checked cell and scalar-payload ceilings derived from display metadata.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeatmapBudget {
    max_cells: usize,
    max_bytes: u64,
}

impl HeatmapBudget {
    /// Maximum caller-prepared grid cells.
    pub const fn max_cells(self) -> usize {
        self.max_cells
    }

    /// Maximum scalar-and-metadata payload bytes.
    pub const fn max_bytes(self) -> u64 {
        self.max_bytes
    }
}

/// Derive the checked heatmap budget for a surface.
///
/// Density provides a conservative baseline; physical pixel dimensions cap
/// the useful cell count when present. Open display metadata may advertise
/// lower or higher `heatmap-max-cells` and `heatmap-max-bytes` values, bounded
/// by [`MAX_HEATMAP_CELLS`] and [`MAX_HEATMAP_BYTES`]. Malformed or overflowing
/// advertised values fail closed.
pub fn heatmap_budget(caps: &SurfaceCaps) -> Result<HeatmapBudget> {
    let (density_cells, density_bytes) = density_budget(caps);
    let advertised_cells =
        display_limit(&caps.display, "heatmap-max-cells")?.unwrap_or(density_cells as u64);
    let advertised_bytes =
        display_limit(&caps.display, "heatmap-max-bytes")?.unwrap_or(density_bytes);
    if advertised_cells == 0 || advertised_cells > MAX_HEATMAP_CELLS as u64 {
        return Err(Error::Eval(format!(
            "surface heatmap cell budget {advertised_cells} is outside 1..={MAX_HEATMAP_CELLS}"
        )));
    }
    if advertised_bytes == 0 || advertised_bytes > MAX_HEATMAP_BYTES {
        return Err(Error::Eval(format!(
            "surface heatmap byte budget {advertised_bytes} is outside 1..={MAX_HEATMAP_BYTES}"
        )));
    }
    let pixel_cells = display_pixel_cells(&caps.display)?;
    let max_cells = usize::try_from(
        pixel_cells.map_or(advertised_cells, |pixels| advertised_cells.min(pixels)),
    )
    .map_err(|_| Error::Eval("surface heatmap cell budget does not fit usize".to_owned()))?;
    if max_cells == 0 {
        return Err(Error::Eval(
            "surface heatmap pixel dimensions admit no cells".to_owned(),
        ));
    }
    Ok(HeatmapBudget {
        max_cells,
        max_bytes: advertised_bytes,
    })
}

/// Return only the checked cell ceiling from [`heatmap_budget`].
pub fn heatmap_cell_budget(caps: &SurfaceCaps) -> Result<usize> {
    heatmap_budget(caps).map(HeatmapBudget::max_cells)
}

/// Return only the checked scalar-payload byte ceiling from [`heatmap_budget`].
pub fn heatmap_byte_budget(caps: &SurfaceCaps) -> Result<u64> {
    heatmap_budget(caps).map(HeatmapBudget::max_bytes)
}

/// Validate and project caller-prepared data to one `scene/heatmap`.
///
/// Oversized input is refused with a diagnostic telling the caller to apply
/// its domain-specific detector rule first. This function never downsamples,
/// drops, reorders, clamps, or otherwise changes a grid cell.
pub fn heatmap_view(data: HeatmapData<'_>, caps: &SurfaceCaps) -> Result<Expr> {
    let cells = data
        .rows
        .checked_mul(data.cols)
        .ok_or_else(|| Error::Eval("heatmap rows * cols overflowed".to_owned()))?;
    if data.rows == 0 || data.cols == 0 {
        return Err(Error::Eval(
            "heatmap rows and cols must be non-zero".to_owned(),
        ));
    }
    if data.values.len() != cells {
        return Err(Error::Eval(format!(
            "heatmap rows * cols is {cells}, but values has {} entries",
            data.values.len()
        )));
    }
    if data.valid.len() != cells {
        return Err(Error::Eval(format!(
            "heatmap rows * cols is {cells}, but valid has {} entries",
            data.valid.len()
        )));
    }
    if let Some(index) = data.values.iter().position(|value| !value.is_finite()) {
        return Err(Error::Eval(format!(
            "heatmap values[{index}] must be finite"
        )));
    }
    if !data.range.0.is_finite() || !data.range.1.is_finite() || data.range.0 > data.range.1 {
        return Err(Error::Eval(
            "heatmap range must be finite with min <= max".to_owned(),
        ));
    }
    if !HEATMAP_PALETTES.contains(&data.palette) {
        return Err(Error::Eval(format!(
            "heatmap palette '{}' is not recognized",
            data.palette
        )));
    }
    require_metadata("label", data.label)?;
    require_metadata("detector", data.detector)?;
    if let Some(advisory) = data.advisory {
        require_metadata("advisory", advisory)?;
    }
    let cells_u64 = u64::try_from(cells)
        .map_err(|_| Error::Eval("heatmap cell count does not fit u64".to_owned()))?;
    let bytes = heatmap_payload_bytes(cells_u64, data.label, data.detector, data.advisory)
        .ok_or_else(|| Error::Eval("heatmap byte footprint overflowed".to_owned()))?;
    let budget = heatmap_budget(caps)?;
    if cells > budget.max_cells {
        return Err(Error::Eval(format!(
            "heatmap has {cells} cells, surface budget is {}; apply the domain detector rule first",
            budget.max_cells
        )));
    }
    if bytes > budget.max_bytes {
        return Err(Error::Eval(format!(
            "heatmap has a {bytes}-byte payload, surface budget is {} bytes; apply the domain detector rule first",
            budget.max_bytes
        )));
    }

    let mut entries = vec![
        ("rows", build::uint(data.rows as u64)),
        ("cols", build::uint(data.cols as u64)),
        (
            "values",
            Expr::List(data.values.iter().copied().map(number).collect()),
        ),
        (
            "valid",
            Expr::List(data.valid.iter().copied().map(Expr::Bool).collect()),
        ),
        ("min", number(data.range.0)),
        ("max", number(data.range.1)),
        ("palette", sym(data.palette)),
        ("label", Expr::String(data.label.to_owned())),
        ("detector", Expr::String(data.detector.to_owned())),
        (
            "footprint",
            data_map(vec![
                ("cells", build::uint(cells_u64)),
                ("bytes", build::uint(bytes)),
            ]),
        ),
    ];
    if let Some(advisory) = data.advisory {
        entries.push(("advisory", Expr::String(advisory.to_owned())));
    }
    let scene = node("heatmap", entries);
    validate_scene(&scene)
        .map_err(|error| Error::Eval(format!("invalid scene/heatmap: {error}")))?;
    Ok(scene)
}

fn require_metadata(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        Err(Error::Eval(format!("heatmap {name} must not be empty")))
    } else {
        Ok(())
    }
}

fn density_budget(caps: &SurfaceCaps) -> (usize, u64) {
    let density = caps.display_density();
    match density.as_ref().map(|symbol| symbol.name.as_ref()) {
        Some("glance") => (4 * 1024, 64 * 1024),
        Some("compact") => (64 * 1024, 1024 * 1024),
        Some("regular") => (256 * 1024, 4 * 1024 * 1024),
        Some("dense") => (MAX_HEATMAP_CELLS, 16 * 1024 * 1024),
        _ => (DEFAULT_CELLS, DEFAULT_BYTES),
    }
}

fn display_limit(display: &Expr, name: &str) -> Result<Option<u64>> {
    access::field(display, name)
        .map(|value| {
            integer(value).ok_or_else(|| {
                Error::Eval(format!(
                    "surface display {name} must be a non-negative integer"
                ))
            })
        })
        .transpose()
}

fn display_pixel_cells(display: &Expr) -> Result<Option<u64>> {
    for name in ["px", "mono-px", "per-eye-px"] {
        let Some(value) = access::field(display, name) else {
            continue;
        };
        let Expr::List(dimensions) = value else {
            return Err(Error::Eval(format!(
                "surface display {name} must be [width, height]"
            )));
        };
        let [width, height] = dimensions.as_slice() else {
            return Err(Error::Eval(format!(
                "surface display {name} must contain exactly two dimensions"
            )));
        };
        let width = integer(width)
            .filter(|value| *value > 0)
            .ok_or_else(|| Error::Eval(format!("surface display {name} width must be positive")))?;
        let height = integer(height).filter(|value| *value > 0).ok_or_else(|| {
            Error::Eval(format!("surface display {name} height must be positive"))
        })?;
        return width
            .checked_mul(height)
            .map(Some)
            .ok_or_else(|| Error::Eval(format!("surface display {name} area overflowed")));
    }
    Ok(None)
}

fn integer(value: &Expr) -> Option<u64> {
    match value {
        Expr::Number(number)
            if matches!(number.domain.name.as_ref(), "i64" | "u64")
                && number.domain.namespace.is_none() =>
        {
            number.canonical.parse().ok()
        }
        _ => None,
    }
}
