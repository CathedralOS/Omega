//! Optimizer module role: executable entrance. Validated semantic ProgramStorage entry contract.
//!
//! This entrance owns the only transition from selected source and target
//! declarations into an address-free semantic entry contract: first validate
//! every retained boundary, then construct the immutable contract.

mod construction;
mod model;
mod validation;

#[cfg(test)]
mod tests;

pub use model::*;

use crate::{
    ProgramStorageEntryDiagnostic, SelectedProgramEntrySourceSignature,
    SelectedProgramStorageEntryPlan,
};

/// Bind the clean optimizer's declaration-only semantic ProgramStorage edge.
///
/// `semantic_application` is the exact calling-plan application custody the
/// native binder replayed for this entry: the schema commits to that
/// application identity, not to the raw ABI plan, so the two must arrive
/// together.
pub fn bind_optimized_program_storage_semantic_entry_contract(
    target: target::NativeTarget,
    selected: &SelectedProgramStorageEntryPlan,
    source: &SelectedProgramEntrySourceSignature,
    semantic_application: &OptimizedProgramStorageSemanticCallingApplication<'_>,
) -> Result<OptimizedProgramStorageSemanticEntryContract, ProgramStorageEntryDiagnostic> {
    let validated = validation::validate(target, selected, source, semantic_application)?;
    Ok(construction::construct(validated))
}
