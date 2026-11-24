//! Settle the provider values the accepted Psi program actually chooses.
//!
//! Source selection and Build-over-default precedence finish before judgment.
//! This owner consumes canonical service schemas, selected declaration values,
//! proved candidate rows and actual native execution inputs. Retained code and
//! schemas are not invocation roots; the caller supplies the reached boundaries.

mod plan_realization;
mod realized_supply;

pub use plan_realization::{
    PublishedComponentSuppliers, TargetSupplyExecutionInputs, TargetSupplyRealizationError,
    TargetSupplyRealizationRequest, realize_target_supply,
};
pub use realized_supply::{
    ProviderPlanRealizationInput, RealizedTargetSupply, SelectedProviderRealization,
    TargetSupplyRealizationInput,
};

mod installation_reach;
pub use installation_reach::{NormalizedNativeSupplies, NormalizedNativeSupply};

fn matches_direct_leaf(
    identities: &terminal_psi::TerminalIdentityTable,
    kind: semantic_vocabulary::BoundaryRequirementKind,
    declaration: &terminal_psi::CheckedBoundaryCallable,
    leaf: &direct_boundary_bindings::DirectBoundaryLeaf,
) -> bool {
    let declaration_identity = if kind == semantic_vocabulary::BoundaryRequirementKind::Trait
        || declaration.execution != terminal_psi::CheckedBoundaryExecution::Provider
    {
        declaration.source.semantic_declaration
    } else {
        declaration.source.machine.identity()
    };
    identities.text(declaration_identity) == Some(leaf.declaration_identity.as_str())
        && identities.text(declaration.source.callable) == Some(leaf.callable_identity.as_str())
}
