//! Owned children of a borrowed construction live in its retained backing home.
//! A descriptor observes that home; it never becomes the children's owner.
use super::*;

impl Retirement<'_> {
    pub(super) fn backing_retired(
        &self,
        value: checked_trees::CheckedStructuralValueHandle,
        path: &[checked_trees::BorrowLoanOwnerSegment],
        active: &mut Vec<RetirementNode>,
    ) -> bool {
        let values = &self.facts.values.structural_values;
        if !values.nodes.is_valid(value) {
            return false;
        }
        let node = values.nodes.get(value);
        if node.machine != self.machine
            || node.state != self.state.symbol
            || !self
                .order
                .at_or_before(node.statement_index as usize, self.statement_index as usize)
            || values
                .root_for_expression(node.state, node.statement_index, node.expression)
                .is_none_or(|root| root.root != value)
        {
            return false;
        }
        let owner = facts::PlaceRoot::Expression(node.expression);
        // The ordinary contextual owner owns this death and all of its
        // descriptor leaves. Before exit, only a completed exact call cleanup
        // can settle it; a future hook or a dependency edge cannot do so.
        if self.facts.contextual_cleanup.any_state_disposition(
            self.machine,
            self.state.symbol,
            |_, disposition| {
                disposition.root == owner
                    && disposition.path.is_empty()
                    && disposition.exclusions.is_empty()
                    && matches!(disposition.source,
                        language_semantics::PermissionEventSource::Call { statement_index, call_ordinal, .. }
                        if statement_index == node.statement_index as usize
                            && (self.at_exit || self.operations[..self.completed_operations].iter().any(|operation| {
                                matches!(operation, CheckedUnitEffectOperationPlan::ContinuationCleanup {
                                    boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate), ..
                                } if coordinate.statement_index as usize == statement_index
                                    && coordinate.call_ordinal as usize == call_ordinal)
                            })))
            },
        ) {
            return true;
        }
        // A projected receiving region may finish independently. Follow its
        // exact stored loan occurrences, including every descriptor beneath
        // the transferred subtree. Missing receiving metadata stays live.
        let mut found = false;
        let retired = self
            .facts
            .borrow
            .loans
            .iter()
            .filter(|(_, loan)| {
                loan.storage_owner == owner
                    && loan.statement_index == node.statement_index as usize
                    && self.facts.borrow.loan_owner_path(loan).starts_with(path)
            })
            .all(|(handle, _)| {
                found = true;
                self.holder_retired_following(handle, active)
            });
        found && retired
    }
}
