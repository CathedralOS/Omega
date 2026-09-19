//! Exact successor handoffs and authored Boolean fallback guards.
use super::super::{
    CheckFacts, CheckedStructuralControlSuccessorPlan, CheckedStructuralControlTransferPlan,
    CheckedStructuralScalarParameterPlan, CheckedUnitEntryClaimPlan,
    CheckedUnitStructuralParameterPlan, ExpressionNode, SymbolHandle, TransitionGuardNode,
    TransitionTargetNode, TypedTrees,
};

use crate::execution::terminal_unit::is_reference;

pub(in crate::execution::terminal_unit) fn exact_false_fallback(
    program: &TypedTrees,
    when_true: &typed_trees::statement::TableTransition,
    when_false: &typed_trees::statement::TableTransition,
) -> bool {
    match when_false.guard {
        TransitionGuardNode::Always => true,
        TransitionGuardNode::When(expression) => {
            let TransitionGuardNode::When(true_expression) = when_true.guard else {
                return false;
            };
            let Some(true_subject) = labeled_boolean_guard(program, true_expression, true) else {
                return false;
            };
            let Some(false_subject) = labeled_boolean_guard(program, expression, false) else {
                return false;
            };
            program
                .expression_table
                .expressions_structurally_equal(true_subject, false_subject)
        }
    }
}

fn labeled_boolean_guard(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
    expected: bool,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    (binary.operator == typed_trees::expression::BinaryOperator::Equal
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(value) if *value == expected
        ))
    .then_some(binary.left)
}

pub(super) fn only_implicit_reference_self_is_omitted(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    scalar: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    program
        .state_parameters(state)
        .iter()
        .enumerate()
        .all(|(position, parameter)| {
            structural
                .iter()
                .any(|candidate| candidate.position as usize == position)
                || scalar
                    .iter()
                    .any(|candidate| candidate.source_position as usize == position)
                || (parameter.is_self && is_reference(program, parameter.type_reference))
                || parameter.relevance.is_erased()
        })
}

pub(super) fn successor(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    source_state: &typed_trees::state::State,
    source_parameters: &[CheckedUnitStructuralParameterPlan],
    target_parameters: &[CheckedUnitStructuralParameterPlan],
    source_claims: &[CheckedUnitEntryClaimPlan],
    target_claims: &[CheckedUnitEntryClaimPlan],
    ordinal: u32,
    transition: &typed_trees::statement::TableTransition,
    expected: SymbolHandle,
    admitted_local_discards: &[SymbolHandle],
) -> Option<CheckedStructuralControlSuccessorPlan> {
    let TransitionTargetNode::Named {
        path, arguments, ..
    } = program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    if path.symbol != expected {
        return None;
    }
    let arguments = program.statement_table.expression_handles(*arguments);
    let transfers = match (
        source_parameters,
        target_parameters,
        source_claims,
        target_claims,
        arguments,
    ) {
        ([], [], [], [], []) => Vec::new(),
        ([source], [target], [source_claim], [target_claim], [argument]) => {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                source_state.symbol,
                usize::try_from(ordinal).ok()?,
                *argument,
            )?;
            let facts::PlaceRoot::Symbol(root) = place.root else {
                return None;
            };
            let source_symbol = program
                .state_parameters(source_state)
                .get(source.position as usize)?
                .symbol;
            if root != source_symbol
                || !place.segments.is_empty()
                || source.type_identity != target.type_identity
                || source.multiplicity != target.multiplicity
                || source.access != target.access
                || !super::custody::exact_claim_alias_events(
                    facts,
                    machine,
                    source_state,
                    ordinal,
                    expected,
                    source_symbol,
                    source_claim,
                    target_claim,
                )
            {
                return None;
            }
            vec![CheckedStructuralControlTransferPlan {
                source: checked_trees::CheckedStructuralControlTransferSourcePlan::Parameter {
                    index: 0,
                },
                target_parameter_index: 0,
            }]
        }
        _ => return None,
    };
    if admitted_local_discards.is_empty() {
        let cleanup = facts.flow.terminal_structural_control_cleanups.for_edge(
            machine.symbol,
            source_state.symbol,
            ordinal,
        )?;
        if cleanup.target_state != expected
            || !cleanup
                .trivial_affine_discard_parameter_positions
                .is_empty()
        {
            return None;
        }
    } else if admitted_local_discards.len() != 1
        || !source_parameters.is_empty()
        || !target_parameters.is_empty()
        || !source_claims.is_empty()
        || !target_claims.is_empty()
        || !super::super::types::return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            source_state.symbol,
            source_parameters,
            program.state_parameters(source_state),
            &[],
            admitted_local_discards,
        )?
        .is_empty()
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: expected,
        transfers,
        scalar_arguments: Vec::new(),
        erased_arguments: Vec::new(),
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
