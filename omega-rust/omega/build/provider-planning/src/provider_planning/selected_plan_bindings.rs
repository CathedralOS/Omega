//! The selected provider plan binding and its program updates.

use std::sync::Arc;
use trust_model::ResolvedAuthoredSelectedProviderGrant;

/// Exact checked-program and selected-plan candidate after every provider
/// grant, receipt, operator-use, and installation-reach decision has replayed.
/// The candidate owns any separated Arc privately until its caller commits it.
#[derive(Debug)]
pub struct SelectedProviderPlanBinding {
    pub(crate) program: Arc<checked_trees::CheckedTrees>,
    pub(crate) selected: effects::SelectedProviderPlanFacts,
    pub(crate) grants: Vec<ResolvedAuthoredSelectedProviderGrant>,
}

impl SelectedProviderPlanBinding {
    pub fn into_parts(
        self,
    ) -> (
        Arc<checked_trees::CheckedTrees>,
        effects::SelectedProviderPlanFacts,
        Vec<ResolvedAuthoredSelectedProviderGrant>,
    ) {
        (self.program, self.selected, self.grants)
    }
}

#[derive(Default)]
pub(crate) struct SelectedProviderProgramUpdates {
    pub(crate) spelled_operator_uses: Vec<(
        arena::Handle<checked_trees::CheckedOperatorUseFact>,
        u64,
        checked_trees::CheckedProviderPlanCommitment,
    )>,
    pub(crate) named_operator_uses: Vec<(
        arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
        u64,
        checked_trees::CheckedProviderPlanCommitment,
    )>,
    pub(crate) named_requirement_uses: Vec<(
        arena::Handle<checked_trees::CheckedNamedRequirementUseFact>,
        u64,
        checked_trees::CheckedProviderPlanCommitment,
    )>,
    pub(crate) admitted_receipts: Vec<(facts::FactHandle, u64)>,
}

impl SelectedProviderProgramUpdates {
    pub(crate) fn is_empty(&self) -> bool {
        self.spelled_operator_uses.is_empty()
            && self.named_operator_uses.is_empty()
            && self.named_requirement_uses.is_empty()
            && self.admitted_receipts.is_empty()
    }

    pub(crate) fn apply(self, checked: &mut checked_trees::CheckedTrees) {
        for (handle, report_fingerprint, commitment) in self.spelled_operator_uses {
            let operator_use = checked.facts.operators.uses.get_mut(handle);
            operator_use.provider_plan_report_fingerprint = report_fingerprint;
            operator_use.provider_plan_commitment = commitment;
        }
        for (handle, report_fingerprint, commitment) in self.named_operator_uses {
            let operator_use = checked.facts.operators.named_uses.get_mut(handle);
            operator_use.provider_plan_report_fingerprint = report_fingerprint;
            operator_use.provider_plan_commitment = commitment;
        }
        for (handle, report_fingerprint, commitment) in self.named_requirement_uses {
            let requirement_use = checked
                .facts
                .operators
                .named_requirement_uses
                .get_mut(handle);
            requirement_use.provider_plan_report_fingerprint = report_fingerprint;
            requirement_use.provider_plan_commitment = commitment;
        }
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
