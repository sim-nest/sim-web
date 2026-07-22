use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use sim_kernel::{
    CapabilitySet, Consistency, Cx, DefaultFactory, EagerPolicy, EvalFabric, EvalMode, EvalReply,
    EvalRequest, Expr, Result, Symbol,
};
use sim_lib_intent::{Origin, field, intent_kind_of, validate_intent};
use sim_lib_view_device::{ConsentReceipt, EdgeId};
use sim_value::build;

use crate::{AudioFrame, MicCapture, transcribe_via_site, watch_mic_capability, watch_mic_grant};

#[test]
fn voice_intent_only_from_model_site_and_consent_gated() {
    let calls = Arc::new(AtomicUsize::new(0));
    let site = RecordingAsrSite {
        calls: Arc::clone(&calls),
    };
    let mic = capture();
    assert_eq!(MicCapture::from_expr(&mic.to_expr()).unwrap(), mic);

    let session = EdgeId::named("trex");
    let receipt = ConsentReceipt::new(
        vec![watch_mic_grant()],
        1_000,
        Vec::new(),
        session.clone(),
        7,
    );
    let granted = CapabilitySet::new().grant(watch_mic_capability());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    let voice = cx
        .with_capabilities(granted, |cx| {
            transcribe_via_site(
                cx,
                &mic,
                &site,
                &receipt,
                &session,
                Origin::human(11),
                build::sym("focused"),
            )
        })
        .expect("watch mic consent permits ASR site realization");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        intent_kind_of(&voice).expect("intent kind").name.as_ref(),
        "invoke"
    );
    assert_eq!(
        field(&voice, "op"),
        Some(&Expr::Symbol(Symbol::qualified(
            "watch/voice",
            "transcript"
        )))
    );
    assert_eq!(
        field(&voice, "args"),
        Some(&Expr::List(vec![Expr::String(
            "start trail recording".to_owned()
        )]))
    );
    validate_intent(&voice).expect("ASR output becomes a normal Intent");
}

#[test]
fn watch_mic_consent_fails_before_realize() {
    let calls = Arc::new(AtomicUsize::new(0));
    let site = RecordingAsrSite {
        calls: Arc::clone(&calls),
    };
    let mic = capture();
    let session = EdgeId::named("trex");
    let granted = CapabilitySet::new().grant(watch_mic_capability());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));

    let missing_visible = ConsentReceipt::new(Vec::new(), 1_000, Vec::new(), session.clone(), 1);
    let denied = cx.with_capabilities(granted.clone(), |cx| {
        transcribe_via_site(
            cx,
            &mic,
            &site,
            &missing_visible,
            &session,
            Origin::human(12),
            build::sym("focused"),
        )
    });
    assert!(denied.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    let other_session = EdgeId::named("other");
    let replayed =
        ConsentReceipt::new(vec![watch_mic_grant()], 1_000, Vec::new(), other_session, 2);
    let denied = cx.with_capabilities(granted, |cx| {
        transcribe_via_site(
            cx,
            &mic,
            &site,
            &replayed,
            &session,
            Origin::human(13),
            build::sym("focused"),
        )
    });
    assert!(denied.is_err());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn mic_capture_rejects_embedded_transcripts() {
    let mut entries = match capture().to_expr() {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    entries.push((build::sym("transcript"), build::text("not from ASR")));

    assert!(MicCapture::from_expr(&Expr::Map(entries)).is_err());
}

fn capture() -> MicCapture {
    MicCapture::new(
        42,
        16_000,
        1,
        vec![
            AudioFrame::new(100, vec![1, 2, 3, 4]).unwrap(),
            AudioFrame::new(120, vec![5, 6]).unwrap(),
        ],
    )
    .unwrap()
}

struct RecordingAsrSite {
    calls: Arc<AtomicUsize>,
}

impl EvalFabric for RecordingAsrSite {
    fn realize(&self, cx: &mut Cx, request: EvalRequest) -> Result<EvalReply> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        assert_eq!(request.consistency, Consistency::LocalFirst);
        assert_eq!(request.mode, EvalMode::Eval);
        assert_eq!(request.required_capabilities, vec![watch_mic_capability()]);
        MicCapture::from_expr(&request.expr).expect("ASR receives raw mic capture");
        Ok(EvalReply {
            value: cx.factory().expr(build::map(vec![
                ("kind", build::qsym("asr", "transcript")),
                ("text", build::text("start trail recording")),
            ]))?,
            diagnostics: Vec::new(),
            trace: None,
        })
    }
}
