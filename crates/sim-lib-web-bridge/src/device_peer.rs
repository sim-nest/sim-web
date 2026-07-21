//! Device peers for synchronized surface hubs.

use sim_kernel::Symbol;
use sim_lib_view_device::{DeviceProfile, EdgeId};

use crate::{SurfaceHub, SurfaceRole};

/// Registers a device edge session as a peer surface on `hub`.
///
/// The device profile is converted to ordinary surface capabilities before
/// registration, so committed edits use the same [`SurfaceHub`] broadcast path
/// as phone, desktop, and terminal panes while projecting to the device's own
/// profile.
pub fn register_device_peer(
    hub: &mut SurfaceHub,
    session: &EdgeId,
    profile: &DeviceProfile,
) -> Symbol {
    register_device_peer_with_role(hub, session, profile, SurfaceRole::Peer)
}

/// Registers a device edge session as a surface with an explicit hub role.
pub fn register_device_peer_with_role(
    hub: &mut SurfaceHub,
    session: &EdgeId,
    profile: &DeviceProfile,
    role: SurfaceRole,
) -> Symbol {
    let surface = device_peer_surface(session);
    hub.register_surface_with_role(
        surface.clone(),
        profile.to_surface_caps(session.as_symbol().as_qualified_str()),
        role,
    );
    surface
}

/// Returns the hub surface id used for a device edge session.
pub fn device_peer_surface(session: &EdgeId) -> Symbol {
    Symbol::qualified("device/peer", session.as_symbol().as_qualified_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    use sim_kernel::{Expr, NumberLiteral};
    use sim_lib_intent::{Origin, intent};
    use sim_lib_view::surface;
    use sim_lib_view_device::{ConsentReceipt, DeviceProfile};
    use sim_value::build;

    fn number(value: &str) -> Expr {
        Expr::Number(NumberLiteral {
            domain: build::keyword("i64"),
            canonical: value.to_owned(),
        })
    }

    fn doc() -> Expr {
        Expr::Map(vec![
            (Expr::Symbol(build::keyword("a")), number("1")),
            (Expr::Symbol(build::keyword("b")), number("2")),
            (Expr::Symbol(build::keyword("c")), number("3")),
        ])
    }

    fn edit(field: &str, value: Expr) -> Expr {
        intent(
            "edit-field",
            Origin::human(1),
            vec![
                ("target", doc()),
                (
                    "path",
                    Expr::List(vec![Expr::Vector(vec![
                        Expr::Symbol(build::keyword("k")),
                        Expr::Symbol(build::keyword(field)),
                    ])]),
                ),
                ("value", value),
            ],
        )
    }

    #[test]
    fn device_peer_receives_committed_edits_projected_by_profile() {
        let session = EdgeId::named("watch-route");
        let receipt = ConsentReceipt::new(Vec::new(), 60_000, Vec::new(), session.clone(), 3);
        assert_eq!(receipt.session, session);

        let watch_caps = surface::preset("watch").unwrap();
        let profile = DeviceProfile::from_surface_caps(&watch_caps);
        let mut hub = SurfaceHub::new();
        let device_surface = register_device_peer(&mut hub, &session, &profile);
        hub.register_surface(
            build::keyword("desktop"),
            surface::preset("desktop").unwrap(),
        );
        hub.seed(build::keyword("doc"), doc());

        let device_scene = hub
            .open(
                &device_surface,
                build::keyword("pane"),
                build::keyword("doc"),
            )
            .unwrap();
        let desktop_scene = hub
            .open(
                &build::keyword("desktop"),
                build::keyword("pane"),
                build::keyword("doc"),
            )
            .unwrap();
        assert_ne!(
            device_scene, desktop_scene,
            "device profile projection should differ from a dense desktop peer"
        );

        let broadcasts = hub
            .submit(
                &build::keyword("desktop"),
                &build::keyword("pane"),
                &edit("a", number("9")),
            )
            .unwrap();

        assert!(broadcasts.iter().any(|item| item.surface == device_surface));
        assert!(
            broadcasts
                .iter()
                .any(|item| item.surface == build::keyword("desktop"))
        );

        let broadcasts = hub
            .submit(
                &device_surface,
                &build::keyword("pane"),
                &edit("b", number("8")),
            )
            .unwrap();

        assert!(broadcasts.iter().any(|item| item.surface == device_surface));
        assert!(
            broadcasts
                .iter()
                .any(|item| item.surface == build::keyword("desktop"))
        );
    }
}
