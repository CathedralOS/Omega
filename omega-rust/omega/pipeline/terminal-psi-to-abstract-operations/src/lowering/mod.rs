//! Optimizer module role: executable entrance. Terminal-to-abstract lowering entrance: validate the entry roster, lower
//! every verified machine operation by operation, and
//! retain the canonical Terminal-Psi identity.

use abstract_operations::AbstractOperationPlan;
use terminal_codec::terminal_psi_identity;
use terminal_verifier::{VerifiedOptimizableTerminalModule, VerifiedTerminalModule};
mod block_bindings;
mod error;
#[path = "machine/lower_machine.rs"]
mod machine;
#[cfg(test)]
mod rejection_audit;

pub use error::LoweringError;

use machine::lower_machine;
use terminal_psi::TerminalModule;

/// Consume the complete module after artifact decoding and independent
/// verification, retaining its explicit control-flow and operation relationships.
pub(crate) fn lower_decoded_verified_module(
    verified: &VerifiedTerminalModule<'_>,
) -> Result<AbstractOperationPlan, LoweringError> {
    lower_decoded_module(verified.module())
}

pub(crate) fn lower_decoded_optimizable_module(
    verified: &VerifiedOptimizableTerminalModule<'_>,
) -> Result<AbstractOperationPlan, LoweringError> {
    lower_decoded_module(verified.module())
}

fn lower_decoded_module(module: &TerminalModule) -> Result<AbstractOperationPlan, LoweringError> {
    // Boundary crash contracts ride through `boundary_machines` unchanged.
    // Terminal verification already covered every caller continuation against
    // them; the realized provider body or builtin settlement commits the trap
    // natively, so no abstract operation needs a separate continuation lane.
    block_bindings::validate_structural_block_bindings(module)?;
    if !module
        .machines
        .iter()
        .any(|machine| machine.id == module.entry)
    {
        return Err(LoweringError::VerifiedEntryMachineMissing(module.entry));
    }
    let functions = module
        .machines
        .iter()
        .map(|machine| lower_machine(module, machine))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AbstractOperationPlan {
        psi: terminal_psi_identity(module).map_err(LoweringError::SemanticIdentity)?,
        entry: module.entry,
        structural_types: module.structural_types.clone().into(),
        boundary_machines: module.boundary_machines.clone(),
        provider_candidates: module.provider_candidates.clone(),
        functions,
    })
}
