//! Glance-card reduction for one-card device tiers.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_scene::{GLANCE_KIND, GlanceAction, GlanceCard, GlanceMetric};
use sim_value::{access, build};

use crate::{DeviceProfile, DeviceTier};

/// Local acknowledgement channel used by a glance adapter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AckChannel {
    /// A haptic pulse.
    Haptic,
    /// A brief glyph flash.
    GlyphFlash,
    /// A short tone.
    Tone,
}

impl AckChannel {
    /// Stable token for serialized ack metadata.
    pub fn token(self) -> &'static str {
        match self {
            AckChannel::Haptic => "haptic",
            AckChannel::GlyphFlash => "glyph-flash",
            AckChannel::Tone => "tone",
        }
    }

    /// Encodes the channel as an unqualified symbol.
    pub fn to_symbol(self) -> Symbol {
        Symbol::new(self.token())
    }
}

/// Abstract budget for fitting one glance card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlanceBudget {
    /// Abstract cells available for the card.
    pub cells: u8,
    /// Maximum rendered glyphs for compact text fields.
    pub glyphs: u16,
    /// Ack channel used for immediate local feedback.
    pub ack: AckChannel,
}

impl GlanceBudget {
    /// Tiny monochrome HUD budget.
    pub fn mono_hud() -> Self {
        Self {
            cells: 2,
            glyphs: 12,
            ack: AckChannel::GlyphFlash,
        }
    }

    /// Round watch-face budget.
    pub fn round_watch() -> Self {
        Self {
            cells: 4,
            glyphs: 24,
            ack: AckChannel::Haptic,
        }
    }
}

/// Device-local input that may need immediate local acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlanceInput {
    /// A tap/press acknowledgement.
    Tap,
}

impl GlanceInput {
    /// Stable token for serialized input metadata.
    pub fn token(self) -> &'static str {
        match self {
            GlanceInput::Tap => "tap",
        }
    }
}

/// State consumed by [`crate::GlanceAdapter`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GlanceState {
    /// Pending input to acknowledge locally.
    pub pending_input: Option<GlanceInput>,
    /// Modeled device tick.
    pub tick: u64,
}

impl GlanceState {
    /// Builds a state with no pending input.
    pub fn idle(tick: u64) -> Self {
        Self {
            pending_input: None,
            tick,
        }
    }

    /// Builds a state carrying one pending input.
    pub fn with_input(input: GlanceInput, tick: u64) -> Self {
        Self {
            pending_input: Some(input),
            tick,
        }
    }
}

/// Reduces general Scenes to one-card `scene/glance` nodes for small tiers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GlanceReducer;

impl GlanceReducer {
    /// Reduces `scene` when `profile` is a one-card tier.
    pub fn reduce(&self, scene: &Expr, profile: &DeviceProfile) -> Result<Expr> {
        reduce_scene_to_glance(scene, profile)
    }
}

/// Reduces a Scene to one `scene/glance` for non-rich device tiers.
pub fn reduce_scene_to_glance(scene: &Expr, profile: &DeviceProfile) -> Result<Expr> {
    sim_lib_scene::validate_scene(scene)
        .map_err(|err| Error::HostError(format!("invalid source scene: {err}")))?;
    if profile.tier == DeviceTier::Rich {
        return Ok(scene.clone());
    }
    Ok(extract_card(scene).to_scene())
}

/// Fits a glance card to a local budget without changing its semantic fields.
pub fn fit_to_budget(glance: &Expr, budget: &GlanceBudget) -> Result<Expr> {
    let mut card = GlanceCard::from_scene(glance)?;
    if !card.bypass_budget {
        let cap = usize::from(budget.glyphs);
        card.title = trim_glyphs(&card.title, cap);
        if let Some(metric) = &mut card.metric {
            metric.label = trim_glyphs(&metric.label, cap / 2);
            metric.value = trim_glyphs(&metric.value, cap);
        }
        if let Some(action) = &mut card.action {
            action.label = trim_glyphs(&action.label, cap / 2);
        }
    }
    card.cells = u16::from(budget.cells);
    Ok(card.to_scene())
}

fn extract_card(scene: &Expr) -> GlanceCard {
    let title = first_text_like(scene).unwrap_or_else(|| {
        kind_name(scene)
            .map(|kind| kind.replace('-', " "))
            .unwrap_or_else(|| "Scene".to_owned())
    });
    let urgency = urgency(scene);
    let bypass = bypass_budget(scene);
    GlanceCard::new(title, first_metric(scene), first_action(scene), urgency, 1)
        .with_budget_bypass(bypass)
}

fn kind_name(expr: &Expr) -> Option<String> {
    let kind = sim_lib_scene::node_kind(expr)?;
    (kind.namespace.as_deref() == Some(sim_lib_scene::kinds::SCENE_NAMESPACE))
        .then(|| kind.name.to_string())
}

fn first_text_like(expr: &Expr) -> Option<String> {
    for field in ["title", "text", "label", "id"] {
        if let Some(text) = access::field_str(expr, field) {
            return Some(text.to_owned());
        }
        if let Some(symbol) = access::field_sym(expr, field) {
            return Some(symbol.name.to_string());
        }
    }
    children(expr).find_map(first_text_like)
}

fn first_metric(expr: &Expr) -> Option<GlanceMetric> {
    if matches!(kind_name(expr).as_deref(), Some("meter" | "badge" | "plot"))
        || access::field(expr, "value").is_some()
    {
        let label = access::field_str(expr, "label")
            .map(str::to_owned)
            .or_else(|| access::field_sym(expr, "status").map(|symbol| symbol.name.to_string()))
            .or_else(|| kind_name(expr))
            .unwrap_or_else(|| "value".to_owned());
        let value = access::field_str(expr, "value")
            .map(str::to_owned)
            .or_else(|| access::field_i64(expr, "value").map(|number| number.to_string()))
            .or_else(|| access::field_str(expr, "text").map(str::to_owned))
            .or_else(|| access::field_str(expr, "label").map(str::to_owned))
            .unwrap_or_else(|| label.clone());
        return Some(GlanceMetric::new(label, value));
    }
    children(expr).find_map(first_metric)
}

fn first_action(expr: &Expr) -> Option<GlanceAction> {
    if kind_name(expr).as_deref() == Some("button") {
        let label = access::field_str(expr, "label")
            .map(str::to_owned)
            .or_else(|| access::field_str(expr, "text").map(str::to_owned))
            .unwrap_or_else(|| "Open".to_owned());
        let target = access::field(expr, "target")
            .cloned()
            .or_else(|| access::field(expr, "control").cloned())
            .unwrap_or_else(|| build::sym("tap"));
        return Some(GlanceAction::new(label, target));
    }
    children(expr).find_map(first_action)
}

fn urgency(expr: &Expr) -> String {
    if let Some(status) = access::field_sym(expr, "status") {
        let token = status.name.as_ref();
        if matches!(token, "error" | "warn" | "critical" | "ok" | "info") {
            return token.to_owned();
        }
    }
    children(expr)
        .find_map(|child| {
            let value = urgency(child);
            (value != "info").then_some(value)
        })
        .unwrap_or_else(|| "info".to_owned())
}

fn bypass_budget(expr: &Expr) -> bool {
    matches!(kind_name(expr).as_deref(), Some("warrant"))
        || access::field_sym(expr, "status")
            .is_some_and(|status| matches!(status.name.as_ref(), "error" | "critical" | "warrant"))
        || children(expr).any(bypass_budget)
}

fn children(expr: &Expr) -> impl Iterator<Item = &Expr> {
    ["children", "nodes", "rows", "items"]
        .into_iter()
        .filter_map(|field| access::field(expr, field))
        .flat_map(|value| match value {
            Expr::List(items) | Expr::Vector(items) => items.as_slice(),
            _ => &[][..],
        })
}

fn trim_glyphs(text: &str, cap: usize) -> String {
    if cap == 0 {
        return String::new();
    }
    let mut out = String::new();
    for (index, ch) in text.chars().enumerate() {
        if index >= cap {
            break;
        }
        out.push(ch);
    }
    out
}

pub(crate) fn is_glance(expr: &Expr) -> bool {
    kind_name(expr).as_deref() == Some(GLANCE_KIND)
}
