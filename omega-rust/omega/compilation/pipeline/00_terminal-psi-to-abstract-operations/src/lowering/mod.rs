//! Optimizer module role: executable entrance. Terminal-to-abstract lowering entrance: validate the entry roster, lower
//! every verified machine operation by operation, and
//! retain the canonical Terminal-Psi identity.

use abstract_operations::AbstractOperationPlan;
use terminal_codec::terminal_psi_identity;
mod error;
mod machine;

pub use error::LoweringError;

use machine::lower_machine;
use terminal_psi::TerminalModule;

/// Lower a decoded module with its already installed executable entry,
/// retaining canonical identity and all control-flow/operation relationships.
pub(crate) fn lower_decoded_module(
    module: &TerminalModule,
    entry: semantic_vocabulary::MachineId,
) -> Result<AbstractOperationPlan, LoweringError> {
    // Boundary crash contracts ride through `boundary_machines` unchanged.
    // Terminal verification already covered every caller continuation against
    // them; the realized provider body or builtin settlement commits the trap
    // natively, so no abstract operation needs a separate continuation lane.
    if !module.machine(entry).is_some() {
        return Err(LoweringError::VerifiedEntryMachineMissing(entry));
    }
    let functions = module
        .machines
        .iter()
        .map(|machine| lower_machine(module, machine))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(AbstractOperationPlan {
        psi: terminal_psi_identity(module).map_err(LoweringError::SemanticIdentity)?,
        entry,
        structural_types: abstract_operations::StructuralTypeCatalog::new(
            module.structural_types.clone(),
            module.structural_referents.clone(),
        )
        .with_placed_views(module.placed_view_plans.clone())
        .with_placed_view_inputs(module.placed_view_inputs.clone())
        .with_quotients(module.quotients.clone())
        .map_err(LoweringError::SemanticIdentity)?,
        boundary_machines: module.boundary_machines.clone(),
        provider_candidates: module.provider_candidates.clone(),
        dynamic_dispatch: module.dynamic_dispatch.clone(),
        identities: module.identities.clone(),
        functions,
    })
}
