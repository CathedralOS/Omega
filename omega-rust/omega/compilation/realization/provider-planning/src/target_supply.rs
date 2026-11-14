//! Settle the provider values the accepted Psi program actually chooses.
//!
//! Source selection and Build-over-default precedence finish before judgment.
//! This owner consumes canonical service schemas, selected declaration values,
//! proved candidate rows and actual native execution inputs. Retained code and
//! schemas are not invocation roots; the caller supplies the reached boundaries.

mod plan_realization;
mod realized_supply;

pub use plan_realization::{
    TargetSupplyExecutionInputs, TargetSupplyRealizationRequest, realize_target_supply,
};
pub use realized_supply::{
    ProviderPlanRealizationInput, RealizedTargetSupply, TargetSupplyRealizationInput,
};

mod source_choices;
pub use source_choices::{
    DemandedRequirement, JudgedProvider, PreparedTargetSupply, ProviderSupply,
    ReachedBoundaryRequirement, SettledRequirement, SettledTargetSupply, SupplyAuthority,
    TargetSupplyRequest,
};
