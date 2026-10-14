//! Ordinary state graphs share one callable closure and publication path.
//!
//! `state_graph` admits and emits the graph; each state's operations emit
//! through `emission::emit_call_operations`, which evaluates each call's
//! operands on the state's schedule and sends every operation, calls
//! included, to `operation_frame::OperationFrame`, the emitter the ordinary
//! machine also uses. `admission` collects the boundaries each state's calls
//! name and refuses an operation the graph has no route for; `catalogs` holds
//! the call targets the frame resolves against; `scalar_calls` selects the
//! scalar helpers a standalone catalog prepares.
use super::{CheckedTrees, LoweringError};
mod admission;
pub(super) mod callable;
mod catalogs;
mod emission;
mod literal_arguments;
mod scalar_calls;
mod state_graph;
pub(super) use state_graph::admit as admit_callable;
pub(super) use state_graph::case_emission::result as state_graph_result;
pub(crate) use state_graph::live;

/// Occurrence rows one composed machine publishes beside its Terminal
/// operations. Every selected comparison its states emit keeps the row
/// that joins it to its checked application; none may be dropped between a
/// state's operation buffer and the closure's published roster.
#[derive(Default)]
pub(in crate::unit::attached_unit) struct ComposedOccurrences {
    pub(in crate::unit::attached_unit) claim_introductions:
        Vec<crate::emission::claim_bindings::ClaimIntroduction>,
    pub(in crate::unit::attached_unit) selected_cleanup_groups:
        Vec<crate::emission::selected_continuations::SelectedCleanupGroup>,
    pub(in crate::unit::attached_unit) quotient_adaptations:
        Vec<crate::emission::operation_emission::buffer::SourceQuotientAdaptation>,
    pub(in crate::unit::attached_unit) quotient_calls:
        Vec<crate::emission::operation_emission::buffer::SourceQuotientCall>,
    pub(in crate::unit::attached_unit) source_calls: Vec<lowered_psi::LoweredSourceCallOccurrence>,
    pub(in crate::unit::attached_unit) selected_ieee_float_comparisons:
        Vec<lowered_psi::LoweredSelectedIeeeFloatComparisonOccurrence>,
    pub(in crate::unit::attached_unit) selected_integer_comparisons:
        Vec<lowered_psi::LoweredSelectedIntegerComparisonOccurrence>,
    pub(in crate::unit::attached_unit) source_checked_obligations:
        Vec<semantic_vocabulary::ObligationId>,
    /// The header block of each live state, in plan order.
    pub(in crate::unit::attached_unit) state_blocks: Vec<lowered_psi::LoweredStateBlock>,
}

impl ComposedOccurrences {
    /// Retain every occurrence row one state's completed buffer recorded.
    fn retain(&mut self, operations: crate::emission::operation_emission::buffer::OperationBuffer) {
        self.selected_cleanup_groups
            .extend(operations.selected_cleanup_groups);
        self.claim_introductions
            .extend(operations.claim_introductions);
        self.source_calls.extend(operations.source_calls);
        self.quotient_adaptations
            .extend(operations.quotient_adaptations);
        self.quotient_calls.extend(operations.quotient_calls);
        self.selected_ieee_float_comparisons
            .extend(operations.selected_ieee_float_comparisons);
        self.selected_integer_comparisons
            .extend(operations.selected_integer_comparisons);
        self.source_checked_obligations
            .extend(operations.source_checked_obligations);
    }
}
