//! Viture stereo reprojection as the one bespoke glasses adapter.

use std::rc::Rc;

use sim_kernel::{Error, Expr, Result};
use sim_lib_scene::{Anchor, AnchorSpace, Transform3};
use sim_lib_view_device::{DeviceProfile, EncodedScene, GlassesClass, LocalAdapter, glasses_class};
use sim_value::{access, build};

use crate::PoseView;

/// Stereo reprojector for the Viture-rich glasses path.
///
/// This is the one bespoke [`LocalAdapter`] in the glasses instance because it
/// turns one content-rate `scene/spatial` packet into two device-rate eye views.
/// Halo and other one-card tiers use the shared DEVICE_3 glance adapter instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reprojector {
    /// Maximum prediction lead applied during this adapter step.
    pub max_predict_ms: u64,
}

impl Reprojector {
    /// Builds a reprojector with a prediction clamp.
    pub fn new(max_predict_ms: u64) -> Self {
        Self { max_predict_ms }
    }
}

impl LocalAdapter for Reprojector {
    type State = PoseView;

    fn adapt(
        &self,
        scene: &EncodedScene,
        state: &Self::State,
        profile: &DeviceProfile,
    ) -> Result<Rc<Expr>> {
        if glasses_class(profile) != Some(GlassesClass::Stereo6Dof) {
            return Ok(scene.shared());
        }
        expect_scene_kind(scene.expr(), "spatial")?;
        let left = project_eye(scene.expr(), state, self.max_predict_ms, Eye::Left)?;
        let right = project_eye(scene.expr(), state, self.max_predict_ms, Eye::Right)?;
        Ok(Rc::new(stereo_scene(
            left,
            right,
            state,
            self.max_predict_ms,
        )))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Eye {
    Left,
    Right,
}

impl Eye {
    fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
        }
    }

    fn sign(self) -> f64 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }
}

fn project_eye(scene: &Expr, state: &PoseView, max_predict_ms: u64, eye: Eye) -> Result<Expr> {
    let children = spatial_children(scene)?;
    let projected = children
        .iter()
        .filter_map(|child| project_child(child, state, max_predict_ms, eye).transpose())
        .collect::<Result<Vec<_>>>()?;
    Ok(sim_lib_scene::data_map(vec![
        ("eye", build::sym(eye.name())),
        ("children", build::list(projected)),
    ]))
}

fn project_child(
    child: &Expr,
    state: &PoseView,
    max_predict_ms: u64,
    eye: Eye,
) -> Result<Option<Expr>> {
    if scene_kind(child).as_deref() != Some("panel") {
        return Ok(Some(child.clone()));
    }
    let anchor = Anchor::from_expr(access::required(child, "anchor", "scene/panel")?)?;
    let transform = Transform3::from_expr(access::required(child, "transform", "scene/panel")?)?;
    let projected = project_transform(&anchor, &transform, state, max_predict_ms, eye);
    if !visible_in_frustum(anchor.space, &projected) {
        return Ok(None);
    }
    Ok(Some(projected_panel(child, &anchor, projected, eye)))
}

fn projected_panel(source: &Expr, anchor: &Anchor, transform: Transform3, eye: Eye) -> Expr {
    let id = access::field_str(source, "id").unwrap_or("panel");
    let body = access::field(source, "body").cloned().unwrap_or(Expr::Nil);
    sim_lib_scene::node(
        "panel",
        vec![
            ("id", build::text(format!("{id}:{}", eye.name()))),
            ("source-panel", build::text(id)),
            ("eye", build::sym(eye.name())),
            ("anchor-rule", build::sym(anchor_rule(anchor.space))),
            ("body", body),
            ("anchor", anchor.to_expr()),
            ("transform", transform.to_expr()),
        ],
    )
}

fn project_transform(
    anchor: &Anchor,
    transform: &Transform3,
    state: &PoseView,
    max_predict_ms: u64,
    eye: Eye,
) -> Transform3 {
    let mut out = transform.clone();
    let eye_offset = eye.sign() * state.inter_eye_m / 2.0;
    match anchor.space {
        AnchorSpace::Head => {
            out.translate_m[0] += eye_offset;
        }
        AnchorSpace::World => {
            out.translate_m[0] -= state.translation_m[0];
            out.translate_m[1] -= state.translation_m[1];
            out.translate_m[2] -= state.translation_m[2];
            out.translate_m[0] += state.clamped_yaw_rad(max_predict_ms).sin() * depth(&out);
            out.translate_m[0] += eye_offset;
        }
        AnchorSpace::Screen => {}
        AnchorSpace::Body | AnchorSpace::Device => {
            out.translate_m[0] += eye_offset * 0.5;
        }
    }
    out
}

fn visible_in_frustum(space: AnchorSpace, transform: &Transform3) -> bool {
    if matches!(space, AnchorSpace::Head | AnchorSpace::Screen) {
        return true;
    }
    let z = transform.translate_m[2];
    if z > 0.05 {
        return false;
    }
    let distance = depth(transform);
    let max_x = distance * (52.0_f64.to_radians() / 2.0).tan();
    transform.translate_m[0].abs() <= max_x
}

fn depth(transform: &Transform3) -> f64 {
    transform.translate_m[2].abs().max(1.0)
}

fn anchor_rule(space: AnchorSpace) -> &'static str {
    match space {
        AnchorSpace::Head => "head-locked",
        AnchorSpace::World => "world-locked",
        AnchorSpace::Screen => "screen-locked",
        AnchorSpace::Body => "body-relative",
        AnchorSpace::Device => "device-relative",
    }
}

fn stereo_scene(left: Expr, right: Expr, state: &PoseView, max_predict_ms: u64) -> Expr {
    sim_lib_scene::node(
        "stereo",
        vec![
            ("left-eye", left),
            ("right-eye", right),
            ("sample-seq", build::uint(state.sample_seq)),
            (
                "predict-ms",
                build::uint(state.clamped_predict_ms(max_predict_ms)),
            ),
            ("age-ms", build::uint(state.age_ms)),
        ],
    )
}

fn spatial_children(scene: &Expr) -> Result<&[Expr]> {
    match access::required(scene, "children", "scene/spatial")? {
        Expr::List(children) => Ok(children),
        _ => Err(Error::HostError(
            "scene/spatial children must be a list".to_owned(),
        )),
    }
}

fn expect_scene_kind(scene: &Expr, expected: &str) -> Result<()> {
    match scene_kind(scene).as_deref() {
        Some(kind) if kind == expected => Ok(()),
        _ => Err(Error::HostError(format!("expected scene/{expected}"))),
    }
}

fn scene_kind(expr: &Expr) -> Option<String> {
    let kind = sim_lib_scene::node_kind(expr)?;
    (kind.namespace.as_deref() == Some(sim_lib_scene::SCENE_NAMESPACE))
        .then(|| kind.name.to_string())
}
