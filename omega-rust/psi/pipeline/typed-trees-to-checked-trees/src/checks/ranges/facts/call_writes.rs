//! Reuse checked call occurrences and the existing structured mutation owner.

use super::RangeFacts;
use crate::CallSite;
use crate::flow::CanonicalPlace;
use typed_trees::{TypedTrees, machine::Machine, state::State};

impl RangeFacts<'_> {
    pub(super) fn structured_call_writes(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        site: &CallSite<'_>,
    ) -> Option<Vec<CanonicalPlace>> {
        let borrows = self.checked_borrows?;
        let state_borrows = borrows
            .states
            .iter()
            .find_map(|(_, row)| (row.state_symbol == state.symbol).then_some(row))?;
        let call = borrows
            .calls
            .span_or_empty(state_borrows.calls)
            .iter()
            .find(|call| {
                call.statement_index == self.statement_index
                    && crate::find_call_site(
                        program,
                        machine.symbol,
                        state.symbol,
                        call.statement_index,
                        call.call_ordinal,
                    )
                    .is_some_and(|candidate| same_site(site, &candidate))
            })?;
        let mut writes = crate::flow::call_mutated_places(
            program,
            machine.symbol,
            state.symbol,
            borrows,
            call,
            &mut self.mutation_summaries,
        )?;
        // Callee expressions and previously captured selector expressions do
        // not execute in the caller's current value namespace. Unknown index
        // coordinates may overlap; only retained fixed selectors narrow writes.
        for write in &mut writes {
            for segment in &mut write.segments {
                if let facts::PlaceSegment::Index { expression } = segment {
                    *expression = typed_trees::expression::ExpressionHandle::invalid();
                }
            }
        }
        crate::flow::close_storage_places_over_aliases(
            program,
            machine.symbol,
            state.symbol,
            self.statement_index,
            writes,
        )
    }
}

fn same_site(left: &CallSite<'_>, right: &CallSite<'_>) -> bool {
    match (left, right) {
        (CallSite::Statement(left), CallSite::Statement(right)) => std::ptr::eq(*left, *right),
        (
            CallSite::Expression {
                expression: left, ..
            },
            CallSite::Expression {
                expression: right, ..
            },
        ) => left == right,
        _ => false,
    }
}
