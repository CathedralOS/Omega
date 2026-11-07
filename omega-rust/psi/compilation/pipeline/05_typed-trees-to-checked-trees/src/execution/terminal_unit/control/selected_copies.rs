//! Associate selected returns with their completed payload reads once. Both
//! scalar and structural consumers use the same checked target and copy list.

use super::CheckFacts;
use checked_trees::{CheckedScalarBranchDestination, CheckedScalarReturnCopies};
use std::collections::BTreeMap;
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

pub(in crate::execution::terminal_unit) struct SelectedCopies<'a> {
    program: &'a dyn validation::CheckingInput,
    state: &'a typed_trees::state::State,
    outgoing: BTreeMap<(usize, u32, u32), Option<Vec<checked_trees::CheckedSelectedPayloadCopy>>>,
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
            let mut values = Vec::new();
            for copy in facts.flow.control.operands.span(row.copied_payloads)? {
                let checked_trees::FlowOperandCompletion::Copied { copy: identity, .. } =
                    copy.completion
                else {
                    return None;
                };
                if copy.state_symbol != state.symbol
                    || copy.statement_index != row.statement_index
                    || copy.evaluation
                        != checked_trees::FlowPermissionEvaluation::TransitionTarget(target)
                {
                    return None;
                }
                if program
                    .primitive_type_reference(copy.type_reference)
                    .is_some()
                    && validation::reference_shell(program, copy.type_reference).is_none()
                {
                    continue;
                }
                let root = facts.values.structural_values.root_for_captured_copy(
                    state.symbol,
                    u32::try_from(row.statement_index).ok()?,
                    identity,
                )?;
                if root.machine != machine
                    || root.state != state.symbol
                    || root.type_reference != copy.type_reference
                    || root.statement_ordinal as usize != row.statement_index
                    || !validation::type_satisfies_declared_property(
                        program,
                        &[],
                        root.type_reference,
                        validation::DeclaredPropertyRequirement::Copy,
                    )
                    || !validation::has_contents(
                        program,
                        root.type_reference,
                        &[],
                        validation::ContentsRequirement::DescriptorCleanup,
                    )
                {
                    return None;
                }
                values.push(checked_trees::CheckedSelectedPayloadCopy {
                    value: root.root,
                    type_identity: validation::stored_structural_type_identity(
                        program,
                        root.type_reference,
                        &[],
                        &[],
                    ),
                });
            }
            outgoing
                .entry((
                    row.statement_index,
                    target.arena_index(),
                    target.generation(),
                ))
                .and_modify(|captured| *captured = None)
                .or_insert(Some(values));
        }
        Some(Self {
            program,
            state,
            outgoing,
        })
    }

    pub(in crate::execution::terminal_unit) fn into_rows(
        self,
    ) -> Option<Vec<checked_trees::CheckedSelectedPayloadCopies>> {
        self.outgoing
            .into_iter()
            .map(|((statement, index, generation), values)| {
                Some(checked_trees::CheckedSelectedPayloadCopies {
                    statement_ordinal: u32::try_from(statement).ok()?,
                    transition_target: arena::Handle::from_parts(index, generation),
                    values: values?,
                })
            })
            .collect()
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
        let (expression, target, copied_payloads) = match self
            .program
            .statement_table
            .statement_at(self.state, *statement_ordinal as usize)?
        {
            StatementNode::Expression(expression) if !is_continuation => {
                (*expression, Default::default(), Vec::new())
            }
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
                    .cloned()
                    .flatten()?;
                (*expression, target, copies)
            }
            _ => return None,
        };
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
