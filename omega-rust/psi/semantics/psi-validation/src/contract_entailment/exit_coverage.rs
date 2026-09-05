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
                    let psi_typed_trees::data::DataMember::Variant(case) = member else {
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
    // arguments. Exclude calls instead of using it as generic call identity.
    // Integer equality here retains the full literal, not a lossy i64 view.
    scalar(program, left)
        && scalar(program, right)
        && program
            .expression_table
            .expressions_structurally_equal(left, right)
}
