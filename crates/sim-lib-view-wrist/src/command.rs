//! Watch actuator command vocabulary.

use sim_kernel::{CapabilityName, Cx, Error, Expr, Result, Symbol};
use sim_lib_scene::GlanceCard;
use sim_lib_view_device::{
    ConsentReceipt, EdgeId, PrivacyMode as ReaperPrivacyMode, ReaperDirective,
};
use sim_value::{access, build};

const WATCH_COMMAND_KIND_NS: &str = "view-wrist";
const WATCH_COMMAND_KIND: &str = "command";
const WATCH_COMMAND_NS: &str = "watch/command";
const HAPTIC_PATTERN_KIND: &str = "haptic-pattern";

/// Stable urgency tokens accepted by watch notifications.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Urgency {
    /// Informational notification.
    Info,
    /// Warning notification.
    Warn,
    /// Error notification.
    Error,
    /// Critical notification.
    Critical,
}

impl Urgency {
    /// Returns the stable urgency token.
    pub fn token(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Critical => "critical",
        }
    }

    /// Decodes an urgency from a symbol.
    pub fn from_symbol(symbol: &Symbol) -> Result<Self> {
        match symbol.name.as_ref() {
            "info" => Ok(Self::Info),
            "warn" => Ok(Self::Warn),
            "error" => Ok(Self::Error),
            "critical" => Ok(Self::Critical),
            other => Err(Error::HostError(format!("unknown watch urgency {other}"))),
        }
    }
}

/// One haptic on/off segment in milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HapticStep {
    /// Duration of the active pulse.
    pub on_ms: u16,
    /// Duration of the gap after the pulse.
    pub off_ms: u16,
}

impl HapticStep {
    /// Builds one haptic segment.
    pub fn new(on_ms: u16, off_ms: u16) -> Self {
        Self { on_ms, off_ms }
    }

    fn to_expr(&self) -> Expr {
        build::map(vec![
            ("on-ms", build::uint(u64::from(self.on_ms))),
            ("off-ms", build::uint(u64::from(self.off_ms))),
        ])
    }

    fn from_expr(expr: &Expr) -> Result<Self> {
        ensure_no_extra(expr, &["on-ms", "off-ms"], "watch haptic step")?;
        Ok(Self {
            on_ms: field_u16(expr, "on-ms", "watch haptic step")?,
            off_ms: field_u16(expr, "off-ms", "watch haptic step")?,
        })
    }
}

/// Named haptic pattern with a semantic meaning tag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HapticPattern {
    /// Stable pattern id.
    pub id: Symbol,
    /// Ordered on/off pulse steps.
    pub steps: Vec<HapticStep>,
    /// Meaning tag, such as `watch/haptic-meaning/confirm`.
    pub meaning: Symbol,
    /// Repeat count for the full step list.
    pub repeat: u8,
}

impl HapticPattern {
    /// Builds a haptic pattern.
    pub fn new(id: Symbol, steps: Vec<HapticStep>, meaning: Symbol, repeat: u8) -> Result<Self> {
        if steps.is_empty() {
            return Err(Error::HostError(
                "watch haptic pattern requires at least one step".to_owned(),
            ));
        }
        if repeat == 0 {
            return Err(Error::HostError(
                "watch haptic pattern repeat must be greater than zero".to_owned(),
            ));
        }
        Ok(Self {
            id,
            steps,
            meaning,
            repeat,
        })
    }

    /// Encodes this pattern as expression data.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            (
                "kind",
                build::qsym(WATCH_COMMAND_KIND_NS, HAPTIC_PATTERN_KIND),
            ),
            ("id", Expr::Symbol(self.id.clone())),
            (
                "steps",
                build::list(self.steps.iter().map(HapticStep::to_expr).collect()),
            ),
            ("meaning", Expr::Symbol(self.meaning.clone())),
            ("repeat", build::uint(u64::from(self.repeat))),
        ])
    }

    /// Decodes a haptic pattern, rejecting unknown fields and commands.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        ensure_kind(
            expr,
            WATCH_COMMAND_KIND_NS,
            HAPTIC_PATTERN_KIND,
            "watch haptic pattern",
        )?;
        ensure_no_extra(
            expr,
            &["kind", "id", "steps", "meaning", "repeat"],
            "watch haptic pattern",
        )?;
        let id = access::required_sym(expr, "id", "watch haptic pattern")?;
        let steps = required_list(expr, "steps", "watch haptic pattern")?
            .iter()
            .map(HapticStep::from_expr)
            .collect::<Result<Vec<_>>>()?;
        let meaning = access::required_sym(expr, "meaning", "watch haptic pattern")?;
        let repeat = field_u8(expr, "repeat", "watch haptic pattern")?;
        Self::new(id, steps, meaning, repeat)
    }
}

/// One command sent through the watch provider's actuator path.
#[derive(Clone, Debug, PartialEq)]
pub enum WatchCommand {
    /// Render a notification on the wrist.
    Notify {
        /// Notification title.
        title: String,
        /// Compact body lines.
        lines: Vec<String>,
        /// Notification urgency.
        urgency: Urgency,
    },
    /// Play a named haptic pattern.
    Haptic {
        /// Pattern to play.
        pattern: HapticPattern,
    },
    /// Set a watch-face data slot.
    SetFaceSlot {
        /// Slot id on the active face.
        slot: String,
        /// Slot value as SIM data.
        value: Expr,
    },
    /// Set or update an alarm.
    SetAlarm {
        /// Stable alarm id.
        id: String,
        /// Alarm time in modeled milliseconds.
        at_ms: u64,
        /// Alarm label.
        label: String,
    },
    /// Toggle privacy mode for sensitive worn streams.
    PrivacyMode {
        /// Whether privacy mode is active.
        enabled: bool,
        /// Redaction retention window in milliseconds.
        window_ms: u64,
    },
}

impl WatchCommand {
    /// Builds a notification command from a `scene/glance` card.
    pub fn notify_from_glance(glance: &Expr) -> Result<Self> {
        let card = GlanceCard::from_scene(glance)?;
        let mut lines = Vec::new();
        if let Some(metric) = card.metric {
            lines.push(format!("{}: {}", metric.label, metric.value));
        }
        if let Some(action) = card.action {
            lines.push(action.label);
        }
        Ok(Self::Notify {
            title: card.title,
            lines,
            urgency: urgency_from_token(&card.urgency)?,
        })
    }

    /// Encodes this command as fail-closed expression data.
    pub fn to_expr(&self) -> Expr {
        match self {
            Self::Notify {
                title,
                lines,
                urgency,
            } => build::map(vec![
                (
                    "kind",
                    build::qsym(WATCH_COMMAND_KIND_NS, WATCH_COMMAND_KIND),
                ),
                ("command", build::qsym(WATCH_COMMAND_NS, "notify")),
                ("title", Expr::String(title.clone())),
                (
                    "lines",
                    build::list(
                        lines
                            .iter()
                            .map(|line| Expr::String(line.clone()))
                            .collect(),
                    ),
                ),
                ("urgency", build::sym(urgency.token())),
            ]),
            Self::Haptic { pattern } => build::map(vec![
                (
                    "kind",
                    build::qsym(WATCH_COMMAND_KIND_NS, WATCH_COMMAND_KIND),
                ),
                ("command", build::qsym(WATCH_COMMAND_NS, "haptic")),
                ("pattern", pattern.to_expr()),
            ]),
            Self::SetFaceSlot { slot, value } => build::map(vec![
                (
                    "kind",
                    build::qsym(WATCH_COMMAND_KIND_NS, WATCH_COMMAND_KIND),
                ),
                ("command", build::qsym(WATCH_COMMAND_NS, "set-face-slot")),
                ("slot", Expr::String(slot.clone())),
                ("value", value.clone()),
            ]),
            Self::SetAlarm { id, at_ms, label } => build::map(vec![
                (
                    "kind",
                    build::qsym(WATCH_COMMAND_KIND_NS, WATCH_COMMAND_KIND),
                ),
                ("command", build::qsym(WATCH_COMMAND_NS, "set-alarm")),
                ("id", Expr::String(id.clone())),
                ("at-ms", build::uint(*at_ms)),
                ("label", Expr::String(label.clone())),
            ]),
            Self::PrivacyMode { enabled, window_ms } => build::map(vec![
                (
                    "kind",
                    build::qsym(WATCH_COMMAND_KIND_NS, WATCH_COMMAND_KIND),
                ),
                ("command", build::qsym(WATCH_COMMAND_NS, "privacy-mode")),
                ("enabled", Expr::Bool(*enabled)),
                ("window-ms", build::uint(*window_ms)),
            ]),
        }
    }

    /// Decodes a command, rejecting unknown command symbols and fields.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        ensure_kind(
            expr,
            WATCH_COMMAND_KIND_NS,
            WATCH_COMMAND_KIND,
            "watch command",
        )?;
        let command = command_symbol(expr)?;
        match command.name.as_ref() {
            "notify" => {
                ensure_command_fields(expr, &["title", "lines", "urgency"])?;
                Ok(Self::Notify {
                    title: access::required_str(expr, "title", "watch notify command")?.to_owned(),
                    lines: string_list(
                        required_list(expr, "lines", "watch notify command")?,
                        "watch notify command lines",
                    )?,
                    urgency: Urgency::from_symbol(&access::required_sym(
                        expr,
                        "urgency",
                        "watch notify command",
                    )?)?,
                })
            }
            "haptic" => {
                ensure_command_fields(expr, &["pattern"])?;
                Ok(Self::Haptic {
                    pattern: HapticPattern::from_expr(access::required(
                        expr,
                        "pattern",
                        "watch haptic command",
                    )?)?,
                })
            }
            "set-face-slot" => {
                ensure_command_fields(expr, &["slot", "value"])?;
                Ok(Self::SetFaceSlot {
                    slot: access::required_str(expr, "slot", "watch face-slot command")?.to_owned(),
                    value: access::required(expr, "value", "watch face-slot command")?.clone(),
                })
            }
            "set-alarm" => {
                ensure_command_fields(expr, &["id", "at-ms", "label"])?;
                Ok(Self::SetAlarm {
                    id: access::required_str(expr, "id", "watch alarm command")?.to_owned(),
                    at_ms: field_u64(expr, "at-ms", "watch alarm command")?,
                    label: access::required_str(expr, "label", "watch alarm command")?.to_owned(),
                })
            }
            "privacy-mode" => {
                ensure_command_fields(expr, &["enabled", "window-ms"])?;
                Ok(Self::PrivacyMode {
                    enabled: access::required_bool(expr, "enabled", "watch privacy command")?,
                    window_ms: field_u64(expr, "window-ms", "watch privacy command")?,
                })
            }
            other => Err(Error::HostError(format!("unknown watch command {other}"))),
        }
    }

    /// Returns the capability required before sending this actuator command.
    pub fn capability_name(&self) -> CapabilityName {
        CapabilityName::new(match self {
            Self::Notify { .. } => "watch/notify",
            Self::Haptic { .. } => "watch/haptic",
            Self::SetFaceSlot { .. } => "watch/face",
            Self::SetAlarm { .. } => "watch/alarm",
            Self::PrivacyMode { .. } => "watch/privacy",
        })
    }

    /// Requires the command's actuator grant from the current context.
    pub fn require_grant(&self, cx: &Cx) -> Result<()> {
        cx.require(&self.capability_name())
    }

    /// Returns the privacy-mode reaper directive when the command enables privacy.
    pub fn as_reaper_directive(&self) -> Option<ReaperDirective> {
        match self {
            Self::PrivacyMode { enabled: true, .. } => Some(ReaperDirective {
                mode: ReaperPrivacyMode::Redact,
                redact: privacy_redact_symbols(),
            }),
            _ => None,
        }
    }

    /// Applies an enabled privacy-mode command to a session consent receipt.
    pub fn privacy_consent_receipt(&self, session: EdgeId, seq: u64) -> Option<ConsentReceipt> {
        match self {
            Self::PrivacyMode {
                enabled: true,
                window_ms,
            } => Some(ConsentReceipt::new(
                vec![Symbol::qualified("watch", "privacy")],
                *window_ms,
                privacy_redact_symbols(),
                session,
                seq,
            )),
            _ => None,
        }
    }

    /// Renders the visible privacy active-state badge for an enabled command.
    pub fn privacy_badge_scene(&self) -> Option<Expr> {
        matches!(self, Self::PrivacyMode { enabled: true, .. })
            .then(|| sim_lib_scene::badge("warn", "privacy active"))
    }
}

fn ensure_command_fields(expr: &Expr, variant_fields: &[&str]) -> Result<()> {
    let mut fields = Vec::with_capacity(2 + variant_fields.len());
    fields.extend(["kind", "command"]);
    fields.extend(variant_fields.iter().copied());
    ensure_no_extra(expr, &fields, "watch command")
}

fn ensure_kind(expr: &Expr, namespace: &str, name: &str, context: &str) -> Result<()> {
    match access::field_sym(expr, "kind") {
        Some(kind)
            if kind.namespace.as_deref() == Some(namespace) && kind.name.as_ref() == name =>
        {
            Ok(())
        }
        _ => Err(Error::HostError(format!(
            "expected {namespace}/{name} {context}"
        ))),
    }
}

fn command_symbol(expr: &Expr) -> Result<Symbol> {
    let command = access::required_sym(expr, "command", "watch command")?;
    if command.namespace.as_deref() != Some(WATCH_COMMAND_NS) {
        return Err(Error::HostError(format!(
            "watch command symbol must be in {WATCH_COMMAND_NS}"
        )));
    }
    Ok(command)
}

fn ensure_no_extra(expr: &Expr, known: &[&str], context: &str) -> Result<()> {
    let Expr::Map(entries) = expr else {
        return Err(Error::TypeMismatch {
            expected: "map",
            found: "non-map",
        });
    };
    for (key, _) in entries {
        let allowed = match key {
            Expr::Symbol(symbol) if symbol.namespace.is_none() => {
                known.contains(&symbol.name.as_ref())
            }
            Expr::String(text) => known.contains(&text.as_str()),
            _ => false,
        };
        if !allowed {
            return Err(Error::HostError(format!(
                "{context} has unknown field {key:?}"
            )));
        }
    }
    Ok(())
}

fn required_list<'a>(expr: &'a Expr, field: &str, context: &str) -> Result<&'a [Expr]> {
    match access::required(expr, field, context)? {
        Expr::List(items) => Ok(items),
        _ => Err(Error::HostError(format!(
            "{context} field {field} is not a list"
        ))),
    }
}

fn string_list(items: &[Expr], context: &str) -> Result<Vec<String>> {
    items
        .iter()
        .map(|item| match item {
            Expr::String(line) => Ok(line.clone()),
            _ => Err(Error::HostError(format!("{context} contains a non-string"))),
        })
        .collect()
}

fn field_u64(expr: &Expr, field: &str, context: &str) -> Result<u64> {
    match access::required(expr, field, context)? {
        Expr::Number(number) if matches!(number.domain.name.as_ref(), "u64" | "i64") => number
            .canonical
            .parse()
            .map_err(|_| Error::HostError(format!("{context} field {field} is not u64"))),
        _ => Err(Error::HostError(format!(
            "{context} field {field} is not u64"
        ))),
    }
}

fn field_u16(expr: &Expr, field: &str, context: &str) -> Result<u16> {
    field_u64(expr, field, context)?
        .try_into()
        .map_err(|_| Error::HostError(format!("{context} field {field} is not u16")))
}

fn field_u8(expr: &Expr, field: &str, context: &str) -> Result<u8> {
    field_u64(expr, field, context)?
        .try_into()
        .map_err(|_| Error::HostError(format!("{context} field {field} is not u8")))
}

fn urgency_from_token(token: &str) -> Result<Urgency> {
    Urgency::from_symbol(&Symbol::new(token))
}

fn privacy_redact_symbols() -> Vec<Symbol> {
    vec![
        Symbol::qualified("watch", "health"),
        Symbol::qualified("watch", "location"),
    ]
}
