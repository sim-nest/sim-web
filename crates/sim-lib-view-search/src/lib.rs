//! An offline, evidence-preserving projection of canonical search records.
//!
//! This crate owns presentation only. It accepts provider-neutral records from
//! `sim-lib-search-core` and immutable web records from `sim-lib-web-core`; it
//! has no HTTP, provider-codec, transport, credential, or orchestration
//! dependency. Remote links are emitted only as inert labels plus explicit
//! [`SearchAction::RequestOpen`] values for a policy host to decide.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use sim_kernel::{ContentId, Expr};
use sim_lib_scene::{box_, stack, text_node};
use sim_lib_search_core::{Citation, ProviderClaim, SearchRun};
use sim_lib_web_core::{WebCapture, WebRepresentation};

/// Stable projection id advertised through the SIM Index.
pub const SEARCH_AUDIT_SURFACE_ID: &str = "view:search-audit";

/// Device-density choice. All variants expose the same semantic evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// One-column disclosure cards suitable for narrow screens.
    Compact,
    /// Two-column result and evidence layout.
    Tablet,
    /// Result table with a persistent provenance inspector.
    Desktop,
}

/// State retained by the pure surface codec.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewState {
    /// Current layout breakpoint.
    pub layout: Layout,
    /// Result selected by zero-based final order.
    pub selected: Option<usize>,
    /// Results whose provenance disclosures are open.
    pub expanded: Vec<usize>,
    /// True when the record is being replayed without network authority.
    pub offline: bool,
    /// True after cancellation was requested.
    pub cancelled: bool,
}

impl Default for ViewState {
    fn default() -> Self {
        Self {
            layout: Layout::Compact,
            selected: None,
            expanded: Vec::new(),
            offline: true,
            cancelled: false,
        }
    }
}

/// An explicit user action. Rendering itself never fetches or opens a URI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SearchAction {
    /// Replace the query through the search policy host.
    SubmitQuery(String),
    /// Cooperatively cancel the owning search operation.
    Cancel,
    /// Select a result by final-order index.
    Select(usize),
    /// Toggle the provenance disclosure for a result.
    ToggleProvenance(usize),
    /// Replay the immutable audit packet with no network authority.
    ReplayOffline,
    /// Ask the policy host to consider opening an inert URI string.
    RequestOpen {
        /// The inert normalized URI shown to the user.
        uri: String,
        /// Receipt or policy revision the host must evaluate.
        policy_receipt: String,
    },
}

/// Applies a reversible local action, returning host-bound actions unchanged.
pub fn apply_action(state: &mut ViewState, action: SearchAction) -> Option<SearchAction> {
    match action {
        SearchAction::Select(index) => state.selected = Some(index),
        SearchAction::ToggleProvenance(index) => {
            if let Some(at) = state.expanded.iter().position(|value| *value == index) {
                state.expanded.remove(at);
            } else {
                state.expanded.push(index);
                state.expanded.sort_unstable();
            }
        }
        SearchAction::ReplayOffline => state.offline = true,
        SearchAction::Cancel => state.cancelled = true,
        action @ (SearchAction::SubmitQuery(_) | SearchAction::RequestOpen { .. }) => {
            return Some(action);
        }
    }
    None
}

/// Immutable capture and representation pair displayed by the audit surface.
#[derive(Clone, Copy)]
pub struct CaptureEvidence<'a> {
    /// Raw immutable exchange capture.
    pub capture: &'a WebCapture,
    /// Normalized representation derived from `capture`.
    pub representation: &'a WebRepresentation,
    /// Named policy/exchange receipt authorizing the capture.
    pub policy_receipt: &'a str,
    /// Explicit robots decision retained by the fetcher.
    pub robots_outcome: &'a str,
    /// Nonempty extraction warnings; an empty set is rendered as exact fidelity.
    pub fidelity_warnings: &'a [String],
}

/// Canonical records required for a completely offline audit projection.
pub struct AuditRecords<'a> {
    /// Provider-neutral search, alias, and contribution records.
    pub run: &'a SearchRun,
    /// Immutable raw/normalized capture records.
    pub captures: &'a [CaptureEvidence<'a>],
    /// Citations already verified by `sim-lib-search-core`.
    pub citations: &'a [Citation],
    /// Optional office anchors saved by the caller's evidence store.
    pub office_anchors: &'a [String],
    /// Policy revision governing query and open/fetch actions.
    pub policy_revision: &'a str,
    /// Optional final-order judge receipt id; never treated as authority.
    pub judge_receipt: Option<&'a str>,
    /// Fetch decisions and denials retained verbatim as inert evidence.
    pub fetch_decisions: &'a [String],
}

/// Projection failure. A tampered selector fails closed instead of degrading to
/// citation-looking prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuditError {
    /// A citation does not match any supplied normalized representation.
    TamperedAnchor(ContentId),
}

/// Render a bounded semantic Scene. Every untrusted string is placed only in a
/// text node, which browser renderers escape; no markup or resource node is
/// created. Content ids provide visible fence markers around source material.
pub fn render(records: &AuditRecords<'_>, state: &ViewState) -> Result<Expr, AuditError> {
    for citation in records.citations {
        let id = &citation.selector.representation_id;
        let Some(rep) = records
            .captures
            .iter()
            .find(|item| &item.representation.content_id == id)
        else {
            return Err(AuditError::TamperedAnchor(id.clone()));
        };
        if citation.selector.verify(rep.representation).is_err() {
            return Err(AuditError::TamperedAnchor(id.clone()));
        }
    }

    let mut sections = vec![box_(
        "banner",
        vec![
            text_node(format!(
                "Search audit | layout={:?} | offline={} | cancelled={}",
                state.layout, state.offline, state.cancelled
            )),
            text_node(format!("Query: {}", records.run.query.text)),
            text_node(format!("Policy revision: {}", records.policy_revision)),
            text_node(format!(
                "Selected sites: {}",
                records
                    .run
                    .query
                    .sites
                    .iter()
                    .map(|s| s.domain.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        ],
    )];

    let observations: Vec<_> = records
        .run
        .pages
        .iter()
        .flat_map(|page| page.observations.iter())
        .collect();
    for (index, observation) in observations.iter().enumerate() {
        let mut body = vec![
            text_node(format!("Result {} of {}", index + 1, observations.len())),
            inert_uri(&observation.retrieval_uri),
        ];
        if let Some(claim) = &observation.claim {
            body.push(provider_claim(claim));
        }
        if state.expanded.contains(&index) {
            for contribution in records
                .run
                .rank
                .iter()
                .filter(|rank| rank.observation as usize == index)
            {
                body.push(text_node(format!(
                    "Rank contribution | {} | {} | {}",
                    contribution.contributor, contribution.score.canonical, contribution.reason
                )));
            }
            if let Some(judge) = records.judge_receipt {
                body.push(text_node(format!("Optional judge receipt: {judge}")));
            }
            for capture in records
                .captures
                .iter()
                .filter(|item| item.capture.retrieval_uri.as_str() == observation.retrieval_uri)
            {
                body.push(capture_scene(capture, records.citations));
            }
            for decision in records.fetch_decisions {
                body.push(text_node(format!("Fetch decision: {decision}")));
            }
        }
        sections.push(box_(
            if state.selected == Some(index) {
                "selected-result"
            } else {
                "result"
            },
            body,
        ));
    }
    if observations.is_empty() {
        sections.push(box_(
            "empty",
            vec![text_node(
                "No results. Provider failures and omissions remain below.",
            )],
        ));
    }
    sections.push(box_(
        "run-evidence",
        vec![
            text_node(format!(
                "Notices: {}",
                records
                    .run
                    .notices
                    .iter()
                    .map(|n| format!("{}: {}", n.code, n.message))
                    .collect::<Vec<_>>()
                    .join(" | ")
            )),
            text_node(format!(
                "Alias clusters/evidence: {}",
                records
                    .run
                    .aliases
                    .iter()
                    .map(|a| format!("{} <-> {} ({})", a.left_uri, a.right_uri, a.basis))
                    .collect::<Vec<_>>()
                    .join(" | ")
            )),
            text_node(format!(
                "Office anchors: {}",
                records.office_anchors.join(", ")
            )),
        ],
    ));
    Ok(stack("vertical", sections))
}

fn inert_uri(uri: &str) -> Expr {
    text_node(format!("Inert link (activate through policy host): {uri}"))
}

fn provider_claim(claim: &ProviderClaim) -> Expr {
    box_(
        "provider-claim-unverified",
        vec![
            text_node("PROVIDER CLAIM — UNVERIFIED — NOT A CITATION"),
            text_node(fence(
                "provider-claim",
                &format!(
                    "provider={} title={} snippet={}",
                    claim.provider,
                    claim.title.as_deref().unwrap_or(""),
                    claim.snippet.as_deref().unwrap_or("")
                ),
            )),
        ],
    )
}

fn capture_scene(capture: &CaptureEvidence<'_>, citations: &[Citation]) -> Expr {
    let warnings = if capture.fidelity_warnings.is_empty() {
        "Fidelity: exact normalized representation".to_owned()
    } else {
        format!(
            "FIDELITY WARNING: {}",
            capture.fidelity_warnings.join(" | ")
        )
    };
    let mut children = vec![
        text_node("VERIFIED CAPTURE EVIDENCE"),
        text_node(format!("Raw capture id: {:?}", capture.capture.content_id)),
        text_node(format!(
            "Normalized representation id: {:?}",
            capture.representation.content_id
        )),
        text_node(format!(
            "Exchange/policy receipt: {}",
            capture.policy_receipt
        )),
        text_node(format!("Robots outcome: {}", capture.robots_outcome)),
        text_node(warnings),
    ];
    for citation in citations
        .iter()
        .filter(|citation| citation.selector.representation_id == capture.representation.content_id)
    {
        children.push(box_(
            "verified-quotation",
            vec![
                text_node("Verified captured quotation"),
                text_node(fence("captured-quotation", &citation.selector.exact)),
                text_node(format!(
                    "Selector: chars {}..{} on {:?}",
                    citation.selector.start,
                    citation.selector.end,
                    citation.selector.representation_id
                )),
            ],
        ));
    }
    box_("capture-provenance", children)
}

fn fence(label: &str, text: &str) -> String {
    format!("--- BEGIN {label} ---\n{text}\n--- END {label} ---")
}

#[cfg(test)]
mod tests;
