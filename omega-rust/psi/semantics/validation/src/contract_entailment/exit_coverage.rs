use super::*;

/// Arm-wise entailment judges listed arms. Reusing that result for every
/// callable exit additionally requires that no implicit path was omitted.
/// Unsupported coverage stays with the path-sensitive exit prover.
pub(crate) fn entailment_covers_all_exits(program: &TypedTrees, machine: &Machine) -> bool {
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
                    program.expression_table.expression(*expression),
                    ExpressionNode::Boolean(true)
                ),
            })
            || complementary_boolean_guards(program, &guards)
            || complete_case_guards(program, &guards)
    })
}

fn complementary_boolean_guards(program: &TypedTrees, guards: &[TransitionGuardNode]) -> bool {
    let comparisons: Vec<_> = guards
        .iter()
        .filter_map(|guard| {
            let TransitionGuardNode::When(expression) = guard else {
                return None;
            };
            let ExpressionNode::Binary(comparison) =
                program.expression_table.expression(*expression)
            else {
                return None;
            };
            let ExpressionNode::Boolean(polarity) =
                program.expression_table.expression(comparison.right)
            else {
                return None;
            };
            (comparison.operator == BinaryOperator::Equal).then_some((comparison.left, *polarity))
        })
        .collect();
    comparisons.iter().any(|(left, polarity)| {
        comparisons.iter().any(|(right, other_polarity)| {
            polarity != other_polarity && same_condition(program, *left, *right)
        })
    })
}

fn complete_case_guards(program: &TypedTrees, guards: &[TransitionGuardNode]) -> bool {
    let comparisons: Vec<_> = guards
        .iter()
        .filter_map(|guard| {
            let TransitionGuardNode::When(expression) = guard else {
                return None;
            };
            let ExpressionNode::Binary(comparison) =
                program.expression_table.expression(*expression)
            else {
                return None;
            };
            let ExpressionNode::Name(case) = program.expression_table.expression(comparison.right)
            else {
                return None;
            };
            (comparison.operator == BinaryOperator::Equal).then_some((comparison.left, case.symbol))
        })
        .collect();
    comparisons.iter().any(|(subject, _)| {
        program.data_definitions().iter().any(|data| {
            let cases: Vec<_> = program
                .data_members(data)
                .iter()
                .filter_map(|member| {
                    let typed_trees::data::DataMember::Variant(case) = member else {
                        return None;
                    };
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

fn same_condition(program: &TypedTrees, left: ExpressionHandle, right: ExpressionHandle) -> bool {
    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            return left.operator == right.operator
                && same_condition(program, left.left, right.left)
                && same_condition(program, left.right, right.right);
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            let arguments = program.expression_table.expression_handles(left.arguments);
            let other_arguments = program.expression_table.expression_handles(right.arguments);
            let proof_only = typed_trees::proof_only::classify(program);
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
                        && program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == left.target_symbol)
                });
        }
        _ => {}
    }
    fn scalar(program: &TypedTrees, expression: ExpressionHandle) -> bool {
        match program.expression_table.expression(expression) {
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
        && program
            .expression_table
            .expressions_structurally_equal(left, right)
}
