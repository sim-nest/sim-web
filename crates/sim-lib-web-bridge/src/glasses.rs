//! Browser/native clients for glasses Scene adaptation.

use std::rc::Rc;

use sim_kernel::{Error, Expr, Result};
use sim_lib_scene::GlanceCard;
use sim_lib_view::SurfaceCaps;
use sim_lib_view_device::{
    DeviceProfile, DeviceSurfaceCapsExt, EncodedScene, GlanceState, GlassesClass, LocalAdapter,
    glasses_class,
};
use sim_lib_view_spatial::{ClampedReprojector, PoseView, halo_glance_config};
use sim_value::{access, build};

/// Side-by-side viewport dimensions advertised by glasses surface capabilities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GlassesViewport {
    per_eye_px: [u32; 2],
}

impl GlassesViewport {
    /// Reads per-eye dimensions from a glasses `SurfaceCaps` display map.
    ///
    /// # Errors
    ///
    /// Returns an error when `per-eye-px` is absent or malformed.
    pub fn from_caps(caps: &SurfaceCaps) -> Result<Self> {
        let values = match access::required(&caps.display, "per-eye-px", "glasses display caps")? {
            Expr::List(values) if values.len() == 2 => values,
            _ => {
                return Err(Error::HostError(
                    "glasses display per-eye-px must contain width and height".to_owned(),
                ));
            }
        };
        Ok(Self {
            per_eye_px: [read_px(&values[0])?, read_px(&values[1])?],
        })
    }

    /// Returns `[width, height]` for one eye.
    pub fn per_eye_px(self) -> [u32; 2] {
        self.per_eye_px
    }

    /// Returns `[width, height]` for the side-by-side frame.
    pub fn frame_px(self) -> [u32; 2] {
        [self.per_eye_px[0].saturating_mul(2), self.per_eye_px[1]]
    }
}

/// Native Viture client that retains one content Scene across device-rate frames.
///
/// Rich profiles reuse the shared clamp-aware spatial reprojector. Display-only
/// profiles return the retained `scene/spatial` packet unchanged for mirroring.
#[derive(Debug)]
pub struct VitureSceneClient {
    profile: DeviceProfile,
    viewport: GlassesViewport,
    reprojector: ClampedReprojector,
    scene: Option<EncodedScene>,
    content_receipts: u64,
}

impl VitureSceneClient {
    /// Builds a client from open surface capabilities.
    ///
    /// # Errors
    ///
    /// Returns an error unless the caps describe stereo 6DoF or display-only
    /// glasses with valid per-eye dimensions.
    pub fn new(caps: &SurfaceCaps, max_predict_ms: u64) -> Result<Self> {
        let profile = caps.device_profile();
        match glasses_class(&profile) {
            Some(GlassesClass::Stereo6Dof | GlassesClass::DisplayOnly) => {}
            _ => {
                return Err(Error::HostError(
                    "Viture client requires stereo or display-only glasses caps".to_owned(),
                ));
            }
        }
        Ok(Self {
            profile,
            viewport: GlassesViewport::from_caps(caps)?,
            reprojector: ClampedReprojector::new(max_predict_ms),
            scene: None,
            content_receipts: 0,
        })
    }

    /// Retains a new content-rate `scene/spatial` packet.
    ///
    /// # Errors
    ///
    /// Returns an error when `scene` is not a spatial Scene root.
    pub fn receive(&mut self, scene: Expr) -> Result<()> {
        expect_scene_kind(&scene, "spatial")?;
        self.scene = Some(EncodedScene::new(scene));
        self.content_receipts = self.content_receipts.saturating_add(1);
        Ok(())
    }

    /// Adapts the retained Scene for one local pose sample.
    ///
    /// # Errors
    ///
    /// Returns an error before the first content receipt or when reprojection
    /// rejects malformed spatial content.
    pub fn frame(&self, pose: &PoseView) -> Result<Rc<Expr>> {
        let scene = self
            .scene
            .as_ref()
            .ok_or_else(|| Error::HostError("Viture client has no content Scene".to_owned()))?;
        let frame = self.reprojector.adapt(scene, pose, &self.profile)?;
        if glasses_class(&self.profile) == Some(GlassesClass::DisplayOnly) {
            return Ok(frame);
        }
        Ok(Rc::new(with_stereo_viewport(frame.as_ref(), self.viewport)))
    }

    /// Returns the number of content-rate Scene packets received.
    pub fn content_receipts(&self) -> u64 {
        self.content_receipts
    }

    /// Returns the side-by-side viewport dimensions.
    pub fn viewport(&self) -> GlassesViewport {
        self.viewport
    }

    /// Returns whether this client is using display-only mirroring.
    pub fn is_mirror(&self) -> bool {
        glasses_class(&self.profile) == Some(GlassesClass::DisplayOnly)
    }
}

/// Native Halo preview client over the shared one-card glance adapter.
#[derive(Debug)]
pub struct HaloPreviewClient {
    profile: DeviceProfile,
    scene: Option<EncodedScene>,
    content_receipts: u64,
}

impl HaloPreviewClient {
    /// Builds a preview client from mono-HUD surface capabilities.
    ///
    /// # Errors
    ///
    /// Returns an error unless the caps resolve to mono-HUD glasses.
    pub fn new(caps: &SurfaceCaps) -> Result<Self> {
        let profile = caps.device_profile();
        if glasses_class(&profile) != Some(GlassesClass::MonoHud) {
            return Err(Error::HostError(
                "Halo preview requires mono-HUD glasses caps".to_owned(),
            ));
        }
        Ok(Self {
            profile,
            scene: None,
            content_receipts: 0,
        })
    }

    /// Retains a new content-rate `scene/glance` card.
    ///
    /// # Errors
    ///
    /// Returns an error when the Scene is not one valid glance card.
    pub fn receive(&mut self, scene: Expr) -> Result<()> {
        GlanceCard::from_scene(&scene)?;
        self.scene = Some(EncodedScene::new(scene));
        self.content_receipts = self.content_receipts.saturating_add(1);
        Ok(())
    }

    /// Fits the retained card to the Halo budget for one local input state.
    ///
    /// # Errors
    ///
    /// Returns an error before the first content receipt or when the shared
    /// glance adapter rejects the card.
    pub fn frame(&self, state: &GlanceState) -> Result<Rc<Expr>> {
        let scene = self
            .scene
            .as_ref()
            .ok_or_else(|| Error::HostError("Halo preview has no glance Scene".to_owned()))?;
        halo_glance_config().adapt(scene, state, &self.profile)
    }

    /// Returns the number of content-rate cards received.
    pub fn content_receipts(&self) -> u64 {
        self.content_receipts
    }
}

fn read_px(expr: &Expr) -> Result<u32> {
    let Expr::Number(number) = expr else {
        return Err(Error::HostError(
            "glasses viewport dimensions must be numbers".to_owned(),
        ));
    };
    number
        .canonical
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::HostError("glasses viewport dimensions must be positive".to_owned()))
}

fn with_stereo_viewport(scene: &Expr, viewport: GlassesViewport) -> Expr {
    let per_eye = viewport.per_eye_px();
    let frame = viewport.frame_px();
    let scene = access::set(scene, "layout", build::sym("side-by-side"));
    let scene = access::set(
        &scene,
        "eye-px",
        build::list(vec![
            build::uint(per_eye[0].into()),
            build::uint(per_eye[1].into()),
        ]),
    );
    access::set(
        &scene,
        "frame-px",
        build::list(vec![
            build::uint(frame[0].into()),
            build::uint(frame[1].into()),
        ]),
    )
}

fn expect_scene_kind(scene: &Expr, expected: &str) -> Result<()> {
    let matches = sim_lib_scene::node_kind(scene).is_some_and(|kind| {
        kind.namespace.as_deref() == Some(sim_lib_scene::SCENE_NAMESPACE)
            && kind.name.as_ref() == expected
    });
    if matches {
        Ok(())
    } else {
        Err(Error::HostError(format!("expected scene/{expected}")))
    }
}
