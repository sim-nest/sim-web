use super::{
    MAX_BODY_BYTES, MAX_HEAD_LINE_BYTES, MAX_HEADER_COUNT, ReadOutcome, RequestLine, query_value,
    read_request_from, write_session_close, write_session_intent, write_session_open,
};
use crate::live::{DefaultLiveSurfaceFactory, LiveSessionTable, decode_intent_body};
use std::io::{BufReader, Cursor};

fn parse(raw: &str) -> ReadOutcome {
    let mut reader = BufReader::new(Cursor::new(raw.as_bytes().to_vec()));
    read_request_from(&mut reader).expect("read")
}

#[test]
fn oversized_content_length_is_rejected_before_allocation() {
    let raw = "POST /api/session/intent HTTP/1.1\r\nContent-Length: 4000000000\r\n\r\n";
    assert!(
        matches!(parse(raw), ReadOutcome::TooLarge),
        "an oversized Content-Length must yield TooLarge (413)"
    );
}

#[test]
fn content_length_at_the_cap_boundary_is_rejected_when_over() {
    let over = MAX_BODY_BYTES + 1;
    let raw = format!("POST /x HTTP/1.1\r\nContent-Length: {over}\r\n\r\n");
    assert!(matches!(parse(&raw), ReadOutcome::TooLarge));
}

#[test]
fn an_oversized_request_line_is_rejected_before_growing_memory() {
    let mut raw = String::from("GET /");
    raw.push_str(&"a".repeat(MAX_HEAD_LINE_BYTES + 16));
    raw.push_str(" HTTP/1.1\r\n\r\n");
    assert!(
        matches!(parse(&raw), ReadOutcome::TooLarge),
        "an oversized request line must yield TooLarge (413)"
    );
}

#[test]
fn an_oversized_header_line_is_rejected_before_growing_memory() {
    let mut raw = String::from("GET /x HTTP/1.1\r\nX-Big: ");
    raw.push_str(&"a".repeat(MAX_HEAD_LINE_BYTES + 16));
    raw.push_str("\r\n\r\n");
    assert!(
        matches!(parse(&raw), ReadOutcome::TooLarge),
        "an oversized header line must yield TooLarge (413)"
    );
}

#[test]
fn too_many_header_lines_are_rejected() {
    let mut raw = String::from("GET /x HTTP/1.1\r\n");
    for _ in 0..(MAX_HEADER_COUNT + 8) {
        raw.push_str("X-Pad: 1\r\n");
    }
    raw.push_str("\r\n");
    assert!(
        matches!(parse(&raw), ReadOutcome::TooLarge),
        "an endless header block must yield TooLarge (413)"
    );
}

#[test]
fn empty_input_is_invalid_not_a_panic() {
    assert!(
        matches!(parse(""), ReadOutcome::Invalid),
        "an empty request must yield Invalid (400)"
    );
}

#[test]
fn a_small_body_within_the_cap_reads() {
    let raw = "POST /x HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello";
    match parse(raw) {
        ReadOutcome::Request(line) => {
            assert_eq!(line.method, "POST");
            assert_eq!(line.body, "hello");
        }
        other => panic!("expected a parsed request, got {other:?}"),
    }
}

#[test]
fn query_values_are_percent_decoded() {
    assert_eq!(
        query_value(
            "/api/session/open?resource=demo%2Fone&pane=pane%20main",
            "resource"
        )
        .unwrap(),
        Some("demo/one".to_owned())
    );
    assert_eq!(
        query_value(
            "/api/session/open?resource=demo%2Fone&pane=pane%20main",
            "pane"
        )
        .unwrap(),
        Some("pane main".to_owned())
    );
    assert_eq!(
        query_value("/api/session/open?resource=demo", "pane").unwrap(),
        None
    );
}

#[test]
fn malformed_query_percent_escape_is_an_error() {
    let error = query_value("/api/session/open?resource=bad%2", "resource")
        .expect_err("bad escape must fail closed");
    assert!(error.to_string().contains("incomplete percent escape"));
}

#[test]
fn malformed_session_open_query_returns_bad_request() {
    let request = RequestLine {
        method: "GET".to_owned(),
        target: "/api/session/open?resource=bad%ZZ".to_owned(),
        body: String::new(),
    };
    let mut response = Vec::new();
    let mut live = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory::new(
        sim_kernel::HandleSeed::new(0x5745_4270),
    )));
    write_session_open(&mut response, &request, &mut live).expect("response");
    let text = String::from_utf8(response).expect("utf-8 response");
    assert!(
        text.starts_with("HTTP/1.1 400 Bad Request"),
        "malformed query must return 400, got {text}"
    );
    assert!(
        text.contains("malformed query value"),
        "structured JSON error must describe the query problem: {text}"
    );
}

fn body(text: &str) -> &str {
    text.split("\r\n\r\n").nth(1).unwrap_or("")
}

fn json_body(text: &str) -> serde_json::Value {
    serde_json::from_str(body(text)).expect("json body")
}

fn session_from_open(text: &str) -> String {
    json_body(text)["session"]
        .as_str()
        .expect("session id")
        .to_owned()
}

fn open_request(target: &str, live: &mut LiveSessionTable) -> String {
    let request = RequestLine {
        method: "GET".to_owned(),
        target: target.to_owned(),
        body: String::new(),
    };
    let mut response = Vec::new();
    write_session_open(&mut response, &request, live).expect("open response");
    String::from_utf8(response).expect("utf-8 response")
}

fn intent_request(target: &str, live: &mut LiveSessionTable, value: &str) -> String {
    let request = RequestLine {
        method: "POST".to_owned(),
        target: target.to_owned(),
        body: format!(
            r#"{{"kind":"intent/edit-field","origin":{{"operator":"human","at-tick":1}},"target":{{}},"path":[],"value":"{value}"}}"#
        ),
    };
    let mut response = Vec::new();
    write_session_intent(&mut response, &request, live).expect("intent response");
    String::from_utf8(response).expect("utf-8 response")
}

#[test]
fn session_open_returns_an_opaque_session_id() {
    let mut live = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory::new(
        sim_kernel::HandleSeed::new(0x5745_4271),
    )));
    let response = open_request("/api/session/open?resource=demo&pane=pane-main", &mut live);
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    let session_id = session_from_open(&response);
    assert_eq!(session_id.len(), 32);
    assert!(json_body(&response).get("scene").is_some());
}

#[test]
fn session_intent_requires_a_well_formed_session_id() {
    let mut live = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory::new(
        sim_kernel::HandleSeed::new(0x5745_4272),
    )));
    let missing = intent_request("/api/session/intent", &mut live, "x");
    assert!(missing.starts_with("HTTP/1.1 400 Bad Request"), "{missing}");
    assert!(missing.contains("missing session id"));

    let malformed = intent_request("/api/session/intent?session=bad", &mut live, "x");
    assert!(
        malformed.starts_with("HTTP/1.1 400 Bad Request"),
        "{malformed}"
    );
    assert!(malformed.contains("malformed session id"));
}

#[test]
fn sessions_cannot_commit_across_browser_ids() {
    let mut live = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory::new(
        sim_kernel::HandleSeed::new(0x5745_4273),
    )));
    let left = session_from_open(&open_request("/api/session/open", &mut live));
    let right_open = open_request("/api/session/open", &mut live);
    let right = session_from_open(&right_open);

    let left_edit = intent_request(
        &format!("/api/session/intent?session={left}&pane=pane-main"),
        &mut live,
        "left-only",
    );
    assert!(left_edit.starts_with("HTTP/1.1 200 OK"), "{left_edit}");

    let right_reconnect = open_request(
        &format!("/api/session/open?session={right}&resource=demo&pane=pane-main"),
        &mut live,
    );
    assert_eq!(
        json_body(&right_reconnect)["scene"],
        json_body(&right_open)["scene"],
        "one browser cannot read another browser's edited state"
    );
}

#[test]
fn closed_session_ids_are_cancelled() {
    let mut live = LiveSessionTable::new(Box::new(DefaultLiveSurfaceFactory::new(
        sim_kernel::HandleSeed::new(0x5745_4274),
    )));
    let session_id = session_from_open(&open_request("/api/session/open", &mut live));
    let request = RequestLine {
        method: "POST".to_owned(),
        target: format!("/api/session/close?session={session_id}"),
        body: String::new(),
    };
    let mut response = Vec::new();
    write_session_close(&mut response, &request, &mut live).expect("close response");
    let text = String::from_utf8(response).expect("utf-8 response");
    assert!(text.starts_with("HTTP/1.1 200 OK"), "{text}");

    let after_close = intent_request(
        &format!("/api/session/intent?session={session_id}"),
        &mut live,
        "after-close",
    );
    assert!(after_close.starts_with("HTTP/1.1 400 Bad Request"));
    assert!(after_close.contains("unknown session id"));
}

#[test]
fn decoded_browser_intent_is_still_accepted_by_session_route() {
    let intent = decode_intent_body(
        r#"{"kind":"intent/edit-field","origin":{"operator":"human","at-tick":1},"target":{},"path":[],"value":"ok"}"#,
    )
    .expect("browser intent decodes");
    assert!(format!("{intent:?}").contains("edit-field"));
}
