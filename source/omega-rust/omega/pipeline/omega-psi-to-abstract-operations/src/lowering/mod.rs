//! Optimizer module role: executable entrance. Terminal-to-abstract lowering entrance: validate the entry roster, lower
//! every verified machine through the ordinary or structural family, and
//! retain the canonical Terminal-Psi identity.

mod error;
mod machine;
mod payloadless;
mod structural;

pub use error::LoweringError;

use crate::shared::*;
use machine::lower_machine;
use psi_terminal::TerminalModule;
use psi_terminal_verifier::VerifiedNativeRankedTerminalModule;

/// Consume the complete verified module after the artifact entry has decoded
/// and verified it. The initial terminal vocabulary has one unconditional
/// executable chain per machine, so its Omega requirement stream is flat and
/// ordered.
pub(crate) fn lower_decoded_verified_module(
    verified: &VerifiedTerminalModule<'_>,
    retain_payloadless_for_optimization: bool,
) -> Result<AbstractOperationPlan, LoweringError> {
    lower_decoded_module(verified.module(), retain_payloadless_for_optimization)
}

pub(crate) fn lower_decoded_native_ranked_module(
    verified: &VerifiedNativeRankedTerminalModule<'_>,
) -> Result<AbstractOperationPlan, LoweringError> {
    lower_decoded_module(verified.module(), false)
}

fn lower_decoded_module(
    module: &TerminalModule,
    retain_payloadless_for_optimization: bool,
) -> Result<AbstractOperationPlan, LoweringError> {
    if !module
        .machines
        .iter()
        .any(|machine| machine.id == module.entry)
    {
        return Err(LoweringError::VerifiedEntryMachineMissing(module.entry));
    }
    let machines = module
        .machines
        .iter()
        .map(|machine| (machine.id, machine))
        .collect::<BTreeMap<_, _>>();
    let functions = module
        .machines
        .iter()
        .map(|machine| {
            lower_machine(
                module,
                machine,
                &machines,
                &module.structural_types,
                retain_payloadless_for_optimization,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AbstractOperationPlan {
        psi: terminal_psi_identity(module).map_err(LoweringError::SemanticIdentity)?,
        entry: module.entry,
        structural_types: module.structural_types.clone(),
        boundary_machines: module.boundary_machines.clone(),
        provider_candidates: module.provider_candidates.clone(),
        functions,
    })
}
