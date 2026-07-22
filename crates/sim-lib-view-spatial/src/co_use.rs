//! Dual-glasses co-use roles and profile validation.

use sim_kernel::{Error, Result};
use sim_lib_view::SurfaceCaps;
use sim_lib_view_device::{
    ConsentReceipt, DeviceProfile, DeviceSurfaceCapsExt, EdgeId, GlassesClass, StalePolicy,
    glasses_class,
};

/// A glasses peer inside one worn co-use session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassesPeer {
    /// The Viture Luma Ultra focus surface.
    Viture,
    /// The Brilliant Labs Halo ambient peer.
    Halo,
}

impl GlassesPeer {
    /// Returns this peer's role in the shared session.
    pub fn role(self) -> GlassesCoUseRole {
        match self {
            Self::Viture => GlassesCoUseRole::Main,
            Self::Halo => GlassesCoUseRole::Peer,
        }
    }

    /// Returns the expected glasses class for this peer.
    pub fn expected_class(self) -> GlassesClass {
        match self {
            Self::Viture => GlassesClass::Stereo6Dof,
            Self::Halo => GlassesClass::MonoHud,
        }
    }

    /// Returns the peer's stable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Viture => "viture",
            Self::Halo => "halo",
        }
    }
}

/// Role of a glasses peer inside one co-use session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GlassesCoUseRole {
    /// Primary focus surface for the shared session.
    Main,
    /// Secondary peer surface for the same canonical session.
    Peer,
}

/// Validated peer profile and adapter-loop policy.
#[derive(Clone, Debug, PartialEq)]
pub struct GlassesPeerConfig {
    /// Which glasses peer this config describes.
    pub peer: GlassesPeer,
    /// Role assigned to the peer.
    pub role: GlassesCoUseRole,
    /// The validated device profile.
    pub profile: DeviceProfile,
    /// Stale policy used by the peer's local adapter loop.
    pub policy: StalePolicy,
}

/// A co-use plan bound to one session and consent receipt.
#[derive(Clone, Debug, PartialEq)]
pub struct GlassesCoUsePlan {
    session: EdgeId,
    consent: ConsentReceipt,
    peers: Vec<GlassesPeerConfig>,
}

impl GlassesCoUsePlan {
    /// Builds an empty plan bound to `session` and its consent receipt.
    pub fn new(session: EdgeId, consent: ConsentReceipt) -> Result<Self> {
        if consent.session != session {
            return Err(Error::HostError(
                "glasses co-use consent receipt is bound to a different session".to_owned(),
            ));
        }
        Ok(Self {
            session,
            consent,
            peers: Vec::new(),
        })
    }

    /// Attach or replace one peer profile in the plan.
    pub fn attach_caps(
        &mut self,
        peer: GlassesPeer,
        caps: &SurfaceCaps,
    ) -> Result<GlassesPeerConfig> {
        let config = glasses_peer_config(peer, caps)?;
        self.peers.retain(|existing| existing.peer != peer);
        self.peers.push(config.clone());
        Ok(config)
    }

    /// Detach one peer from the plan.
    pub fn detach(&mut self, peer: GlassesPeer) -> Option<GlassesPeerConfig> {
        let index = self
            .peers
            .iter()
            .position(|existing| existing.peer == peer)?;
        Some(self.peers.remove(index))
    }

    /// Returns true while any peer still holds the session.
    pub fn is_alive(&self) -> bool {
        !self.peers.is_empty()
    }

    /// Returns the shared session id.
    pub fn session(&self) -> &EdgeId {
        &self.session
    }

    /// Returns the session-bound consent receipt.
    pub fn consent(&self) -> &ConsentReceipt {
        &self.consent
    }

    /// Returns the attached peer configs.
    pub fn peers(&self) -> &[GlassesPeerConfig] {
        &self.peers
    }
}

/// Validates caps for a specific glasses peer and returns its config.
pub fn glasses_peer_config(peer: GlassesPeer, caps: &SurfaceCaps) -> Result<GlassesPeerConfig> {
    let profile = caps.device_profile();
    let class = glasses_class(&profile).ok_or_else(|| {
        Error::HostError(format!("{} caps do not describe glasses", peer.label()))
    })?;
    if class != peer.expected_class() {
        return Err(Error::HostError(format!(
            "{} caps resolve to {class:?}, expected {:?}",
            peer.label(),
            peer.expected_class()
        )));
    }
    Ok(GlassesPeerConfig {
        peer,
        role: peer.role(),
        profile,
        policy: peer_policy(peer),
    })
}

fn peer_policy(peer: GlassesPeer) -> StalePolicy {
    match peer {
        GlassesPeer::Viture => StalePolicy::Predict,
        GlassesPeer::Halo => StalePolicy::HoldLast,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use sim_lib_view_device::DeviceCapability;

    use crate::{halo_loop, viture_loop};

    #[test]
    fn viture_and_halo_share_one_session() {
        let edge = EdgeId::named("wear-session");
        let consent = ConsentReceipt::new(
            vec![
                DeviceCapability::Pose.grant_symbol(),
                DeviceCapability::Mic.grant_symbol(),
            ],
            60_000,
            Vec::new(),
            edge.clone(),
            9,
        );
        let mut plan = GlassesCoUsePlan::new(edge.clone(), consent.clone()).unwrap();

        let viture_caps = SurfaceCaps::from_preset("glasses-luma-ultra", "viture.co-use").unwrap();
        let halo_caps = SurfaceCaps::from_preset("glasses-hud", "halo.co-use").unwrap();
        let viture = plan.attach_caps(GlassesPeer::Viture, &viture_caps).unwrap();
        let halo = plan.attach_caps(GlassesPeer::Halo, &halo_caps).unwrap();

        assert_eq!(plan.session(), &edge);
        assert_eq!(plan.consent(), &consent);
        assert_eq!(plan.peers().len(), 2);
        assert_eq!(viture.role, GlassesCoUseRole::Main);
        assert_eq!(halo.role, GlassesCoUseRole::Peer);
        assert_eq!(viture.policy, StalePolicy::Predict);
        assert_eq!(halo.policy, StalePolicy::HoldLast);
        assert_eq!(viture_loop(&viture.profile, 12).0.policy(), viture.policy);
        assert_eq!(halo_loop(&halo.profile).0.policy(), halo.policy);

        assert!(plan.detach(GlassesPeer::Viture).is_some());
        assert!(plan.is_alive(), "Halo keeps the shared session alive");
        assert_eq!(plan.peers()[0].peer, GlassesPeer::Halo);
        assert_eq!(plan.session(), &edge);
        assert_eq!(plan.consent(), &consent);
    }
}
