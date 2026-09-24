//! Ordinary state graphs share one callable closure and publication path.
//!
//! `state_graph` admits and emits the graph; each state's operations emit
//! through `emission::emit_call_operations`, which evaluates each call's
//! operands on the state's schedule and sends every operation, calls
//! included, to `operation_frame::OperationFrame`, the emitter the ordinary
//! machine also uses. `admission` and `internal_calls::admission` decide which
//! call shapes a state admits; `catalogs` holds the call targets the frame
//! resolves against; `scalar_calls` selects the scalar helpers a standalone
//! catalog prepares.
use super::{CheckedTrees, LoweringError};
mod admission;
pub(super) mod callable;
mod catalogs;
pub(super) mod dynamic_result;
mod emission;
mod internal_calls;
mod literal_arguments;
mod scalar_calls;
mod state_graph;
pub(crate) use crate::producer_result::SourceMappedLowered;
pub(super) use callable::admit as admit_callable;
pub(crate) use catalogs::ComposedCatalogs;
pub(super) use state_graph::case_emission::result as state_graph_result;
pub(crate) use state_graph::live;

/// Occurrence rows one composed machine publishes beside its Terminal
/// operations. Every selected comparison or FMA its states emit keeps the row
/// that joins it to its checked application; none may be dropped between a
/// state's operation buffer and the closure's published roster.
#[derive(Default)]
pub(in crate::unit::attached_unit) struct ComposedOccurrences {
    pub(in crate::unit::attached_unit) source_calls: Vec<lowered_psi::LoweredSourceCallOccurrence>,
    pub(in crate::unit::attached_unit) selected_ieee_float_fmas:
        Vec<lowered_psi::LoweredSelectedIeeeFloatFmaOccurrence>,
    pub(in crate::unit::attached_unit) selected_ieee_float_comparisons:
        Vec<lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(in crate::unit::attached_unit) selected_integer_comparisons:
        Vec<lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
}

impl ComposedOccurrences {
    /// Retain every occurrence row one state's completed buffer recorded.
    fn retain(&mut self, operations: crate::emission::operation_emission::buffer::OperationBuffer) {
        self.source_calls.extend(operations.source_calls);
        self.selected_ieee_float_fmas
            .extend(operations.selected_ieee_float_fmas);
        self.selected_ieee_float_comparisons
            .extend(operations.selected_ieee_float_comparisons);
        self.selected_integer_comparisons
            .extend(operations.selected_integer_comparisons);
    }
}

pub(crate) fn lower_composed_unit_control_machine(
    checked: &CheckedTrees,
    plan: &checked_trees::CheckedComposedUnitControlMachinePlan,
) -> Result<SourceMappedLowered, LoweringError> {
    if !state_graph::has_shared_graph_custody(checked, plan) {
        return super::unsupported("composed Unit control has unsupported graph custody");
    }
    let mut lowered = super::lower_unit_effect_closure(checked, plan.machine)?;
    super::finalize_operation_proofs(&mut lowered.terminal)?;
    Ok(lowered)
}
