//! Hardware bring-up ledger for glasses lanes.
//!
//! The ledger separates claimed hardware facts from verified route enablement.
//! CI and modeled/stub paths use the default fixture with every lane unverified;
//! a real hardware lane becomes enableable only after its entry is explicitly
//! marked verified with firmware or version evidence.

use sim_kernel::{Error, Expr, Result, Symbol};
use sim_value::{access, build};

/// Namespace for glasses bring-up ledger expressions.
pub const GLASSES_BRINGUP_NAMESPACE: &str = "glasses";

/// Kind tag for glasses bring-up ledger expressions.
pub const GLASSES_BRINGUP_KIND: &str = "bringup";

/// Built-in hardware-free glasses bring-up fixture name.
pub const GLASSES_BRINGUP_FIXTURE: &str = "glasses/bringup";

/// Committed quoted glasses bring-up fixture text.
pub const GLASSES_BRINGUP_FIXTURE_TEXT: &str = include_str!("../glasses/bringup");

/// Viture Carina 6DoF pose lane.
pub const VITURE_CARINA_LANE: &str = "viture_carina";

/// Viture 3DoF IMU lane.
pub const VITURE_LEGACY_IMU_LANE: &str = "viture_legacy";

/// Viture UVC/stereo camera lane.
pub const VITURE_UVC_CAMERA_LANE: &str = "viture_uvc_cam";

/// Halo direct BLE display and input lane.
pub const HALO_BLE_DIRECT_LANE: &str = "halo_ble_direct";

/// Halo Web Bluetooth display and input lane.
pub const HALO_WEB_BLUETOOTH_LANE: &str = "halo_web_bt";

/// Halo phone-relay display and input lane.
pub const HALO_PHONE_RELAY_LANE: &str = "halo_phone_relay";

/// Halo one-shot camera lane.
pub const HALO_CAMERA_LANE: &str = "halo_camera";

/// All hardware lanes that require bring-up verification before real use.
pub const GLASSES_BRINGUP_LANES: [&str; 7] = [
    VITURE_CARINA_LANE,
    VITURE_LEGACY_IMU_LANE,
    VITURE_UVC_CAMERA_LANE,
    HALO_BLE_DIRECT_LANE,
    HALO_WEB_BLUETOOTH_LANE,
    HALO_PHONE_RELAY_LANE,
    HALO_CAMERA_LANE,
];

/// One hardware lane's claimed facts and verification state.
#[derive(Clone, Debug, PartialEq)]
pub struct BringUpEntry {
    /// Stable hardware lane token.
    pub lane: Symbol,
    /// Claimed hardware facts for this lane, stored as authored SIM data.
    pub claims: Expr,
    /// True only after a human hardware audit verifies the lane.
    pub verified: bool,
    /// Firmware identifier recorded with the verification evidence.
    pub firmware: Option<String>,
    /// Hardware, bridge, or provider version recorded with the verification evidence.
    pub version: Option<String>,
    /// Human-readable notes about the lane and its evidence.
    pub notes: Vec<String>,
}

impl BringUpEntry {
    /// Builds an unverified entry for one lane.
    pub fn unverified(lane: &str, claims: Expr, notes: Vec<String>) -> Self {
        Self {
            lane: Symbol::new(lane),
            claims,
            verified: false,
            firmware: None,
            version: None,
            notes,
        }
    }

    /// Encodes the entry as a SIM expression.
    pub fn to_expr(&self) -> Expr {
        build::map(vec![
            ("lane", Expr::Symbol(self.lane.clone())),
            ("claims", self.claims.clone()),
            ("verified", Expr::Bool(self.verified)),
            ("firmware", optional_text(self.firmware.as_deref())),
            ("version", optional_text(self.version.as_deref())),
            ("notes", string_list(&self.notes)),
        ])
    }

    /// Decodes one entry expression whose map key supplied `lane`.
    pub fn from_expr(lane: Symbol, expr: &Expr) -> Result<Self> {
        let context = "glasses bring-up entry";
        let entries = access::map_entries(expr, context)?;
        if let Some(Expr::Symbol(field_lane)) = access::entry_field(entries, "lane")
            && field_lane != &lane
        {
            return Err(Error::Eval(format!(
                "{context} lane {} does not match map key {}",
                field_lane.as_qualified_str(),
                lane.as_qualified_str()
            )));
        }
        let claims = access::entry_required(entries, "claims", context)?.clone();
        access::map_entries(&claims, "glasses bring-up claims")?;
        Ok(Self {
            lane,
            claims,
            verified: access::entry_required_bool(entries, "verified", context)?,
            firmware: required_optional_string(entries, "firmware", context)?,
            version: required_optional_string(entries, "version", context)?,
            notes: optional_string_list(entries, "notes", context)?,
        })
    }
}

/// Hardware bring-up ledger for all glasses lanes.
#[derive(Clone, Debug, PartialEq)]
pub struct BringUpLedger {
    /// One entry per hardware lane.
    pub entries: Vec<BringUpEntry>,
    /// Notes that apply to the full ledger.
    pub notes: Vec<String>,
}

impl BringUpLedger {
    /// Builds the hardware-free default ledger.
    pub fn default_glasses() -> Self {
        Self {
            entries: GLASSES_BRINGUP_LANES
                .iter()
                .copied()
                .map(default_entry)
                .collect(),
            notes: vec![
                "verified flags change only with hardware bring-up ledger evidence".to_owned(),
            ],
        }
    }

    /// Decodes a glasses bring-up ledger expression.
    pub fn from_expr(expr: &Expr) -> Result<Self> {
        let context = "glasses bring-up ledger";
        let entries = access::map_entries(expr, context)?;
        let kind = access::entry_required_sym(entries, "kind", context)?;
        let expected_kind = Symbol::qualified(GLASSES_BRINGUP_NAMESPACE, GLASSES_BRINGUP_KIND);
        if kind != &expected_kind {
            return Err(Error::Eval(format!(
                "{context} kind must be {}",
                expected_kind.as_qualified_str()
            )));
        }

        let lanes =
            access::map_entries(access::entry_required(entries, "lanes", context)?, context)?;
        let mut decoded = Vec::with_capacity(lanes.len());
        for (key, value) in lanes {
            let lane = lane_key(key)?;
            decoded.push(BringUpEntry::from_expr(lane, value)?);
        }
        let ledger = Self {
            entries: decoded,
            notes: optional_string_list(entries, "notes", context)?,
        };
        ledger.require_all_lanes()?;
        Ok(ledger)
    }

    /// Encodes the ledger as a SIM expression.
    pub fn to_expr(&self) -> Expr {
        Expr::Map(vec![
            (
                build::sym("kind"),
                Expr::Symbol(Symbol::qualified(
                    GLASSES_BRINGUP_NAMESPACE,
                    GLASSES_BRINGUP_KIND,
                )),
            ),
            (build::sym("fixture"), build::text(GLASSES_BRINGUP_FIXTURE)),
            (
                build::sym("lanes"),
                Expr::Map(
                    self.entries
                        .iter()
                        .map(|entry| (Expr::Symbol(entry.lane.clone()), entry.to_expr()))
                        .collect(),
                ),
            ),
            (build::sym("notes"), string_list(&self.notes)),
        ])
    }

    /// Returns the entry for `lane`, if the ledger contains it.
    pub fn entry(&self, lane: &str) -> Option<&BringUpEntry> {
        self.entries.iter().find(|entry| lane_matches(entry, lane))
    }

    /// Returns the mutable entry for `lane`, if the ledger contains it.
    pub fn entry_mut(&mut self, lane: &str) -> Option<&mut BringUpEntry> {
        self.entries
            .iter_mut()
            .find(|entry| lane_matches(entry, lane))
    }

    /// Enables a lane only when its verified flag is true.
    pub fn enable_lane(&self, lane: &str) -> Result<()> {
        match self.entry(lane) {
            Some(entry) if entry.verified => Ok(()),
            _ => Err(Error::HostError(format!("lane {lane} not verified"))),
        }
    }

    fn require_all_lanes(&self) -> Result<()> {
        for lane in GLASSES_BRINGUP_LANES {
            if self.entry(lane).is_none() {
                return Err(Error::Eval(format!(
                    "glasses bring-up ledger is missing lane {lane}"
                )));
            }
        }
        Ok(())
    }
}

/// Returns the built-in glasses bring-up fixture names.
pub fn glasses_bringup_fixture_names() -> [&'static str; 1] {
    [GLASSES_BRINGUP_FIXTURE]
}

/// Returns the named glasses bring-up fixture.
pub fn glasses_bringup_fixture(name: &str) -> Option<Expr> {
    match name {
        GLASSES_BRINGUP_FIXTURE => Some(default_glasses_bringup_fixture()),
        _ => None,
    }
}

/// Builds the default glasses bring-up fixture.
pub fn default_glasses_bringup_fixture() -> Expr {
    BringUpLedger::default_glasses().to_expr()
}

fn default_entry(lane: &str) -> BringUpEntry {
    match lane {
        VITURE_CARINA_LANE => BringUpEntry::unverified(
            lane,
            build::map(vec![
                ("device", build::sym("viture-luma-ultra")),
                ("route", build::sym("carina")),
                ("sample", build::sym("pose-6dof")),
            ]),
            vec!["Carina pose frames gate the rich Viture reprojector".to_owned()],
        ),
        VITURE_LEGACY_IMU_LANE => BringUpEntry::unverified(
            lane,
            build::map(vec![
                ("device", build::sym("viture-luma-ultra")),
                ("route", build::sym("legacy-imu")),
                ("sample", build::sym("pose-3dof")),
            ]),
            vec!["3DoF IMU frames support the display-only fallback".to_owned()],
        ),
        VITURE_UVC_CAMERA_LANE => BringUpEntry::unverified(
            lane,
            build::map(vec![
                ("device", build::sym("viture-luma-ultra")),
                ("route", build::sym("uvc-camera")),
                ("sample", build::sym("camera-frame")),
            ]),
            vec!["UVC camera frames require by-reference storage and consent".to_owned()],
        ),
        HALO_BLE_DIRECT_LANE => BringUpEntry::unverified(
            lane,
            build::map(vec![
                ("device", build::sym("halo")),
                ("route", build::sym("ble-direct")),
                ("sample", build::sym("lua-diff-frame")),
            ]),
            vec!["Direct BLE carries bounded Halo display diffs and tap input".to_owned()],
        ),
        HALO_WEB_BLUETOOTH_LANE => BringUpEntry::unverified(
            lane,
            build::map(vec![
                ("device", build::sym("halo")),
                ("route", build::sym("web-bluetooth")),
                ("sample", build::sym("lua-diff-frame")),
            ]),
            vec!["Web Bluetooth carries bounded Halo display diffs and tap input".to_owned()],
        ),
        HALO_PHONE_RELAY_LANE => BringUpEntry::unverified(
            lane,
            build::map(vec![
                ("device", build::sym("halo")),
                ("route", build::sym("phone-relay")),
                ("sample", build::sym("lua-diff-frame")),
            ]),
            vec!["Phone relay carries bounded Halo display diffs and tap input".to_owned()],
        ),
        HALO_CAMERA_LANE => BringUpEntry::unverified(
            lane,
            build::map(vec![
                ("device", build::sym("halo")),
                ("route", build::sym("camera")),
                ("sample", build::sym("camera-frame")),
                (
                    "resolution",
                    build::list(vec![build::uint(640), build::uint(480)]),
                ),
            ]),
            vec!["Halo camera frames stay one-shot and consent-gated".to_owned()],
        ),
        _ => unreachable!("unknown glasses bring-up lane"),
    }
}

fn lane_key(expr: &Expr) -> Result<Symbol> {
    match expr {
        Expr::Symbol(symbol) if symbol.namespace.is_none() => Ok(symbol.clone()),
        Expr::String(value) => Ok(Symbol::new(value.as_str())),
        _ => Err(Error::Eval(
            "glasses bring-up lane key must be a bare symbol or string".to_owned(),
        )),
    }
}

fn lane_matches(entry: &BringUpEntry, lane: &str) -> bool {
    entry.lane.namespace.is_none() && entry.lane.name.as_ref() == lane
}

fn optional_text(value: Option<&str>) -> Expr {
    value.map(build::text).unwrap_or(Expr::Nil)
}

fn string_list(values: &[String]) -> Expr {
    build::list(values.iter().cloned().map(build::text).collect())
}

fn required_optional_string(
    entries: &[(Expr, Expr)],
    name: &str,
    context: &'static str,
) -> Result<Option<String>> {
    match access::entry_required(entries, name, context)? {
        Expr::Nil => Ok(None),
        Expr::String(value) => Ok(Some(value.clone())),
        _ => Err(Error::TypeMismatch {
            expected: "string or nil",
            found: "non-string",
        }),
    }
}

fn optional_string_list(
    entries: &[(Expr, Expr)],
    name: &str,
    context: &'static str,
) -> Result<Vec<String>> {
    let Some(value) = access::entry_field(entries, name) else {
        return Ok(Vec::new());
    };
    let Expr::List(items) = value else {
        return Err(Error::TypeMismatch {
            expected: "list",
            found: "non-list",
        });
    };
    items
        .iter()
        .map(|item| match item {
            Expr::String(value) => Ok(value.clone()),
            _ => Err(Error::TypeMismatch {
                expected: "string",
                found: "non-string",
            }),
        })
        .collect::<Result<Vec<_>>>()
        .map_err(|error| Error::Eval(format!("{context} {name}: {error}")))
}
