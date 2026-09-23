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
    facts: &CheckFacts,
    when_true: &typed_trees::statement::TableTransition,
    when_false: &typed_trees::statement::TableTransition,
) -> bool {
    match when_false.guard {
        TransitionGuardNode::Always => true,
        TransitionGuardNode::When(expression) => {
            let TransitionGuardNode::When(true_expression) = when_true.guard else {
                return false;
            };
            boolean_literal_fallback(program, true_expression, expression)
                || builtin_complement_guard(program, facts, true_expression, expression)
                || closed_case_complement(program, true_expression, expression)
        }
    }
}

/// `subject == literal` paired with `subject == !literal` over one subject is
/// the exact Boolean split: the second arm holds exactly where the first
/// fails, in either authored order and with the literal on either side.
fn boolean_literal_fallback(
    program: &TypedTrees,
    true_expression: typed_trees::expression::ExpressionHandle,
    false_expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let (Some((true_subject, true_value)), Some((false_subject, false_value))) = (
        boolean_label(program, true_expression),
        boolean_label(program, false_expression),
    ) else {
        return false;
    };
    true_value != false_value
        && program
            .expression_table
            .expressions_structurally_equal(true_subject, false_subject)
}

/// `subject == literal` or `literal == subject` with a Boolean literal side.
fn boolean_label(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<(typed_trees::expression::ExpressionHandle, bool)> {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if binary.operator != typed_trees::expression::BinaryOperator::Equal {
        return None;
    }
    if let ExpressionNode::Boolean(value) = program.expression_table.expression(binary.right) {
        return Some((binary.left, *value));
    }
    if let ExpressionNode::Boolean(value) = program.expression_table.expression(binary.left) {
        return Some((binary.right, *value));
    }
    None
}

/// A case test pair `subject == A` / `subject == B` over one closed sum is
/// the authored two-variant split: exactly one variant holds at a time, so
/// the second arm fires exactly where the first fails. Only a two-variant
/// roster makes an authored pair exhaustive — a three-or-more variant sum
/// leaves cases the pair never names, and a repeated case splits nothing.
/// The subject is re-read for the second guard, so it must evaluate to the
/// value the first guard observed, exactly like the builtin complement.
fn closed_case_complement(
    program: &TypedTrees,
    true_expression: typed_trees::expression::ExpressionHandle,
    false_expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let (Some((true_subject, true_case)), Some((false_subject, false_case))) = (
        crate::proof::exact_outcome_case_test(program, true_expression),
        crate::proof::exact_outcome_case_test(program, false_expression),
    ) else {
        return false;
    };
    if true_case == false_case
        || !program
            .expression_table
            .expressions_structurally_equal(true_subject, false_subject)
        || !reevaluation_stable(program, true_subject)
    {
        return false;
    }
    let variant_owner = |case| {
        program.data_definitions().iter().find(|definition| {
            program.data_members(definition).iter().any(|member| {
                matches!(
                    member,
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.symbol == case
                )
            })
        })
    };
    let Some(owner) = variant_owner(true_case) else {
        return false;
    };
    let Some(other) = variant_owner(false_case) else {
        return false;
    };
    if owner.symbol != other.symbol {
        return false;
    }
    program.data_members(owner).iter().all(|member| {
        matches!(member, typed_trees::data::DataMember::Variant(variant)
            if variant.symbol == true_case || variant.symbol == false_case)
    })
}

/// Builtin `L == R` paired with `L != R` over identical operands is the
/// authored `!(L == R)`: equality's own complement, so the second guard
/// holds exactly where the first fails — for integer, Boolean, and float
/// subjects alike (NaN makes `!=` true where `==` is false). Ordered pairs
/// such as `<`/`>=` are not admitted this way: NaN falsifies both sides.
///
/// The admission is exact only while the second guard's operands re-evaluate
/// to the values the first guard observed: builtin operators have no authored
/// bodies, and `reevaluation_stable` keeps calls, atomic observations, and
/// deferred match evaluation out of the operand trees.
fn builtin_complement_guard(
    program: &TypedTrees,
    facts: &CheckFacts,
    true_expression: typed_trees::expression::ExpressionHandle,
    false_expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let (ExpressionNode::Binary(true_binary), ExpressionNode::Binary(false_binary)) = (
        program.expression_table.expression(true_expression),
        program.expression_table.expression(false_expression),
    ) else {
        return false;
    };
    if !matches!(
        (true_binary.operator, false_binary.operator),
        (
            typed_trees::expression::BinaryOperator::Equal,
            typed_trees::expression::BinaryOperator::NotEqual
        ) | (
            typed_trees::expression::BinaryOperator::NotEqual,
            typed_trees::expression::BinaryOperator::Equal
        )
    ) {
        return false;
    }
    program
        .expression_table
        .expressions_structurally_equal(true_binary.left, false_binary.left)
        && program
            .expression_table
            .expressions_structurally_equal(true_binary.right, false_binary.right)
        && crate::values::operator_is_builtin(&facts.operators, true_expression)
        && crate::values::operator_is_builtin(&facts.operators, false_expression)
        && reevaluation_stable(program, true_binary.left)
        && reevaluation_stable(program, true_binary.right)
}

/// The second guard's operands are re-read after the first guard fails. Calls
/// carry effects and need not repeat their result, atomic observations can
/// see intervening writes, and match evaluation defers arbitrary arms — none
/// of those nodes can stand in for the value the first guard observed.
fn reevaluation_stable(
    program: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    let mut nodes = Vec::new();
    crate::monomorphization::collect_expression_tree(program, expression, &mut nodes);
    nodes.iter().all(|node| {
        !matches!(
            program.expression_table.expression(*node),
            ExpressionNode::Match(_) | ExpressionNode::Atomic(_) | ExpressionNode::Call(_)
        )
    })
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
        || super::super::types::return_unit_affine_discards(
            program,
            facts,
            machine.symbol,
            source_state.symbol,
            source_parameters,
            program.state_parameters(source_state),
            &[],
            admitted_local_discards,
            // With no operations there are no moved projections to
            // reconstruct, so the residual lookup never reads the map.
            &std::collections::BTreeMap::new(),
        )
        .is_none_or(|(trivial, residuals, _)| !trivial.is_empty() || !residuals.is_empty())
    {
        return None;
    }
    Some(CheckedStructuralControlSuccessorPlan {
        statement_ordinal: ordinal,
        target_state: expected,
        transfers,
        scalar_arguments: Vec::new(),
        erased_arguments: Vec::new(),
        erased_proof_arguments: Vec::new(),
        trivial_affine_discard_parameter_positions: Vec::new(),
    })
}
