use super::*;
use sim_kernel::{ContentId, Datum, NumberLiteral, Symbol};
use sim_lib_net_core::normalize_retrieval_uri;
use sim_lib_search_core::{RankContribution, SearchNotice, SearchPage, SearchQuery, SearchSite};
use sim_lib_web_core::{
    DecodeLimits, RepresentationMetadata, WebCapture, WebExchange, WebRepresentation,
};

fn fixture() -> (SearchRun, WebCapture, WebRepresentation, Citation) {
    let query = SearchQuery::checked(
        "SIM <script>alert(1)</script>".into(),
        vec![SearchSite {
            domain: "example.test".into(),
            include_subdomains: false,
        }],
        None,
        10,
    )
    .unwrap();
    let uri = normalize_retrieval_uri(
        "https://example.test/%D9%85%D8%B1%D8%AD%D8%A8%D8%A7?long=abcdefghijklmnopqrstuvwxyz",
    )
    .unwrap();
    let body = b"verified quotation".to_vec();
    let raw_id = Datum::Bytes(body.clone()).content_id().unwrap();
    let capture = WebCapture::checked(
        uri.clone(),
        raw_id,
        body,
        WebExchange {
            method: "GET".into(),
            status: 200,
            final_uri: uri.as_str().into(),
            media_type: Some("text/plain".into()),
            received_bytes: 18,
        },
        DecodeLimits::default(),
    )
    .unwrap();
    let rep = WebRepresentation::checked(
        capture.content_id.clone(),
        "verified quotation".into(),
        RepresentationMetadata {
            codec: "text".into(),
            codec_version: "1".into(),
            media_type: "text/plain".into(),
            charset: Some("utf-8".into()),
            language: None,
            fidelity_warnings: vec!["script nodes removed".into()],
        },
        DecodeLimits::default(),
    )
    .unwrap();
    let selector = rep.select(0, 8).unwrap();
    let citation = Citation::checked(&rep, selector).unwrap();
    let observation = sim_lib_search_core::SearchObservation::checked(
        capture.retrieval_uri.as_str(),
        Some(ProviderClaim {
            provider: "fixture".into(),
            uri: capture.retrieval_uri.as_str().into(),
            title: Some("<img src=x>".into()),
            snippet: Some("not verified".into()),
            position: Some(1),
        }),
        Some(capture.content_id.clone()),
    )
    .unwrap();
    let run = SearchRun {
        query: query.clone(),
        pages: vec![SearchPage {
            query,
            observations: vec![observation],
            continuation: None,
        }],
        notices: vec![SearchNotice {
            code: "partial".into(),
            message: "one engine offline".into(),
        }],
        aliases: vec![],
        rank: vec![RankContribution {
            observation: 0,
            contributor: "site/fixture".into(),
            score: NumberLiteral {
                domain: Symbol::qualified("core", "decimal"),
                canonical: "0.5".into(),
            },
            reason: "reciprocal rank".into(),
        }],
    };
    (run, capture, rep, citation)
}

#[test]
fn snapshot_distinguishes_claims_quotes_and_full_provenance() {
    let (run, capture, rep, citation) = fixture();
    let warnings = vec!["script nodes removed".to_owned()];
    let evidence = [CaptureEvidence {
        capture: &capture,
        representation: &rep,
        policy_receipt: "policy:7/exchange:9",
        robots_outcome: "allowed from immutable robots receipt",
        fidelity_warnings: &warnings,
    }];
    let records = AuditRecords {
        run: &run,
        captures: &evidence,
        citations: &[citation],
        office_anchors: &["doc:research#p4".into()],
        policy_revision: "policy:7",
        judge_receipt: Some("judge:2"),
        fetch_decisions: &["selected by final order; fetched".into()],
    };
    for layout in [Layout::Compact, Layout::Tablet, Layout::Desktop] {
        let scene = render(
            &records,
            &ViewState {
                layout,
                selected: Some(0),
                expanded: vec![0],
                offline: true,
                cancelled: false,
            },
        )
        .unwrap();
        let snapshot = format!("{scene:?}");
        assert!(snapshot.contains("PROVIDER CLAIM — UNVERIFIED — NOT A CITATION"));
        assert!(snapshot.contains("Verified captured quotation"));
        assert!(snapshot.contains("FIDELITY WARNING"));
        assert!(snapshot.contains("Rank contribution"));
        assert!(snapshot.contains("Raw capture id"));
        assert!(snapshot.contains("policy:7/exchange:9"));
        assert!(!snapshot.contains("scene/image"));
    }
}

#[test]
fn actions_round_trip_without_ambient_effects() {
    let mut state = ViewState::default();
    assert_eq!(apply_action(&mut state, SearchAction::Select(4)), None);
    assert_eq!(
        apply_action(&mut state, SearchAction::ToggleProvenance(4)),
        None
    );
    assert_eq!(apply_action(&mut state, SearchAction::Cancel), None);
    assert!(state.cancelled && state.expanded == [4] && state.selected == Some(4));
    let open = SearchAction::RequestOpen {
        uri: "https://example.test".into(),
        policy_receipt: "denied:offline".into(),
    };
    assert_eq!(apply_action(&mut state, open.clone()), Some(open));
    let query = SearchAction::SubmitQuery("new query".into());
    assert_eq!(apply_action(&mut state, query.clone()), Some(query));
}

#[test]
fn tampered_anchor_fails_closed_and_empty_offline_is_usable() {
    let (run, capture, rep, mut citation) = fixture();
    citation.selector.representation_id =
        ContentId::from_bytes(Symbol::qualified("core", "sha256"), [7; 32]);
    let evidence = [CaptureEvidence {
        capture: &capture,
        representation: &rep,
        policy_receipt: "denied",
        robots_outcome: "denied",
        fidelity_warnings: &[],
    }];
    let records = AuditRecords {
        run: &run,
        captures: &evidence,
        citations: &[citation],
        office_anchors: &[],
        policy_revision: "offline",
        judge_receipt: None,
        fetch_decisions: &["denied by policy".into()],
    };
    assert!(matches!(
        render(&records, &ViewState::default()),
        Err(AuditError::TamperedAnchor(_))
    ));
    let empty = SearchRun {
        pages: vec![],
        rank: vec![],
        ..run
    };
    let records = AuditRecords {
        run: &empty,
        captures: &[],
        citations: &[],
        office_anchors: &[],
        policy_revision: "offline",
        judge_receipt: None,
        fetch_decisions: &[],
    };
    let snapshot = format!("{:?}", render(&records, &ViewState::default()).unwrap());
    assert!(snapshot.contains("No results"));
}
// conformance: search-view tests prove inert provenance projection and explicit activation.
