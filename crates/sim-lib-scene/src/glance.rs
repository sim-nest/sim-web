//! One-card Scene helpers for tiny glance surfaces.
//!
//! A `scene/glance` is the reduced, portable card shape shared by small device
//! surfaces. It is still an ordinary Scene node: a tagged map with open fields,
//! not a renderer-specific widget.

use sim_kernel::{Error, Expr, Result};
use sim_value::{access, build};

use crate::model::{node, validate_scene};

/// The local scene kind name for one-card glance nodes.
pub const GLANCE_KIND: &str = "glance";

/// One metric row on a glance card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlanceMetric {
    /// Metric label.
    pub label: String,
    /// Metric value rendered as compact text.
    pub value: String,
}

impl GlanceMetric {
    /// Builds a metric row.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }

    fn to_expr(&self) -> Expr {
        build::map(vec![
            ("label", Expr::String(self.label.clone())),
            ("value", Expr::String(self.value.clone())),
        ])
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        Ok(Self {
            label: access::required_str(expr, "label", "scene/glance metric")?.to_owned(),
            value: access::required_str(expr, "value", "scene/glance metric")?.to_owned(),
        })
    }
}

/// The single primary action on a glance card.
#[derive(Clone, Debug, PartialEq)]
pub struct GlanceAction {
    /// Action label shown on the card.
    pub label: String,
    /// Stable action target/control token as Scene data.
    pub target: Expr,
}

impl GlanceAction {
    /// Builds an action row.
    pub fn new(label: impl Into<String>, target: Expr) -> Self {
        Self {
            label: label.into(),
            target,
        }
    }

    fn to_expr(&self) -> Expr {
        build::map(vec![
            ("label", Expr::String(self.label.clone())),
            ("target", self.target.clone()),
        ])
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        Ok(Self {
            label: access::required_str(expr, "label", "scene/glance action")?.to_owned(),
            target: access::required(expr, "target", "scene/glance action")?.clone(),
        })
    }
}

/// A single portable card for HUD, watch, and other tiny display surfaces.
#[derive(Clone, Debug, PartialEq)]
pub struct GlanceCard {
    /// The card title.
    pub title: String,
    /// Optional single metric row.
    pub metric: Option<GlanceMetric>,
    /// Optional single action row.
    pub action: Option<GlanceAction>,
    /// Urgency token such as `info`, `warn`, or `error`.
    pub urgency: String,
    /// Abstract cell budget, not a device unit.
    pub cells: u16,
    /// Whether safety-critical content bypasses local trimming.
    pub bypass_budget: bool,
}

impl GlanceCard {
    /// Builds a card with optional metric/action rows.
    pub fn new(
        title: impl Into<String>,
        metric: Option<GlanceMetric>,
        action: Option<GlanceAction>,
        urgency: impl Into<String>,
        cells: u16,
    ) -> Self {
        Self {
            title: title.into(),
            metric,
            action,
            urgency: urgency.into(),
            cells,
            bypass_budget: false,
        }
    }

    /// Marks this card as exempt from local glyph trimming.
    pub fn with_budget_bypass(mut self, bypass: bool) -> Self {
        self.bypass_budget = bypass;
        self
    }

    /// Encodes the card as a `scene/glance` node.
    pub fn to_scene(&self) -> Expr {
        let mut entries = vec![
            ("title", Expr::String(self.title.clone())),
            ("urgency", build::sym(&self.urgency)),
            ("cells", build::uint(u64::from(self.cells))),
            ("bypass-budget", Expr::Bool(self.bypass_budget)),
        ];
        if let Some(metric) = &self.metric {
            entries.push(("metric", metric.to_expr()));
        }
        if let Some(action) = &self.action {
            entries.push(("action", action.to_expr()));
        }
        node(GLANCE_KIND, entries)
    }

    /// Reads a `scene/glance` node back into a typed card.
    pub fn from_scene(expr: &Expr) -> Result<Self> {
        validate_scene(expr)
            .map_err(|err| Error::HostError(format!("invalid scene/glance: {err}")))?;
        match crate::model::node_kind(expr) {
            Some(kind)
                if kind.namespace.as_deref() == Some(crate::kinds::SCENE_NAMESPACE)
                    && kind.name.as_ref() == GLANCE_KIND => {}
            _ => {
                return Err(Error::HostError("expected a scene/glance card".to_owned()));
            }
        }
        let cells = field_u16(expr, "cells").unwrap_or(1);
        Ok(Self {
            title: access::required_str(expr, "title", "scene/glance")?.to_owned(),
            metric: access::field(expr, "metric")
                .map(GlanceMetric::from_expr)
                .transpose()?,
            action: access::field(expr, "action")
                .map(GlanceAction::from_expr)
                .transpose()?,
            urgency: access::field_sym(expr, "urgency")
                .map(|symbol| symbol.name.to_string())
                .unwrap_or_else(|| "info".to_owned()),
            cells,
            bypass_budget: access::field_bool(expr, "bypass-budget").unwrap_or(false),
        })
    }
}

fn field_u16(expr: &Expr, name: &str) -> Option<u16> {
    let Expr::Number(number) = access::field(expr, name)? else {
        return None;
    };
    number.canonical.parse().ok()
}

/// Builds a `scene/glance` node.
pub fn glance_card(
    title: impl Into<String>,
    metric: Option<GlanceMetric>,
    action: Option<GlanceAction>,
    urgency: impl Into<String>,
    cells: u16,
) -> Expr {
    GlanceCard::new(title, metric, action, urgency, cells).to_scene()
}
