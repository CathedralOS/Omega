//! Reuse checked call occurrences and the existing structured mutation owner.

use super::RangeFacts;
use crate::CallSite;
use crate::flow::CanonicalPlace;
use typed_trees::{TypedTrees, machine::Machine, state::State};

#[cfg(test)]
mod tests;

/// Immutable owner-local call identity, borrowed across incoming passes and branches.
pub(in crate::checks::ranges) struct RangeCallContext<'program> {
    machine: &'program Machine,
    state: &'program State,
    borrows: &'program checked_trees::BorrowFacts,
    borrow_calls: &'program [checked_trees::BorrowCallFact],
    flow_calls: &'program [checked_trees::FlowCallFact],
    call_frames: Option<&'program validation::CallFrameResolver<'program>>,
}

impl<'program> RangeCallContext<'program> {
    pub(in crate::checks::ranges) fn new(
        machine: &'program Machine,
        state: &'program State,
        borrows: &'program checked_trees::BorrowFacts,
        flow: &'program checked_trees::FlowFacts,
        call_frames: Option<&'program validation::CallFrameResolver<'program>>,
    ) -> Self {
        let borrow_calls = borrows
            .states
            .iter()
            .find_map(|(_, owner)| {
                (owner.machine_symbol == machine.symbol && owner.state_symbol == state.symbol)
                    .then(|| borrows.calls.span_or_empty(owner.calls))
            })
            .unwrap_or(&[]);
        let flow_calls = flow
            .control
            .states
            .iter()
            .find_map(|(_, owner)| {
                (owner.machine_symbol == machine.symbol && owner.state_symbol == state.symbol)
                    .then(|| flow.control.calls.span_or_empty(owner.calls))
            })
            .unwrap_or(&[]);
        Self {
            machine,
            state,
            borrows,
            borrow_calls,
            flow_calls,
            call_frames,
        }
    }

    fn find_call(
        &self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        statement_index: usize,
        site: &CallSite<'_>,
    ) -> Option<&checked_trees::BorrowCallFact> {
        if machine.symbol != self.machine.symbol || state.symbol != self.state.symbol {
            return None;
        }
        let call = self.flow_calls.iter().find(|call| {
            call.statement_index == statement_index && match site {
                CallSite::Expression { expression, .. } => expression.is_valid()
                    && call.authored_expression == *expression,
                CallSite::Statement(statement_call) => call.call_ordinal == 0
                    && !call.authored_expression.is_valid()
                    && matches!(program.statement_table.statements(state.statement_nodes).get(statement_index),
                        Some(typed_trees::statement::StatementNode::Call(candidate))
                            if std::ptr::eq(*statement_call, candidate)),
                CallSite::TransitionNamed { .. } => false,
            }
        })?;
        self.borrow_calls.iter().find(|borrow_call| {
            borrow_call.statement_index == call.statement_index
                && borrow_call.call_ordinal == call.call_ordinal
                && borrow_call.target_symbol == call.target_symbol
        })
    }
}

impl RangeFacts<'_> {
    pub(super) fn structured_call_writes(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        state: &State,
        site: &CallSite<'_>,
    ) -> Option<Vec<CanonicalPlace>> {
        let context = self.checked_calls?;
        let call = context.find_call(program, machine, state, self.statement_index, site)?;
        let mut writes = crate::flow::call_mutated_places(
            program,
            machine.symbol,
            state.symbol,
            context.borrows,
            call,
            &self.mutation_summaries,
            context.call_frames,
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
        crate::flow::close_storage_places_over_aliases_with_resolver(
            program,
            machine.symbol,
            state.symbol,
            self.statement_index,
            writes,
            context.call_frames,
        )
    }
}
