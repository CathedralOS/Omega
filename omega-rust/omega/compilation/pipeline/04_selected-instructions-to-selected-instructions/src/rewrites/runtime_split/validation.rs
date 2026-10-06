//! Independent validation of one runtime live-range split.
//!
//! The validator never calls [`super::admission`]: it re-derives the split's
//! legality from the source records — the named register's single pinned
//! definition, its scalar and definition-site custody, the target's copy
//! row, the reference scan over instruction operands, terminator operands
//! and outgoing transports, and the dominance the definition block must hold
//! over every use block — then builds the one transformed plan that audit
//! permits and requires the proposal to equal it by content. Equality covers
//! stripping: removing the copy and the fresh register and rebinding the
//! references back restores the complete source, because every other record
//! of the expected plan is the source verbatim. A producer admission error
//! therefore fails here even when the proposal is exactly what that producer
//! emitted.

use std::sync::Arc;

use selected_instructions::SelectedInstructionPlan;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{RuntimeSplitError, RuntimeSplitReceipt, ValidatedRuntimeSplit};
use crate::ValidatedSelectedAnalysis;

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_runtime_split(
    source: &impl ValidatedSelectedAnalysis,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeSplit, RuntimeSplitError> {
    Ok(ValidatedRuntimeSplit {
        receipt: RuntimeSplitReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
