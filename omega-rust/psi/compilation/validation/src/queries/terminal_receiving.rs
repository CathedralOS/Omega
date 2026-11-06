//! Checking a program without an entry still owes its saved-reference checks.
//! Keep the machines containing actual uses that need those checks, rather
//! than selecting bodies from their signatures or scanning unused values.

use checked_trees::{CheckedTrees, FlowOperandCompletion, FlowOperandFact, FlowStateFact};
use std::collections::HashSet;
use symbols::SymbolHandle;

/// Callers whose retained values need the final provider checks. Retention
/// grants no forwarding, membership or permission; those checks still run.
pub fn terminal_receiving_machines(
    checked: &CheckedTrees,
) -> impl Iterator<Item = SymbolHandle> + '_ {
    checked
        .facts
        .flow
        .control
        .states
        .iter()
        .filter_map(move |(_, state)| {
            state_needs_receiving(checked, state).then_some(state.machine_symbol)
        })
}

fn state_needs_receiving(checked: &CheckedTrees, state: &FlowStateFact) -> bool {
    let control = &checked.facts.flow.control;
    let mut pending = Vec::new();
    for call in control.calls.span_or_empty(state.calls) {
        if !call.lender_preservation.is_empty() || !call.reference_forwarding.is_empty() {
            return true;
        }
        pending.extend(call.operands.handles());
    }
    for statement in control.statements.span_or_empty(state.statements) {
        pending.extend([
            statement.stored_value,
            statement.evaluated_value,
            statement.store_target,
        ]);
        for invocation in control
            .operator_invocations
            .span_or_empty(statement.operator_invocations)
        {
            pending.extend(invocation.operands.handles());
        }
        for observation in control
            .value_observations
            .span_or_empty(statement.value_observations)
        {
            pending.extend([observation.receiver, observation.result]);
        }
    }
    for exit in control.exits.span_or_empty(state.exits) {
        pending.push(exit.returned_value);
    }
    for outgoing in control.outgoing.span_or_empty(state.outgoing) {
        pending.push(outgoing.returned_value);
        pending.extend(outgoing.copied_payloads.handles());
    }
    let mut visited = HashSet::new();
    while let Some(capture) = pending.pop() {
        if !capture.is_valid() || !visited.insert(capture) {
            continue;
        }
        if !control.operands.is_valid(capture) {
            // An unavailable retained occurrence cannot prove the checks are
            // unnecessary. Keep its owner for ordinary validation to reject.
            return true;
        }
        let operand = control.operands.get(capture);
        if forms_exclusive_reference_carrier(checked, operand) {
            return true;
        }
        pending.extend([operand.backing, operand.value_source]);
        pending.extend(operand.evaluated_inputs.handles());
        match operand.completion {
            FlowOperandCompletion::Constructed(children) => {
                let Some(children) = control.operand_children.span(children) else {
                    return true;
                };
                pending.extend(children.iter().map(|child| child.value));
            }
            FlowOperandCompletion::Selected(alternatives) => {
                let Some(alternatives) = control.operand_alternatives.span(alternatives) else {
                    return true;
                };
                if alternatives.is_empty() {
                    return true;
                }
                for alternative in alternatives {
                    pending.extend([alternative.pattern, alternative.value]);
                }
            }
            FlowOperandCompletion::Copied { copy, .. } => {
                if !control.copied_values.is_valid(copy) {
                    return true;
                }
                pending.push(control.copied_values.get(copy).source);
            }
            FlowOperandCompletion::Observation
            | FlowOperandCompletion::IncomingContents { .. }
            | FlowOperandCompletion::InvocationResult { .. } => {}
        }
    }
    false
}

fn forms_exclusive_reference_carrier(checked: &CheckedTrees, operand: &FlowOperandFact) -> bool {
    let Some(formation) = crate::reference_formation_source(&checked.typed, operand.expression)
    else {
        return false;
    };
    if formation.access == language_semantics::ReferenceAccess::Shared {
        return false;
    }
    let Some(reference) = crate::reference_shell(&checked.typed, operand.type_reference) else {
        return true;
    };
    // Replacing a number cannot install a new saved reference. Whole pointer
    // cells and carriers with reference fields need the complete output roster;
    // lifetime-only dependencies are not physical pointer slots.
    let Some(paths) =
        crate::borrow_contracts::carried_reference_paths(&checked.typed, reference.referee)
    else {
        return true;
    };
    paths.into_iter().any(|(path, _)| {
        let Some(path) = crate::borrow_contracts::structural_owner_path(&checked.typed, &path)
            .and_then(|path| {
                crate::reference_result_custody::loan_place_segments(
                    &checked.typed,
                    reference.referee,
                    &path,
                )
            })
        else {
            return true;
        };
        let Some(leaf) =
            crate::owned_projection_type_reference(&checked.typed, reference.referee, &path)
        else {
            return true;
        };
        let Some(leaf) = crate::reference_shell(&checked.typed, leaf) else {
            return true;
        };
        // A slice stores its pointer and extent as a view descriptor, rather
        // than the reference-word role checked by the forwarding collector.
        !matches!(
            checked
                .typed
                .type_reference_table
                .type_reference(crate::peel_declared_constraints(
                    &checked.typed,
                    leaf.referee
                ),),
            typed_trees::types::TypeReferenceNode::Slice { .. }
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena::{Handle, HandleSpan};
    use checked_trees::FlowStatementFact;

    fn with_retained_value(completion: FlowOperandCompletion) -> CheckedTrees {
        let mut checked = CheckedTrees::default();
        let control = &mut checked.facts.flow.control;
        let value = control.operands.append(FlowOperandFact {
            completion,
            ..Default::default()
        });
        let statements = control.statements.insert_many([FlowStatementFact {
            evaluated_value: value,
            ..Default::default()
        }]);
        control.states.append(FlowStateFact {
            machine_symbol: Handle::from_arena_index(1),
            statements,
            ..Default::default()
        });
        checked
    }

    #[test]
    fn unavailable_required_value_children_keep_their_checking_owner() {
        for completion in [
            FlowOperandCompletion::Constructed(HandleSpan::from_parts(
                Handle::from_arena_index(1),
                1,
            )),
            FlowOperandCompletion::Selected(HandleSpan::from_parts(Handle::from_arena_index(1), 1)),
            FlowOperandCompletion::Selected(HandleSpan::empty()),
        ] {
            let checked = with_retained_value(completion);
            assert_eq!(
                terminal_receiving_machines(&checked).collect::<Vec<_>>(),
                vec![Handle::from_arena_index(1)]
            );
        }
    }

    #[test]
    fn a_complete_empty_value_does_not_create_a_receiving_requirement() {
        let checked = with_retained_value(FlowOperandCompletion::Constructed(HandleSpan::empty()));
        assert!(terminal_receiving_machines(&checked).next().is_none());
    }
}
