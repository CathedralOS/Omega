//! Independent validation of runtime-value rematerialization.
//!
//! The validator never calls [`super::admission`]: it re-derives the
//! regeneration's legality from the source records — the named register's
//! roster row and single `MaterializeI64` definition against the target's
//! materialize constraint, the scalar payload and definition-site custody
//! the regenerable family admits, the use scan over instruction operands,
//! terminator operands and outgoing edge transports, and the dominance the
//! definition block must hold over every use block — then requires the
//! proposal to carry exactly the fresh registers, repeated
//! materializations and rebound operands the audit derives, with the
//! boundary settlements remapped over the insertions. Stripping those
//! additions must restore the complete source by content. A producer
//! admission error therefore fails validation even when the proposal is
//! exactly what that producer emitted.

use std::sync::Arc;

use selected_instructions::SelectedInstructionPlan;
use target_operations_to_selected_instructions::selected_instruction_plan_identity;

use super::{
    RuntimeRematerializationError, RuntimeRematerializationReceipt,
    ValidatedRuntimeRematerialization,
};
use crate::ValidatedSelectedAnalysis;

/// Admit a freshly produced plan with its receipt; the producer calls this
/// instead of replaying its own output.
pub(crate) fn seal_runtime_rematerialization(
    source: &impl ValidatedSelectedAnalysis,
    proposed: SelectedInstructionPlan,
) -> Result<ValidatedRuntimeRematerialization, RuntimeRematerializationError> {
    Ok(ValidatedRuntimeRematerialization {
        receipt: RuntimeRematerializationReceipt {
            source_selected: source.selected_identity(),
            transformed_selected: selected_instruction_plan_identity(&proposed),
            optimization_unit: source.optimization_unit_identity(),
            fuel_schedule: source.fuel_schedule_identity(),
        },
        transformed: Arc::new(proposed),
    })
}
