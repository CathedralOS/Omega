//! Host-call authorization check.
//!
//! Ties the boundary provider registry and inferred effect sets to a resolution
//! gate: a boundary call that requests host authority must be backed by an
//! approved provider (chapter 18, "Host Providers"). Ordinary application code
//! that implements its own host-authority boundary trait is minting authority it
//! does not hold, and the call is rejected as an unapproved host call.

use crate::labels::symbol_name;
use omega_checked_trees::CheckFacts;
use omega_core::diagnostics::Diagnostic;
use omega_effects::{audit_host_calls, build_provider_registry};

pub(crate) fn check_host_call_authorization(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let registry = build_provider_registry(program);
    let unapproved = audit_host_calls(program, &facts.effects, &registry);

    if unapproved.is_empty() {
        return Ok(());
    }

    let diagnostics = unapproved
        .into_iter()
        .map(|call| {
            let authority = call
                .missing_authority
                .names()
                .collect::<Vec<_>>()
                .join(", ");
            Diagnostic::error(format!(
                "unapproved host call: {} in {} acquires host authority ({}) with no approved boundary provider",
                symbol_name(program, call.boundary_trait_symbol),
                symbol_name(program, call.state_symbol),
                authority,
            ))
        })
        .collect::<Vec<_>>();

    Err(diagnostics)
}
