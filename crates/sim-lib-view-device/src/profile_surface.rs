//! Surface capability conversion for device profiles.

use sim_kernel::{Expr, Symbol};
use sim_lib_view::SurfaceCaps;
use sim_value::build;

use crate::profile::{DeviceProfile, has_symbol};

impl DeviceProfile {
    /// Converts this profile back into open surface capabilities for `client_id`.
    ///
    /// This is the bridge used by device peers: the device keeps the typed
    /// profile for routing and tier decisions, while the synchronized surface
    /// hub receives ordinary [`SurfaceCaps`] and continues to project scenes by
    /// open capability metadata.
    pub fn to_surface_caps(&self, client_id: impl Into<String>) -> SurfaceCaps {
        SurfaceCaps {
            client_id: client_id.into(),
            preset: Symbol::qualified("surface", self.kind.name.clone()),
            display: display_map_from_profile(&self.display),
            input: flags_map(&self.input),
            transport: transport_map_from_profile(&self.links),
            privacy: self.policy.clone(),
            rate: self.rate.to_expr(),
            codecs: vec![
                Symbol::qualified("surface", "lisp"),
                Symbol::qualified("surface", "json"),
            ],
        }
    }
}

fn display_map_from_profile(symbols: &[Symbol]) -> Expr {
    let mut entries = vec![(
        Expr::Symbol(build::keyword("media")),
        build::list(Vec::new()),
    )];
    if let Some(density) = first_named(symbols, &["glance", "compact", "regular", "dense"]) {
        entries.push((
            Expr::Symbol(build::keyword("density")),
            Expr::Symbol(density),
        ));
    }
    if let Some(shape) = first_named(symbols, &["round", "flat"]) {
        entries.push((Expr::Symbol(build::keyword("shape")), Expr::Symbol(shape)));
    }
    if has_symbol(symbols, "stereo") {
        entries.push((Expr::Symbol(build::keyword("stereo")), Expr::Bool(true)));
    }
    if has_symbol(symbols, "hud") {
        entries.push((Expr::Symbol(build::keyword("lines")), build::uint(2)));
    }
    Expr::Map(entries)
}

fn flags_map(symbols: &[Symbol]) -> Expr {
    Expr::Map(
        symbols
            .iter()
            .map(|symbol| {
                (
                    Expr::Symbol(Symbol::new(symbol.name.clone())),
                    Expr::Bool(true),
                )
            })
            .collect(),
    )
}

fn transport_map_from_profile(links: &[Symbol]) -> Expr {
    let kind = links
        .first()
        .cloned()
        .unwrap_or_else(|| Symbol::new("local"));
    build::map(vec![
        ("kind", Expr::Symbol(kind)),
        (
            "offline-queue",
            Expr::Bool(has_symbol(links, "relay") || has_symbol(links, "phone-relay")),
        ),
        ("ordered", Expr::Bool(true)),
        ("round-trip-ms", build::uint(1)),
    ])
}

fn first_named(symbols: &[Symbol], names: &[&str]) -> Option<Symbol> {
    names
        .iter()
        .find_map(|name| symbols.iter().find(|symbol| symbol.name.as_ref() == *name))
        .cloned()
}
