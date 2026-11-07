//! Associate selected returns with their completed payload reads once. Both
//! scalar and structural consumers use the same checked target and copy list.

use super::CheckFacts;
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarReturnCopies};
use std::collections::BTreeMap;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

pub(in crate::execution::terminal_unit) struct SelectedCopies<'a> {
    program: &'a dyn validation::CheckingInput,
    facts: &'a CheckFacts,
    state: &'a typed_trees::state::State,
    outgoing:
        BTreeMap<(usize, u32, u32), Option<arena::HandleSpan<checked_trees::FlowOperandFact>>>,
}

impl<'a> SelectedCopies<'a> {
    pub(in crate::execution::terminal_unit) fn new(
        program: &'a dyn validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: symbols::SymbolHandle,
        state: &'a typed_trees::state::State,
    ) -> Option<Self> {
        let flow = super::super::types::state_flow(facts, machine, state.symbol)?;
        let mut outgoing = BTreeMap::new();
        for row in facts.flow.control.outgoing.span(flow.outgoing)? {
            if row.machine_symbol != machine || row.state_symbol != state.symbol {
                return None;
            }
            let target = row.transition_target;
            outgoing
                .entry((
                    row.statement_index,
                    target.arena_index(),
                    target.generation(),
                ))
                .and_modify(|captured| *captured = None)
                .or_insert(Some(row.copied_payloads));
        }
        Some(Self {
            program,
            facts,
            state,
            outgoing,
        })
    }

    pub(in crate::execution::terminal_unit) fn for_destination(
        &self,
        destination: &CheckedScalarBranchDestination,
    ) -> Option<(
        typed_trees::expression::ExpressionHandle,
        CheckedScalarReturnCopies,
    )> {
        let CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = destination
        else {
            return None;
        };
        let (expression, target, copied_payloads, evaluation) =
            match self
                .program
                .statement_table
                .statement_at(self.state, *statement_ordinal as usize)?
            {
                StatementNode::Expression(expression) if !is_continuation => (
                    *expression,
                    Default::default(),
                    arena::HandleSpan::empty(),
                    checked_trees::FlowPermissionEvaluation::Statement,
                ),
                StatementNode::Transition(transition)
                    if transition.exit == TransitionExit::Ordinary =>
                {
                    let target = if *is_continuation {
                        transition.continuation
                    } else {
                        transition.target
                    };
                    let TransitionTargetNode::Value(expression) =
                        self.program.statement_table.transition_target(target)
                    else {
                        return None;
                    };
                    let copies = self
                        .outgoing
                        .get(&(
                            *statement_ordinal as usize,
                            target.arena_index(),
                            target.generation(),
                        ))
                        .copied()
                        .flatten()?;
                    (
                        *expression,
                        target,
                        copies,
                        checked_trees::FlowPermissionEvaluation::TransitionTarget(target),
                    )
                }
                _ => return None,
            };
        for copy in self.facts.flow.control.operands.span(copied_payloads)? {
            if copy.state_symbol != self.state.symbol
                || copy.statement_index != *statement_ordinal as usize
                || copy.evaluation != evaluation
                || !matches!(
                    copy.completion,
                    checked_trees::FlowOperandCompletion::Copied { .. }
                )
            {
                return None;
            }
        }
        Some((
            expression,
            CheckedScalarReturnCopies {
                statement_ordinal: *statement_ordinal,
                is_continuation: *is_continuation,
                transition_target: target,
                copied_payloads,
            },
        ))
    }
}
