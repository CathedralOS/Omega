//! Optimizer module role: executable entrance. Terminal-to-abstract lowering entrance: validate the entry roster, lower
//! every verified machine operation by operation, and
//! retain the canonical Terminal-Psi identity.

mod block_bindings;
mod error;
mod machine;

pub use error::LoweringError;

use crate::shared::*;
use machine::lower_machine;
use terminal_psi::TerminalModule;

/// Consume the complete verified module after the artifact entry has decoded
/// and verified it. The initial terminal vocabulary has one unconditional
/// executable chain per machine, so its Omega requirement stream is flat and
/// ordered.
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
        .map(|machine| {
            lower_machine(
                module,
                machine,
                &module.structural_types,
                &module.dynamic_dispatch,
            )
        })
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
