use super::{
    BinaryOperator, ExpressionHandle, ExpressionNode, Machine, StatementNode, TransitionGuardNode,
};
use crate::frozen_program::expression_table;
/// Arm-wise entailment judges listed arms. Reusing that result for every
/// callable exit additionally requires that no implicit path was omitted.
/// Unsupported coverage stays with the path-sensitive exit prover.
pub(crate) fn entailment_covers_all_exits(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
) -> bool {
    program.machine_states(machine).iter().all(|state| {
        let guards: Vec<_> = program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::Transition(transition) => Some(transition.guard),
                _ => None,
            })
            .collect();
        guards.is_empty()
            || guards.iter().any(|guard| match guard {
                TransitionGuardNode::Always => true,
                TransitionGuardNode::When(expression) => matches!(
                    expression_table(program).expression(*expression),
                    ExpressionNode::Boolean(true)
                ),
            })
            || complementary_boolean_guards(program, &guards)
            || complete_case_guards(program, &guards)
    })
}

/// The `when` guards' equality comparisons as (left, mapped-right) pairs: a
/// non-`when`, non-binary, or non-`==` guard contributes nothing, and
/// `map_right` decides which right-operand shapes each caller reads.
fn equality_comparisons<T>(
    program: &dyn crate::CheckingInput,
    guards: &[TransitionGuardNode],
    map_right: impl Fn(&typed_trees::expression::TableBinaryExpression) -> Option<T>,
) -> Vec<(ExpressionHandle, T)> {
    guards
        .iter()
        .filter_map(|guard| {
            let TransitionGuardNode::When(expression) = guard else {
                return None;
            };
            let Some(comparison) = expression_table(program).binary_node(*expression) else {
                return None;
            };
            (comparison.operator == BinaryOperator::Equal)
                .then(|| map_right(comparison))
                .flatten()
                .map(|right| (comparison.left, right))
        })
        .collect()
}

fn complementary_boolean_guards(
    program: &dyn crate::CheckingInput,
    guards: &[TransitionGuardNode],
) -> bool {
    let comparisons = equality_comparisons(program, guards, |comparison| {
        let polarity = expression_table(program).boolean_node(comparison.right)?;
        Some(*polarity)
    });
    comparisons.iter().any(|(left, polarity)| {
        comparisons.iter().any(|(right, other_polarity)| {
            polarity != other_polarity && same_condition(program, *left, *right)
        })
    })
}

fn complete_case_guards(
    program: &dyn crate::CheckingInput,
    guards: &[TransitionGuardNode],
) -> bool {
    let comparisons = equality_comparisons(program, guards, |comparison| {
        let Some(case) = expression_table(program).name_path_node(comparison.right) else {
            return None;
        };
        Some(case.symbol)
    });
    comparisons.iter().any(|(subject, _)| {
        program.data_definitions().iter().any(|data| {
            let cases: Vec<_> = program
                .data_members(data)
                .iter()
                .filter_map(|member| {
                    let case = member.as_variant()?;
                    Some(case.symbol)
                })
                .collect();
            !cases.is_empty()
                && cases.iter().all(|case| {
                    comparisons.iter().any(|(other_subject, selected_case)| {
                        case == selected_case && same_condition(program, *subject, *other_subject)
                    })
                })
        })
    })
}

fn same_condition(
    program: &dyn crate::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    match (
        expression_table(program).expression(left),
        expression_table(program).expression(right),
    ) {
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            return left.operator == right.operator
                && same_condition(program, left.left, right.left)
                && same_condition(program, left.right, right.right);
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            let arguments = expression_table(program).expression_handles(left.arguments);
            let other_arguments = expression_table(program).expression_handles(right.arguments);
            let proof_only = crate::proof_only_classification(program);
            return left.target_symbol.is_valid()
                && left.target_symbol == right.target_symbol
                && !left.receiver.is_valid()
                && !right.receiver.is_valid()
                && left.machine_arguments == right.machine_arguments
                && left.static_requirement_dispatch == right.static_requirement_dispatch
                && left.quotient_operation.is_none()
                && right.quotient_operation.is_none()
                && left.private_layout_operation.is_none()
                && right.private_layout_operation.is_none()
                && left.evidence_arguments.is_empty()
                && right.evidence_arguments.is_empty()
                && arguments.len() == other_arguments.len()
                && arguments
                    .iter()
                    .zip(other_arguments)
                    .all(|(left, right)| same_condition(program, *left, *right))
                && program.machines().iter().any(|machine| {
                    proof_only.is_proof_machine(program, machine)
                        && typed_trees::state::state_by_symbol(
                            program.machine_states(machine),
                            left.target_symbol,
                        )
                        .is_some()
                });
        }
        _ => {}
    }
    fn scalar(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
        match expression_table(program).expression(expression) {
            ExpressionNode::Boolean(_) | ExpressionNode::Integer(_) => true,
            ExpressionNode::Name(path) => path.symbol.is_valid(),
            ExpressionNode::Binary(binary) => {
                scalar(program, binary.left) && scalar(program, binary.right)
            }
            _ => false,
        }
    }
    // The general equality helper does not compare a call's static machine
    // arguments. Calls above have their own exact denotational identity.
    // Integer equality here retains the full literal, not a lossy i64 view.
    scalar(program, left)
        && scalar(program, right)
        && expression_table(program).expressions_structurally_equal(left, right)
}
