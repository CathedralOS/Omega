//! Boundary-provider approval check.
//!
//! Every boundary capability call must resolve through the approved provider
//! edge for that exact capability. Ordinary application code that implements a
//! whole boundary trait is minting authority it does not hold and rejects.

use crate::labels::symbol_name;
use omega_checked_trees::CheckFacts;
use omega_core::diagnostics::Diagnostic;
use omega_effects::{audit_boundary_provider_calls, build_boundary_provider_approval_registry};

pub(crate) fn check_boundary_provider_approval(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let registry = build_boundary_provider_approval_registry(program);
    let unapproved = audit_boundary_provider_calls(program, &facts.operations, &registry);

    if unapproved.is_empty() {
        return Ok(());
    }

    let diagnostics = unapproved
        .into_iter()
        .map(|call| {
            Diagnostic::error(format!(
                "unapproved boundary call: {} in {} exercises a boundary capability with no approved provider for that exact capability",
                symbol_name(program, call.boundary_trait_symbol),
                symbol_name(program, call.state_symbol),
            ))
        })
        .collect::<Vec<_>>();

    Err(diagnostics)
}
