//! The selected provider plan binding and its program updates.

use crate::trust_model::ResolvedAuthoredSelectedProviderGrant;
use std::sync::Arc;

/// Exact checked-program and selected-plan candidate after every provider
/// grant, receipt, operator-use, and installation-reach decision has replayed.
/// The candidate owns any separated Arc privately until its caller commits it.
#[derive(Debug)]
pub struct SelectedProviderPlanBinding {
    pub(crate) program: Arc<typed_trees_to_checked_trees::checked_trees::CheckedTrees>,
    pub(crate) selected:
        abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts,
    pub(crate) grants: Vec<ResolvedAuthoredSelectedProviderGrant>,
}

impl SelectedProviderPlanBinding {
    pub fn into_parts(
        self,
    ) -> (
        Arc<typed_trees_to_checked_trees::checked_trees::CheckedTrees>,
        abstract_operations_to_target_operations::effects::SelectedProviderPlanFacts,
        Vec<ResolvedAuthoredSelectedProviderGrant>,
    ) {
        (self.program, self.selected, self.grants)
    }
}

#[derive(Default)]
pub(crate) struct SelectedProviderProgramUpdates {
    pub(crate) admitted_receipts: Vec<(typed_trees_to_checked_trees::fact_plan::FactHandle, u64)>,
}

impl SelectedProviderProgramUpdates {
    pub(crate) fn is_empty(&self) -> bool {
        self.admitted_receipts.is_empty()
    }

    pub(crate) fn apply(
        self,
        checked: &mut typed_trees_to_checked_trees::checked_trees::CheckedTrees,
    ) {
        for (handle, identity) in self.admitted_receipts {
            checked
                .facts
                .semantic
                .facts
                .get_mut(handle)
                .evidence
                .receipt_identity = identity;
        }
    }
}
