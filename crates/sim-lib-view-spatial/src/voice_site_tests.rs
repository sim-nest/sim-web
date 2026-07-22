use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sim_kernel::{
    CapabilitySet, Consistency, Cx, DefaultFactory, EagerPolicy, EvalFabric, EvalMode, EvalReply,
    EvalRequest, Expr, Result, Symbol,
};
use sim_lib_intent::{Origin, field, intent, intent_kind_of, validate_intent};
use sim_lib_view_device::{ConsentReceipt, EdgeId};
use sim_value::build;

use crate::{
    AsrSite, AsrSitePlacement, XrMicChunkRef, glasses_mic_capability, glasses_mic_grant,
    voice_intent_via_site,
};

#[test]
fn voice_needs_site_and_session_bound_consent() {
    let calls = Arc::new(AtomicUsize::new(0));
    let fabric = RecordingGlassesAsrSite {
        calls: Arc::clone(&calls),
    };
    let site = AsrSite::phone_relay(&fabric);
    let chunk = chunk_ref();
    assert_eq!(XrMicChunkRef::from_expr(&chunk.to_expr()).unwrap(), chunk);
    assert_eq!(site.placement(), AsrSitePlacement::PhoneRelay);

    let session = EdgeId::named("halo");
    let granted = CapabilitySet::new().grant(glasses_mic_capability());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));

    let missing_visible = ConsentReceipt::new(Vec::new(), 1_000, Vec::new(), session.clone(), 1);
    let denied = cx.with_capabilities(granted.clone(), |cx| {
        voice_intent_via_site(cx, &chunk, Some(&site), &missing_visible, &session)
    });
    assert!(format!("{}", denied.unwrap_err()).contains("visible consent"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let replayed = ConsentReceipt::new(
        vec![glasses_mic_grant()],
        1_000,
        Vec::new(),
        EdgeId::named("other"),
        2,
    );
    let denied = cx.with_capabilities(granted.clone(), |cx| {
        voice_intent_via_site(cx, &chunk, Some(&site), &replayed, &session)
    });
    assert!(format!("{}", denied.unwrap_err()).contains("not for this session"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let receipt = ConsentReceipt::new(
        vec![glasses_mic_grant()],
        1_000,
        Vec::new(),
        session.clone(),
        3,
    );
    let unavailable = cx.with_capabilities(granted.clone(), |cx| {
        voice_intent_via_site(cx, &chunk, None, &receipt, &session)
    });
    assert!(format!("{}", unavailable.unwrap_err()).contains("voice unavailable"));
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let voice = cx
        .with_capabilities(granted, |cx| {
            voice_intent_via_site(cx, &chunk, Some(&site), &receipt, &session)
        })
        .expect("glasses mic consent permits ASR site realization");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        intent_kind_of(&voice).expect("intent kind").name.as_ref(),
        "invoke"
    );
    assert_eq!(
        field(&voice, "op"),
        Some(&Expr::Symbol(Symbol::qualified(
            "glasses/voice",
            "modeled-asr"
        )))
    );
    assert!(!matches!(
        field(&voice, "args"),
        Some(Expr::List(items)) if items.iter().any(|item| matches!(item, Expr::String(_)))
    ));
    validate_intent(&voice).expect("ASR output is a normal Intent");
}

#[test]
fn xr_mic_chunk_ref_rejects_audio_and_transcript_side_channels() {
    for field_name in ["transcript", "text", "pcm", "frames"] {
        let mut entries = match chunk_ref().to_expr() {
            Expr::Map(entries) => entries,
            _ => unreachable!(),
        };
        entries.push((build::sym(field_name), build::text("bypass")));

        let err = XrMicChunkRef::from_expr(&Expr::Map(entries)).unwrap_err();
        assert!(format!("{err}").contains("unexpected field"));
    }
}

fn chunk_ref() -> XrMicChunkRef {
    XrMicChunkRef::new(
        Symbol::qualified("xr/mic-chunk", "fixture-42"),
        42,
        16_000,
        1,
        4096,
    )
    .unwrap()
}

struct RecordingGlassesAsrSite {
    calls: Arc<AtomicUsize>,
}

impl EvalFabric for RecordingGlassesAsrSite {
    fn realize(&self, cx: &mut Cx, request: EvalRequest) -> Result<EvalReply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.consistency, Consistency::LocalFirst);
        assert_eq!(request.mode, EvalMode::Eval);
        assert_eq!(
            request.required_capabilities,
            vec![glasses_mic_capability()]
        );
        let chunk = XrMicChunkRef::from_expr(&request.expr).expect("ASR receives a chunk ref");
        Ok(EvalReply {
            value: cx.factory().expr(intent(
                "invoke",
                Origin::agent(chunk.seq),
                vec![
                    ("target", build::sym("focused")),
                    (
                        "op",
                        Expr::Symbol(Symbol::qualified("glasses/voice", "modeled-asr")),
                    ),
                    (
                        "args",
                        build::list(vec![
                            Expr::Symbol(chunk.ref_id),
                            build::map(vec![("bytes", build::uint(chunk.byte_len))]),
                        ]),
                    ),
                ],
            ))?,
            diagnostics: Vec::new(),
            trace: None,
        })
    }
}
