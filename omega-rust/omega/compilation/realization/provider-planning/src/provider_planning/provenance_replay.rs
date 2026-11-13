// Row provenance is retained data. Satisfaction derivation and old checked
// adapter replay no longer create provider plans.
use effects::provider_plan::ProviderPlan;
pub use language_semantics::service_surface::ProviderSchemaDeclaration;
pub use requirement_identities::{same_semantic_name, satisfies_plan_name};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderRowRealization {
    Authored(symbols::SymbolHandle),
    BuildNative(std::sync::Arc<crate::native_bindings::NativeBuildRowOrigin>),
    Toolchain,
}

impl ProviderRowRealization {
    pub const fn authored(&self) -> Option<symbols::SymbolHandle> {
        match self {
            Self::Authored(symbol) => Some(*symbol),
            _ => None,
        }
    }
    pub fn native(&self) -> Option<&crate::native_bindings::NativeBuildRowOrigin> {
        match self {
            Self::BuildNative(origin) => Some(origin),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanProvenance {
    pub schema: ProviderSchemaDeclaration,
    pub provider_type: Option<symbols::SymbolHandle>,
    pub row_requirements: Vec<symbols::SymbolHandle>,
    pub row_realizations: Vec<ProviderRowRealization>,
    pub row_target_machine_origins: Vec<Option<SelectedTargetMachineOrigin>>, // Pre-resolution target-scoped declaration custody for catalog-inferred rows; None for ordinary checked, legacy external, and evaluated-payload rows.
}

pub use language_semantics::provider_declarations::SelectedTargetMachineOrigin;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedProviderPlan {
    pub plan: ProviderPlan,
    pub provenance: ProviderPlanProvenance,
}

mod requirement_identities {
    pub use symbols::same_semantic_name;

    pub fn satisfies_plan_name(target: &str, trait_name: &str, provider_type: &str) -> String {
        match (target.is_empty(), provider_type.is_empty()) {
            (true, true) => format!("satisfies::{trait_name}"),
            (false, true) => format!("{target}::satisfies::{trait_name}"),
            (true, false) => format!("{provider_type}::satisfies::{trait_name}"),
            (false, false) => format!("{target}::{provider_type}::satisfies::{trait_name}"),
        }
    }
}
