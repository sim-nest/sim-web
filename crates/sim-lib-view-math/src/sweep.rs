//! Parameter sweeps: slider/knob-driven plots with snapshot and compare.
//!
//! A sweep is a parametric series `y = f(x; param)`. A slider or knob drives
//! `param` through `intent/set-param`; each change recomputes the series and
//! re-renders the plot live. Snapshots freeze the current series so several
//! parameter settings can be compared in one overlaid plot.

use sim_kernel::{Error, Expr, Result};
use sim_lib_scene::{node, sym};

use crate::num::{as_f64, number};
use crate::plot::{multi_plot_view, response_plot_view};

/// A live parameter sweep over a built-in linear family `y = param * x`.
pub struct Sweep {
    param: f64,
    samples: usize,
    snapshots: Vec<(String, Vec<(f64, f64)>)>,
}

impl Sweep {
    /// A sweep starting at `param` with `samples` points.
    pub fn new(param: f64, samples: usize) -> Self {
        Self {
            param,
            samples: samples.max(2),
            snapshots: Vec::new(),
        }
    }

    /// The current parameter value.
    pub fn param(&self) -> f64 {
        self.param
    }

    fn series(&self) -> Vec<(f64, f64)> {
        (0..self.samples)
            .map(|i| {
                let x = i as f64;
                (x, self.param * x)
            })
            .collect()
    }

    /// The plot for the current parameter, with the slider control bound to
    /// `intent/set-param`.
    pub fn plot(&self) -> Expr {
        let plot = multi_plot_view(&[(format!("param={}", self.param), self.series())]);
        node(
            "stack",
            vec![
                ("role", sym("sweep")),
                ("dir", sym("column")),
                (
                    "children",
                    Expr::List(vec![
                        node(
                            "slider",
                            vec![
                                ("param", sym("slope")),
                                ("min", number(-5.0)),
                                ("max", number(5.0)),
                                ("value", number(self.param)),
                            ],
                        ),
                        plot,
                    ]),
                ),
            ],
        )
    }

    /// Apply an `intent/set-param`, updating the parameter and returning the new
    /// plot. The plot updates live with the parameter.
    pub fn set_param(&mut self, intent: &Expr) -> Result<Expr> {
        match intent_field(intent, "kind") {
            Some(Expr::Symbol(kind)) if &*kind.name == "set-param" => {}
            _ => return Err(Error::HostError("expected an intent/set-param".to_owned())),
        }
        let value = intent_field(intent, "value")
            .and_then(as_f64)
            .ok_or_else(|| Error::HostError("set-param 'value' must be a number".to_owned()))?;
        self.param = value;
        Ok(self.plot())
    }

    /// Freeze the current series so it can be compared with later settings.
    pub fn snapshot(&mut self) {
        self.snapshots
            .push((format!("param={}", self.param), self.series()));
    }

    /// The number of frozen snapshots.
    pub fn snapshot_count(&self) -> usize {
        self.snapshots.len()
    }

    /// An overlaid plot of every snapshot plus the current series, for compare.
    pub fn compare(&self) -> Expr {
        let mut series = self.snapshots.clone();
        series.push((format!("param={} (current)", self.param), self.series()));
        multi_plot_view(&series)
    }
}

/// Build a labelled response sweep from precomputed series.
pub fn response_sweep_view(
    lens: &str,
    role: &str,
    param: SweepParam<'_>,
    series_list: &[(String, Vec<(f64, f64)>)],
) -> Expr {
    node(
        "stack",
        vec![
            ("lens", sym(lens)),
            ("role", sym(role)),
            ("dir", sym("column")),
            (
                "children",
                Expr::List(vec![
                    node(
                        "slider",
                        vec![
                            ("param", sym(param.name)),
                            ("min", number(param.min)),
                            ("max", number(param.max)),
                            ("value", number(param.value)),
                        ],
                    ),
                    response_plot_view(lens, "response-plot", series_list),
                ]),
            ),
        ],
    )
}

/// The parameter controlled by a response sweep.
pub struct SweepParam<'a> {
    /// Parameter name used by `intent/set-param`.
    pub name: &'a str,
    /// Minimum slider value.
    pub min: f64,
    /// Maximum slider value.
    pub max: f64,
    /// Current slider value.
    pub value: f64,
}

fn intent_field<'a>(intent: &'a Expr, name: &str) -> Option<&'a Expr> {
    let Expr::Map(entries) = intent else {
        return None;
    };
    entries.iter().find_map(|(key, value)| {
        matches!(key, Expr::Symbol(symbol) if &*symbol.name == name && symbol.namespace.is_none())
            .then_some(value)
    })
}
