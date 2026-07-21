//! Dual-watch fleet roles, sensor quorum, and two-handed gestures.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_lib_intent::{Origin, WristRawInput, intent, validate_intent};

/// Maximum accepted confidence value, expressed in ten-thousandths.
pub const FLEET_CONFIDENCE_MAX: u16 = 10_000;

/// Stable side identity for a two-watch fleet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WristSide {
    /// The watch worn on the left wrist.
    Left,
    /// The watch worn on the right wrist.
    Right,
}

impl WristSide {
    /// Returns this side's stable role token.
    pub fn side_role(self) -> WornRole {
        match self {
            Self::Left => WornRole::LeftWrist,
            Self::Right => WornRole::RightWrist,
        }
    }
}

/// Current high-level wearer state for role assignment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WornActivity {
    /// The watch is available for foreground field duty.
    Active,
    /// The wearer marked this watch as quiet but still available.
    Quiet,
    /// The watch reports sleep context.
    Sleep,
    /// The watch is not currently worn.
    OffBody,
}

/// Roles a watch can hold inside a two-watch fleet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WornRole {
    /// The foreground watch for field alerts and primary controls.
    Field,
    /// A quiet watch that receives subdued output.
    Quiet,
    /// A sleep-context watch.
    Sleep,
    /// A worn backup for failover and corroboration.
    Backup,
    /// The physical left-wrist identity.
    LeftWrist,
    /// The physical right-wrist identity.
    RightWrist,
    /// An off-body watch retained only as a beacon.
    OffBodyBeacon,
}

/// Live state used to assign one watch in a dual-watch fleet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WatchFleetMember {
    /// The physical wrist side.
    pub side: WristSide,
    /// Current battery percentage.
    pub battery_percent: u8,
    /// Whether the watch is charging.
    pub charging: bool,
    /// Current activity context.
    pub activity: WornActivity,
}

impl WatchFleetMember {
    /// Builds one member of a dual-watch fleet.
    pub fn new(
        side: WristSide,
        battery_percent: u8,
        charging: bool,
        activity: WornActivity,
    ) -> Result<Self> {
        if battery_percent > 100 {
            return Err(Error::HostError(
                "watch fleet battery percent must be between 0 and 100".to_owned(),
            ));
        }
        Ok(Self {
            side,
            battery_percent,
            charging,
            activity,
        })
    }
}

/// The side identity and current duty role assigned to one watch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignedWornRole {
    /// The physical wrist role, either left or right.
    pub side_role: WornRole,
    /// The current fleet duty role.
    pub duty_role: WornRole,
}

/// Role assignment for a left/right pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualWatchRoles {
    /// Role assignment for the left watch.
    pub left: AssignedWornRole,
    /// Role assignment for the right watch.
    pub right: AssignedWornRole,
}

impl DualWatchRoles {
    /// Returns the side currently holding the field role, if any.
    pub fn field_side(&self) -> Option<WristSide> {
        if self.left.duty_role == WornRole::Field {
            Some(WristSide::Left)
        } else if self.right.duty_role == WornRole::Field {
            Some(WristSide::Right)
        } else {
            None
        }
    }
}

/// Assigns side and duty roles for two watches.
///
/// Active watches compete for the field role by battery state, with charging as
/// a strong tie-breaker. The other worn active watch becomes backup; quiet,
/// sleep, and off-body states keep their explicit duty roles.
pub fn assign_worn_roles(
    left: &WatchFleetMember,
    right: &WatchFleetMember,
) -> Result<DualWatchRoles> {
    if left.side != WristSide::Left || right.side != WristSide::Right {
        return Err(Error::HostError(
            "watch fleet role assignment requires left then right members".to_owned(),
        ));
    }
    let field = field_side(left, right);
    Ok(DualWatchRoles {
        left: AssignedWornRole {
            side_role: WristSide::Left.side_role(),
            duty_role: duty_role(left, field),
        },
        right: AssignedWornRole {
            side_role: WristSide::Right.side_role(),
            duty_role: duty_role(right, field),
        },
    })
}

/// UI-level sensor value from one watch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FleetSensorSample {
    side: WristSide,
    sensor: Symbol,
    value: i64,
    confidence: u16,
}

impl FleetSensorSample {
    /// Builds a fleet sensor sample.
    pub fn new(side: WristSide, sensor: Symbol, value: i64, confidence: u16) -> Result<Self> {
        if confidence > FLEET_CONFIDENCE_MAX {
            return Err(Error::HostError(format!(
                "fleet sensor confidence must be <= {FLEET_CONFIDENCE_MAX}"
            )));
        }
        Ok(Self {
            side,
            sensor,
            value,
            confidence,
        })
    }

    /// Returns the watch side that produced the sample.
    pub fn side(&self) -> WristSide {
        self.side
    }

    /// Returns the sensor symbol.
    pub fn sensor(&self) -> &Symbol {
        &self.sensor
    }

    /// Returns the scalar sensor value.
    pub fn value(&self) -> i64 {
        self.value
    }

    /// Returns the sample confidence in ten-thousandths.
    pub fn confidence(&self) -> u16 {
        self.confidence
    }
}

/// Quorum result for two scalar fleet sensor samples.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FleetSensorQuorum {
    /// Both watches agree within the configured tolerance.
    Agree {
        /// Sensor shared by both samples.
        sensor: Symbol,
        /// Averaged value.
        value: i64,
        /// Quorum confidence in ten-thousandths.
        confidence: u16,
    },
    /// The samples diverge enough that the fleet should prefer one side.
    LowConfidence {
        /// Sensor shared by both samples.
        sensor: Symbol,
        /// Higher-confidence side.
        prefer: WristSide,
        /// Value from the preferred side.
        value: i64,
        /// Absolute disagreement between the samples.
        delta: u64,
        /// Lowered quorum confidence in ten-thousandths.
        confidence: u16,
    },
}

impl FleetSensorQuorum {
    /// Returns the quorum confidence in ten-thousandths.
    pub fn confidence(&self) -> u16 {
        match self {
            Self::Agree { confidence, .. } | Self::LowConfidence { confidence, .. } => *confidence,
        }
    }
}

/// Scores two same-sensor scalar samples as one fleet value.
pub fn fleet_sensor_quorum(
    a: &FleetSensorSample,
    b: &FleetSensorSample,
    max_delta: u64,
) -> Result<FleetSensorQuorum> {
    if a.sensor != b.sensor {
        return Err(Error::HostError(format!(
            "fleet sensor quorum requires matching sensors, found {} and {}",
            a.sensor, b.sensor
        )));
    }
    let delta = a.value.abs_diff(b.value);
    let confidence = a.confidence.min(b.confidence);
    if delta > max_delta {
        let prefer = if a.confidence >= b.confidence {
            a.side
        } else {
            b.side
        };
        Ok(FleetSensorQuorum::LowConfidence {
            sensor: a.sensor.clone(),
            prefer,
            value: if prefer == a.side { a.value } else { b.value },
            delta,
            confidence: confidence / 2,
        })
    } else {
        Ok(FleetSensorQuorum::Agree {
            sensor: a.sensor.clone(),
            value: average_i64(a.value, b.value)?,
            confidence,
        })
    }
}

/// Swipe direction used by two-handed gestures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SwipeDirection {
    /// A leftward swipe.
    Left,
    /// A rightward swipe.
    Right,
    /// An upward swipe.
    Up,
    /// A downward swipe.
    Down,
}

/// Physical input used by the two-handed fleet coordinator.
#[derive(Clone, Debug, PartialEq)]
pub enum FleetWristInput {
    /// A tap recognized on one wrist.
    Tap {
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A long press recognized on one wrist.
    LongPress {
        /// Held duration in milliseconds.
        held_ms: u64,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A swipe recognized on one wrist.
    Swipe {
        /// Swipe direction.
        direction: SwipeDirection,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
    /// A stable raise recognized on one wrist.
    Raised {
        /// Stable raise duration in milliseconds.
        stable_ms: u64,
        /// Monotonic event timestamp.
        at_ms: u64,
    },
}

impl FleetWristInput {
    /// Builds a tap input.
    pub fn tap(at_ms: u64) -> Self {
        Self::Tap { at_ms }
    }

    /// Builds a long-press input.
    pub fn long_press(held_ms: u64, at_ms: u64) -> Self {
        Self::LongPress { held_ms, at_ms }
    }

    /// Builds a swipe input.
    pub fn swipe(direction: SwipeDirection, at_ms: u64) -> Self {
        Self::Swipe { direction, at_ms }
    }

    /// Builds a raised-wrist input.
    pub fn raised(stable_ms: u64, at_ms: u64) -> Self {
        Self::Raised { stable_ms, at_ms }
    }

    /// Converts supported single-wrist raw input into fleet input.
    pub fn from_raw(raw: &WristRawInput) -> Option<Self> {
        match raw {
            WristRawInput::Tap { at_ms, .. } => Some(Self::tap(*at_ms)),
            WristRawInput::Button { held_ms, at_ms, .. } => {
                Some(Self::long_press(*held_ms, *at_ms))
            }
            WristRawInput::Raise {
                stable_ms, at_ms, ..
            } => Some(Self::raised(*stable_ms, *at_ms)),
            WristRawInput::Crown { .. } | WristRawInput::Touch { .. } => None,
        }
    }

    /// Returns the monotonic event timestamp.
    pub fn at_ms(&self) -> u64 {
        match self {
            Self::Tap { at_ms }
            | Self::LongPress { at_ms, .. }
            | Self::Swipe { at_ms, .. }
            | Self::Raised { at_ms, .. } => *at_ms,
        }
    }
}

/// Timing thresholds for two-handed gesture composition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoHandedTiming {
    /// Maximum time span between left and right inputs.
    pub window_ms: u64,
    /// Minimum right-button hold duration for cancel.
    pub long_press_ms: u64,
    /// Minimum stable raise duration for opening the palette.
    pub raise_stable_ms: u64,
}

impl Default for TwoHandedTiming {
    fn default() -> Self {
        Self {
            window_ms: 300,
            long_press_ms: 650,
            raise_stable_ms: 180,
        }
    }
}

/// Composes one left and one right wrist input into a standard Intent.
///
/// Left tap plus right tap acknowledges on the nearest rich surface. A right
/// long press plus left swipe cancels the active pane. Both watches raised opens
/// the watch palette on the nearest rich surface.
pub fn two_handed_intent(
    left: &FleetWristInput,
    right: &FleetWristInput,
    origin: Origin,
    pane: Expr,
    nearest_rich_surface: Expr,
    timing: TwoHandedTiming,
) -> Option<Expr> {
    if !within_window(left.at_ms(), right.at_ms(), timing.window_ms) {
        return None;
    }
    if matches!(left, FleetWristInput::Tap { .. }) && matches!(right, FleetWristInput::Tap { .. }) {
        return checked_intent(intent(
            "invoke",
            origin,
            vec![
                ("target", nearest_rich_surface),
                ("op", Expr::Symbol(watch_acknowledge_op())),
                ("args", Expr::List(Vec::new())),
            ],
        ));
    }
    if matches!(left, FleetWristInput::Swipe { .. }) && is_long_press(right, timing.long_press_ms) {
        return checked_intent(intent("cancel", origin, vec![("pane", pane)]));
    }
    if is_raised(left, timing.raise_stable_ms) && is_raised(right, timing.raise_stable_ms) {
        return checked_intent(intent(
            "open",
            origin,
            vec![
                ("value", Expr::Symbol(watch_palette_symbol())),
                ("pane", nearest_rich_surface),
            ],
        ));
    }
    None
}

/// Returns the operation symbol used by the two-handed acknowledge Intent.
pub fn watch_acknowledge_op() -> Symbol {
    Symbol::qualified("watch/fleet", "acknowledge")
}

/// Returns the palette value symbol opened by a two-handed raise.
pub fn watch_palette_symbol() -> Symbol {
    Symbol::qualified("watch/fleet", "palette")
}

fn field_side(left: &WatchFleetMember, right: &WatchFleetMember) -> Option<WristSide> {
    match (
        left.activity == WornActivity::Active,
        right.activity == WornActivity::Active,
    ) {
        (true, true) => Some(if field_score(left) >= field_score(right) {
            WristSide::Left
        } else {
            WristSide::Right
        }),
        (true, false) => Some(WristSide::Left),
        (false, true) => Some(WristSide::Right),
        (false, false) => None,
    }
}

fn field_score(member: &WatchFleetMember) -> u16 {
    u16::from(member.battery_percent) + if member.charging { 101 } else { 0 }
}

fn duty_role(member: &WatchFleetMember, field: Option<WristSide>) -> WornRole {
    match member.activity {
        WornActivity::OffBody => WornRole::OffBodyBeacon,
        WornActivity::Sleep => WornRole::Sleep,
        WornActivity::Quiet => WornRole::Quiet,
        WornActivity::Active if Some(member.side) == field => WornRole::Field,
        WornActivity::Active => WornRole::Backup,
    }
}

fn average_i64(a: i64, b: i64) -> Result<i64> {
    let average = (i128::from(a) + i128::from(b)) / 2;
    average
        .try_into()
        .map_err(|_| Error::HostError("fleet sensor average is out of range".to_owned()))
}

fn within_window(a: u64, b: u64, window_ms: u64) -> bool {
    a.abs_diff(b) <= window_ms
}

fn is_long_press(input: &FleetWristInput, min_ms: u64) -> bool {
    matches!(input, FleetWristInput::LongPress { held_ms, .. } if *held_ms >= min_ms)
}

fn is_raised(input: &FleetWristInput, min_ms: u64) -> bool {
    matches!(input, FleetWristInput::Raised { stable_ms, .. } if *stable_ms >= min_ms)
}

fn checked_intent(expr: Expr) -> Option<Expr> {
    validate_intent(&expr).ok()?;
    Some(expr)
}
