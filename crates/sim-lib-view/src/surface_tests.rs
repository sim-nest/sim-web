use super::*;

#[test]
fn every_preset_round_trips() {
    for name in SURFACE_PRESETS {
        let caps = preset(name).expect("preset exists");
        assert_eq!(caps.preset_name(), *name);
        let back = SurfaceCaps::from_expr(&caps.to_expr()).expect("round-trips");
        assert_eq!(caps, back, "{name} caps must round-trip losslessly");
    }
}

#[test]
fn unknown_preset_is_none() {
    assert!(preset("hologram").is_none());
}

#[test]
fn from_preset_overrides_client_id() {
    let caps = SurfaceCaps::from_preset("cli", "tty.local.7").unwrap();
    assert_eq!(caps.client_id, "tty.local.7");
    assert_eq!(caps.preset_name(), "cli");
}

#[test]
fn capability_accessors_read_fields() {
    let cli = preset("cli").unwrap();
    assert!(cli.input_flag("keyboard"));
    assert!(!cli.input_flag("touch"));
    assert_eq!(cli.display_density().unwrap().name.as_ref(), "dense");
    assert!(cli.accepts_codec("lisp"));
    assert!(!cli.accepts_codec("algol"));

    let watch = preset("watch").unwrap();
    assert!(watch.input_flag("haptic-ack"));
    assert_eq!(watch.display_density().unwrap().name.as_ref(), "glance");
}

#[test]
fn surface_map_field_wrong_shape_fails_closed() {
    // A caps map whose `display` field is not a map must fail closed with a
    // located `BadField`, never partial caps.
    let mut entries = match preset("cli").unwrap().to_expr() {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    for (key, value) in entries.iter_mut() {
        if matches!(key, Expr::Symbol(symbol) if &*symbol.name == "display") {
            *value = Expr::Bool(true);
        }
    }
    assert_eq!(
        SurfaceCaps::from_expr(&Expr::Map(entries)),
        Err(SurfaceError::BadField("display"))
    );
}

#[test]
fn surface_map_field_missing_flows_through_sim_value_reader() {
    // A missing map field is reported by the shared `sim_value::access`
    // reader, adopted as `SurfaceError::Field` via `From<sim_value::Error>`.
    let mut entries = match preset("cli").unwrap().to_expr() {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    entries.retain(|(key, _)| !matches!(key, Expr::Symbol(s) if &*s.name == "transport"));
    match SurfaceCaps::from_expr(&Expr::Map(entries)) {
        Err(SurfaceError::Field(message)) => assert!(message.contains("transport")),
        other => panic!("expected a located field error, got {other:?}"),
    }
}

#[test]
fn parse_fails_closed() {
    assert_eq!(
        SurfaceCaps::from_expr(&Expr::Nil),
        Err(SurfaceError::NotCaps)
    );
    // A caps map missing `codecs` must not yield partial caps.
    let mut entries = match preset("cli").unwrap().to_expr() {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    entries.retain(|(key, _)| !matches!(key, Expr::Symbol(s) if &*s.name == "codecs"));
    assert_eq!(
        SurfaceCaps::from_expr(&Expr::Map(entries)),
        Err(SurfaceError::MissingField("codecs"))
    );
}

#[test]
fn missing_rate_map_defaults_to_safe_envelope() {
    let mut entries = match preset("cli").unwrap().to_expr() {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    entries.retain(|(key, _)| !matches!(key, Expr::Symbol(s) if &*s.name == "rate"));
    let caps = SurfaceCaps::from_expr(&Expr::Map(entries)).expect("older caps parse");
    assert_eq!(caps.rate, rate_map(1, 1, 1000));
}

#[test]
fn missing_output_and_stream_maps_default_to_empty_metadata() {
    let mut entries = match preset("watch-glance-large").unwrap().to_expr() {
        Expr::Map(entries) => entries,
        _ => unreachable!(),
    };
    entries.retain(|(key, _)| {
        !matches!(key, Expr::Symbol(s) if matches!(s.name.as_ref(), "output" | "streams"))
    });

    let caps = SurfaceCaps::from_expr(&Expr::Map(entries)).expect("older caps parse");

    assert_eq!(caps.output, build::map(Vec::new()));
    assert_eq!(caps.streams, build::map(Vec::new()));
}
