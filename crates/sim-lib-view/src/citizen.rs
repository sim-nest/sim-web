use sim_citizen_derive::Citizen;
use sim_kernel::{CapabilityName, Symbol};

/// Registration record for a view lens, exposed as a runtime Citizen.
#[derive(Clone, Debug, PartialEq, Citizen)]
#[citizen(symbol = "view/LensDescriptor", version = 1)]
pub struct ViewLensDescriptor {
    /// Stable lens id.
    pub id: Symbol,
    /// Lens kind tag.
    pub kind: Symbol,
    /// Value classes this lens claims to render.
    pub claimed_classes: Vec<Symbol>,
    /// Capabilities the lens requires to run.
    pub required_capabilities: Vec<CapabilityName>,
    /// Whether this lens is the universal default fallback.
    pub universal_default: bool,
}

impl Default for ViewLensDescriptor {
    fn default() -> Self {
        Self {
            id: Symbol::new(crate::UNIVERSAL_DEFAULT_LENS),
            kind: Symbol::new("view"),
            claimed_classes: Vec::new(),
            required_capabilities: Vec::new(),
            universal_default: true,
        }
    }
}

/// Returns the class symbol for the view lens descriptor Citizen.
pub fn view_lens_descriptor_class_symbol() -> Symbol {
    Symbol::qualified("view", "LensDescriptor")
}
