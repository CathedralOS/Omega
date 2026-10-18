//! Seal the runtime-value spill and bind it to its selected source.
//!
//! Admission owns the definition, dominance, transport and last-writer
//! derivation. This seal publishes the producer's exact transformation; it
//! does not independently reconstruct admission. Receiving physical stages
//! retain their ordinary input and custody checks.
use selected_instructions::SelectedInstructionPlan;

use super::{RuntimeSpillError, RuntimeSpillReceipt, ValidatedRuntimeSpill};
use crate::ValidatedSelectedAnalysis;
use crate::rewrites::sealed;
/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_runtime_spill_with_span_policy(
    source: &impl ValidatedSelectedAnalysis,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSpill, RuntimeSpillError> {
    Ok(sealed!(
        ValidatedRuntimeSpill,
        RuntimeSpillReceipt,
        source,
        proposed
    ))
}
