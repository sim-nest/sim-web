use std::sync::Arc;

use sim_kernel::{CapabilitySet, Cx, DefaultFactory, EagerPolicy, Error, Expr, Symbol};
use sim_lib_scene::{GlanceAction, GlanceCard, GlanceMetric, validate_scene};
use sim_lib_view_device::{EdgeId, PrivacyMode};
use sim_value::{access, build};

use crate::{HapticPattern, HapticStep, Urgency, WatchCommand};

#[test]
fn watch_commands_roundtrip_and_reject_unknown_forms() {
    let commands = vec![
        WatchCommand::Notify {
            title: "Storm cell".to_owned(),
            lines: vec!["ETA 18 min".to_owned(), "Tap for route".to_owned()],
            urgency: Urgency::Warn,
        },
        WatchCommand::Haptic {
            pattern: confirm_pattern(),
        },
        WatchCommand::SetFaceSlot {
            slot: "upper-left".to_owned(),
            value: build::text("72 bpm"),
        },
        WatchCommand::SetAlarm {
            id: "morning".to_owned(),
            at_ms: 31_200_000,
            label: "Run".to_owned(),
        },
        WatchCommand::PrivacyMode {
            enabled: true,
            window_ms: 120_000,
        },
    ];

    for command in commands {
        assert_eq!(
            WatchCommand::from_expr(&command.to_expr()).unwrap(),
            command
        );
    }

    let unknown_command = build::map(vec![
        ("kind", build::qsym("view-wrist", "command")),
        ("command", build::qsym("watch/command", "unknown")),
    ]);
    assert!(WatchCommand::from_expr(&unknown_command).is_err());

    let extra_field = build::map(vec![
        ("kind", build::qsym("view-wrist", "command")),
        ("command", build::qsym("watch/command", "notify")),
        ("title", build::text("Hi")),
        ("lines", build::list(Vec::new())),
        ("urgency", build::sym("info")),
        ("ignored-timer", build::uint(5)),
    ]);
    assert!(WatchCommand::from_expr(&extra_field).is_err());

    let qualified_extra = Expr::Map(vec![
        (build::sym("kind"), build::qsym("view-wrist", "command")),
        (
            build::sym("command"),
            build::qsym("watch/command", "privacy-mode"),
        ),
        (build::sym("enabled"), Expr::Bool(true)),
        (build::sym("window-ms"), build::uint(5)),
        (build::qsym("watch/private", "timer"), build::uint(5)),
    ]);
    assert!(WatchCommand::from_expr(&qualified_extra).is_err());

    let bad_pattern = build::map(vec![
        ("kind", build::qsym("view-wrist", "haptic-pattern")),
        ("id", build::qsym("watch/haptic", "empty")),
        ("steps", build::list(Vec::new())),
        ("meaning", build::qsym("watch/haptic-meaning", "confirm")),
        ("repeat", build::uint(1)),
    ]);
    assert!(HapticPattern::from_expr(&bad_pattern).is_err());
}

#[test]
fn notification_renders_from_scene_glance_card() {
    let card = GlanceCard::new(
        "Weather alert",
        Some(GlanceMetric::new("rain", "12 mm")),
        Some(GlanceAction::new("Dismiss", build::sym("dismiss"))),
        "warn",
        1,
    )
    .to_scene();

    let command = WatchCommand::notify_from_glance(&card).unwrap();
    assert_eq!(
        command,
        WatchCommand::Notify {
            title: "Weather alert".to_owned(),
            lines: vec!["rain: 12 mm".to_owned(), "Dismiss".to_owned()],
            urgency: Urgency::Warn,
        }
    );
    assert_eq!(
        WatchCommand::from_expr(&command.to_expr()).unwrap(),
        command
    );
}

#[test]
fn privacy_mode_is_reaper_directive_and_visible_badge() {
    let command = WatchCommand::PrivacyMode {
        enabled: true,
        window_ms: 30_000,
    };
    let directive = command.as_reaper_directive().expect("directive");
    assert_eq!(directive.mode, PrivacyMode::Redact);
    assert_eq!(
        directive.redact,
        vec![
            Symbol::qualified("watch", "health"),
            Symbol::qualified("watch", "location"),
        ]
    );

    let receipt = command
        .privacy_consent_receipt(EdgeId::named("trex"), 9)
        .expect("receipt");
    assert_eq!(receipt.retain_ms, 30_000);
    assert_eq!(receipt.redact, directive.redact);
    assert_eq!(receipt.seq, 9);

    let badge = command.privacy_badge_scene().expect("badge");
    validate_scene(&badge).unwrap();
    assert_eq!(
        access::field_sym(&badge, "status")
            .expect("status")
            .name
            .as_ref(),
        "warn"
    );
    assert_eq!(access::field_str(&badge, "label"), Some("privacy active"));

    let disabled = WatchCommand::PrivacyMode {
        enabled: false,
        window_ms: 30_000,
    };
    assert!(disabled.as_reaper_directive().is_none());
    assert!(disabled.privacy_badge_scene().is_none());
}

#[test]
fn watch_command_send_is_grant_gated() {
    let command = WatchCommand::Haptic {
        pattern: confirm_pattern(),
    };
    assert_eq!(command.capability_name().as_str(), "watch/haptic");

    let cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    assert!(matches!(
        command.require_grant(&cx),
        Err(Error::CapabilityDenied { .. })
    ));

    let granted = CapabilitySet::new().grant(command.capability_name());
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    cx.with_capabilities(granted, |cx| command.require_grant(cx))
        .unwrap();
}

fn confirm_pattern() -> HapticPattern {
    HapticPattern::new(
        Symbol::qualified("watch/haptic", "confirm"),
        vec![HapticStep::new(30, 20), HapticStep::new(30, 0)],
        Symbol::qualified("watch/haptic-meaning", "confirm"),
        1,
    )
    .unwrap()
}
