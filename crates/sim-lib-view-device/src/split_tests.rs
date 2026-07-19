use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use sim_kernel::{Cx, DefaultFactory, EagerPolicy, Error, Expr, Result};
use sim_lib_view::{Draft, Operation, SurfaceCaps, SurfaceCodec};
use sim_value::build;

use crate::{EncodedScene, LocalAdapter, MirrorAdapter, Split, device_profile_demo, tier_preset};

#[test]
fn split_encodes_once_adapts_many() {
    let encode_count = Arc::new(AtomicUsize::new(0));
    let codec = CountingEncoder {
        encode_count: Arc::clone(&encode_count),
    };
    let profile = tier_preset(crate::DeviceTier::Actuator);
    let split = Split::new(StateTagAdapter, profile);
    let caps = SurfaceCaps::from_preset("watch", "watch.local.split").expect("watch caps");
    let mut cx = test_cx();
    let states = [1_u64, 2, 3, 4];

    let run = split
        .run(
            &codec,
            &mut cx,
            &Expr::String("value".to_owned()),
            &caps,
            &states,
        )
        .expect("split run");

    assert_eq!(encode_count.load(Ordering::SeqCst), 1);
    assert_eq!(run.frames.len(), states.len());
    assert_eq!(run.encoded.expr(), &encoded_scene());
    for (frame, state) in run.frames.iter().zip(states) {
        assert_eq!(frame.as_ref(), &adapted_scene(state));
    }
}

#[test]
fn mirror_adapter_reuses_shared_scene_without_deep_clone() {
    let profile = tier_preset(crate::DeviceTier::Display);
    let encoded = EncodedScene::new(device_profile_demo());
    let split = Split::new(MirrorAdapter, profile);
    let states = [(), (), ()];

    let frames = split.adapt_many(&encoded, &states).expect("mirror adapts");

    assert_eq!(frames.len(), states.len());
    for frame in frames {
        assert!(Rc::ptr_eq(&frame, &encoded.shared()));
    }
}

#[derive(Clone)]
struct CountingEncoder {
    encode_count: Arc<AtomicUsize>,
}

impl SurfaceCodec for CountingEncoder {
    fn encode(&self, _cx: &mut Cx, _value: &Expr, _caps: &SurfaceCaps) -> Result<Expr> {
        self.encode_count.fetch_add(1, Ordering::SeqCst);
        Ok(encoded_scene())
    }

    fn decode(&self, _cx: &mut Cx, _value: &Expr, _intent: &Expr) -> Result<Draft> {
        Err(Error::HostError(
            "counting encoder only supports encode".to_owned(),
        ))
    }

    fn commit(&self, _cx: &mut Cx, _draft: &Draft) -> Result<Operation> {
        Err(Error::HostError(
            "counting encoder only supports encode".to_owned(),
        ))
    }
}

#[derive(Clone, Copy, Debug)]
struct StateTagAdapter;

impl LocalAdapter for StateTagAdapter {
    type State = u64;

    fn adapt(
        &self,
        scene: &EncodedScene,
        state: &Self::State,
        _profile: &crate::DeviceProfile,
    ) -> Result<Rc<Expr>> {
        Ok(Rc::new(build::map(vec![
            ("kind", build::qsym("device", "adapted")),
            ("scene", scene.expr().clone()),
            ("state", build::uint(*state)),
        ])))
    }
}

fn encoded_scene() -> Expr {
    build::map(vec![
        ("kind", build::qsym("scene", "text")),
        ("text", build::text("encoded")),
    ])
}

fn adapted_scene(state: u64) -> Expr {
    build::map(vec![
        ("kind", build::qsym("device", "adapted")),
        ("scene", encoded_scene()),
        ("state", build::uint(state)),
    ])
}

fn test_cx() -> Cx {
    Cx::new(Arc::new(EagerPolicy), Arc::new(DefaultFactory))
}
