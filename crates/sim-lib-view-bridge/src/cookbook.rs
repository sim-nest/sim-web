//! Cookbook fixtures for the BRIDGE packet review surface.

use sim_codec_bridge::{
    BridgeHeader, BridgePacket, BridgePart, BridgeProvenance, BridgeReceiptPayload,
    stamp_packet_cid,
};
use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Expr, Symbol};
use sim_lib_view::surface::SurfaceCaps;
use std::sync::Arc;

/// Builds a packet review scene showing drafter, reviewer, and judge seats.
pub fn packet_review_demo() -> Expr {
    let mut cx = Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory));
    let packet = sample_packet();
    let caps = SurfaceCaps::from_preset("desktop", "cookbook").expect("desktop caps exist");
    crate::bridge_packet_view(&mut cx, &packet, &caps).expect("demo packet renders")
}

fn sample_packet() -> BridgePacket {
    stamp_packet_cid(&BridgePacket {
        header: BridgeHeader {
            cid: None,
            move_kind: Symbol::new("receipt"),
            from: "model:judge".to_owned(),
            to: vec!["human:reviewer".to_owned(), "model:drafter".to_owned()],
            role: Symbol::new("judge"),
            parents: vec!["core/sha256-bridge-v1:merged#move=reply".to_owned()],
            task: Symbol::new("Rc1"),
            output: Symbol::new("Rc1"),
            ceiling: Vec::new(),
            context: Vec::new(),
            provenance: BridgeProvenance::default(),
        },
        body: vec![BridgePart {
            id: Symbol::new("Rc1"),
            kind: Symbol::qualified("bridge", "Receipt"),
            payload: BridgeReceiptPayload::new(
                Symbol::new("accepted"),
                vec!["body/O2/payload".to_owned()],
            )
            .to_expr(),
        }],
        warrant: None,
    })
    .expect("sample packet stamps")
}

#[cfg(test)]
mod tests {
    #[test]
    fn packet_review_demo_is_a_scene() {
        let scene = super::packet_review_demo();
        sim_lib_scene::validate_scene(&scene).expect("demo scene validates");
    }
}
