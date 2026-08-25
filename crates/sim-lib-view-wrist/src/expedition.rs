//! Policy-only expedition projection over the continuity weave.
//!
//! The projector receives semantic fields and explicit authority facts. It has
//! no device topology, coordinate, pointer, or sensor-frame input surface.

/// A continuity surface role requested by a policy pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurfaceRole {
    /// Complete, visible phone Scene.
    PhoneScene,
    /// Reduced glance surface.
    Glance,
    /// Audible projection.
    Audible,
    /// System notification.
    Notification,
    /// Locked-screen projection.
    LockScreen,
}

/// One semantic field offered for projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisclosureField {
    /// Stable semantic name declared by the pack.
    pub name: String,
    /// Already-rendered semantic value; never a raw sensor frame.
    pub value: String,
    /// Whether the pack declared this field for the requested role.
    pub declared: bool,
    /// Whether policy classifies the field as sensitive.
    pub sensitive: bool,
    /// Whether model-authored content received human review.
    pub reviewed: bool,
}

/// Complete authority intersection at one projection boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AuthorityIntersection {
    /// Mission admits the projection.
    pub mission: bool,
    /// Current passport admits it.
    pub passport: bool,
    /// Route lease is live.
    pub route_lease: bool,
    /// Endpoint grant covers the role.
    pub endpoint_grant: bool,
}

impl AuthorityIntersection {
    fn permits(self) -> bool {
        self.mission && self.passport && self.route_lease && self.endpoint_grant
    }
}

/// Requested attention state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attention {
    /// Expedition focus is visible on the authoritative phone Scene.
    VisibleExpedition,
    /// Focus is absent or belongs elsewhere.
    NotVisible,
}

/// Clutch-qualified semantic input; raw pointer coordinates are not representable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SemanticClutch {
    /// Keyboard command qualified by the expedition clutch.
    Keyboard(String),
    /// Touch action resolved by the visible Scene before entering policy.
    Touch(String),
}

/// Successful disposable expedition projection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpeditionProjection {
    /// Content identity shared by journal, book, closure, recipe, and Scene.
    pub content_id: String,
    /// Admitted semantic fields.
    pub fields: Vec<DisclosureField>,
    /// Visible semantic focus action, when clutch-qualified.
    pub focus_action: Option<String>,
}

/// Every stop or refusal is a normal pack result, never a transport error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContinuityOutcome {
    /// Projection succeeded.
    Projected(ExpeditionProjection),
    /// Caller requested the ordinary stop outcome.
    Stopped { content_id: String },
    /// Policy refused projection while retaining phone/keyboard continuation.
    Refused {
        /// Stable content identity retained by the phone continuation.
        content_id: String,
        /// Stable refusal code.
        reason: &'static str,
    },
}

/// Stateless policy projection for expedition roles.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExpeditionProjector;

impl ExpeditionProjector {
    /// Intersect authority and disclosure into a disposable projection.
    pub fn project(
        self,
        content_id: &str,
        role: SurfaceRole,
        authority: AuthorityIntersection,
        attention: Attention,
        fields: &[DisclosureField],
        clutch: Option<&SemanticClutch>,
    ) -> ContinuityOutcome {
        if !authority.permits() {
            return refused(content_id, "authority-intersection-denied");
        }
        if attention != Attention::VisibleExpedition {
            return refused(content_id, "expedition-focus-not-visible");
        }
        if fields.iter().any(|field| !field.declared) {
            return refused(content_id, "undeclared-sensitive-field");
        }
        let reduced = matches!(
            role,
            SurfaceRole::Glance
                | SurfaceRole::Audible
                | SurfaceRole::Notification
                | SurfaceRole::LockScreen
        );
        if reduced
            && fields.iter().any(|field| {
                field.sensitive
                    || !field.reviewed
                    || matches!(field.name.as_str(), "secret" | "private-note")
            })
        {
            return refused(content_id, "sensitive-field-on-reduced-role");
        }
        ContinuityOutcome::Projected(ExpeditionProjection {
            content_id: content_id.to_owned(),
            fields: fields.to_vec(),
            focus_action: clutch.map(|input| match input {
                SemanticClutch::Keyboard(action) | SemanticClutch::Touch(action) => action.clone(),
            }),
        })
    }

    /// Produce the ordinary stop result without invalidating continuation data.
    pub fn stop(self, content_id: &str) -> ContinuityOutcome {
        ContinuityOutcome::Stopped {
            content_id: content_id.to_owned(),
        }
    }
}

fn refused(content_id: &str, reason: &'static str) -> ContinuityOutcome {
    ContinuityOutcome::Refused {
        content_id: content_id.to_owned(),
        reason,
    }
}

#[cfg(test)]
mod expedition_tests;
