use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, ProofConstraint,
    ProofObligation, ProofPlan,
};
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::TransitionGuardNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerRange {
    minimum: i64,
    maximum: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FloatRange {
    minimum: f64,
    maximum: f64,
}

pub fn check_proof_plan(proof_plan: &ProofPlan) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, obligation) in proof_plan.obligations.iter() {
        match obligation {
            ProofObligation::BoundedAssignment(obligation) => {
                check_bounded_assignment(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedCallArgument(obligation) => {
                check_bounded_call_argument(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedInitializer(obligation) => {
                check_bounded_initializer(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedStateReturn(obligation) => {
                check_bounded_state_return(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedTransitionArgument(obligation) => {
                check_bounded_transition_argument(proof_plan, obligation, &mut diagnostics);
            }
            ProofObligation::BoundedValue(_) | ProofObligation::GuardedTransition(_) => {}
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_bounded_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_assignment_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = guarded_integer_range_for_assignment(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_assignment_integer(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_assignment_integer(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_assignment(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_assignment_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_assignment_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn check_bounded_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_initializer_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = integer_range_for_initializer(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_initializer_integer(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_initializer_integer(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_initializer(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_initializer_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_initializer_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn check_bounded_state_return(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_return_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = integer_range_for_return_value(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_return_integer(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_return_integer(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_return_value(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_return_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_return_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn check_bounded_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_call_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(argument_range) = integer_range_for_call_argument(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_call_integer(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_call_integer(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(argument_range) = float_range_for_call_argument(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_call_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_call_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn check_bounded_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    check_transition_named_constraints(proof_plan, obligation, diagnostics);

    if let Some(target_range) =
        integer_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let argument_range = guarded_integer_range_for_transition_argument(proof_plan, obligation);

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_transition_integer(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(argument_range) = float_range_for_transition_argument(proof_plan, obligation)
        else {
            diagnostics.push(cannot_prove_bounded_transition_float(
                proof_plan,
                obligation,
                target_range,
            ));
            return;
        };

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_transition_float(
                proof_plan,
                obligation,
                target_range,
            ));
        }
    }
}

fn integer_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn guarded_integer_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> IntegerRange {
    let base =
        integer_range_for_transition_argument(proof_plan, obligation).unwrap_or(IntegerRange {
            minimum: i64::MIN,
            maximum: i64::MAX,
        });

    apply_handle_guard(proof_plan, base, obligation.argument, &obligation.guard)
}

fn float_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(*value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => float_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn float_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(*value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }
    }
}

fn float_range_for_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(*value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => float_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn float_range_for_return_value(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(*value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }
    }
}

fn float_range_for_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(*value)?;
            Some(FloatRange {
                minimum: value,
                maximum: value,
            })
        }
        _ => None,
    }
}

fn integer_range_for_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn integer_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.value_constraints,
        )),
    }
}

fn guarded_integer_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<IntegerRange> {
    let mut range = integer_range_for_assignment(proof_plan, obligation)?;

    if let Some(guard) = &obligation.state_guard {
        range = apply_assignment_guard(proof_plan, range, obligation.value, guard);
    }

    Some(range)
}

fn integer_range_for_return_value(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.value_constraints,
        )),
    }
}

fn integer_range_for_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
) -> Option<IntegerRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => None,
    }
}

fn integer_range_from_constraints(constraints: &[ProofConstraint]) -> Option<IntegerRange> {
    let mut range: Option<IntegerRange> = None;

    for constraint in constraints {
        let ProofConstraint::IntegerRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = IntegerRange {
            minimum: *minimum,
            maximum: *maximum,
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    for constraint in constraints {
        let ProofConstraint::Named(name) = constraint else {
            continue;
        };

        let implied = match name.as_str() {
            "non_negative" => Some(IntegerRange {
                minimum: 0,
                maximum: i64::MAX,
            }),
            "positive" => Some(IntegerRange {
                minimum: 1,
                maximum: i64::MAX,
            }),
            _ => None,
        };

        let Some(implied) = implied else {
            continue;
        };

        range = Some(match range {
            Some(existing) => IntegerRange {
                minimum: existing.minimum.max(implied.minimum),
                maximum: existing.maximum.min(implied.maximum),
            },
            None => implied,
        });
    }

    range
}

fn type_constraints<'proof>(
    proof_plan: &'proof ProofPlan<'_>,
    constraints: HandleSpan<ProofConstraint>,
) -> &'proof [ProofConstraint] {
    proof_plan.type_constraints.span(constraints).unwrap_or(&[])
}

fn float_range_from_constraints(constraints: &[ProofConstraint]) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints {
        let ProofConstraint::FloatRange { minimum, maximum } = constraint else {
            continue;
        };

        let candidate = FloatRange {
            minimum: minimum.value(),
            maximum: maximum.value(),
        };

        range = Some(match range {
            Some(existing) => FloatRange {
                minimum: existing.minimum.max(candidate.minimum),
                maximum: existing.maximum.min(candidate.maximum),
            },
            None => candidate,
        });
    }

    range
}

fn finite_float_literal(value: omega_typed_trees::expression::FloatLiteral) -> Option<f64> {
    let value = value.value();
    value.is_finite().then_some(value)
}

fn integer_literal_handle(proof_plan: &ProofPlan, expression: ExpressionHandle) -> Option<i64> {
    match proof_plan.program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Name(path)
            if proof_plan
                .program
                .expression_table
                .name_path_members(path.members)
                .iter()
                .map(|member| member.as_str())
                .eq(["u32", "MAX"]) =>
        {
            Some(u32::MAX as i64)
        }
        _ => None,
    }
}

fn apply_handle_guard(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> IntegerRange {
    match guard {
        TransitionGuardNode::Always => range,
        TransitionGuardNode::When(condition) => {
            apply_handle_condition(proof_plan, range, argument, *condition)
        }
    }
}

fn apply_assignment_guard(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    value: ExpressionHandle,
    guard: &TransitionGuardNode,
) -> IntegerRange {
    let range = apply_handle_guard(proof_plan, range, value, guard);

    let ExpressionNode::Binary(value_binary) =
        proof_plan.program.expression_table.expression(value)
    else {
        return range;
    };
    if value_binary.operator != BinaryOperator::Subtract {
        return range;
    }

    let TransitionGuardNode::When(condition) = guard else {
        return range;
    };
    let condition = unwrap_true_guard_condition(proof_plan, *condition);
    let ExpressionNode::Binary(condition_binary) =
        proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    let lower_bound =
        if expressions_equivalent_for_proof(proof_plan, value_binary.left, condition_binary.left)
            && expressions_equivalent_for_proof(
                proof_plan,
                value_binary.right,
                condition_binary.right,
            )
        {
            match condition_binary.operator {
                BinaryOperator::Greater => Some(1),
                BinaryOperator::GreaterOrEqual => Some(0),
                _ => None,
            }
        } else if expressions_equivalent_for_proof(
            proof_plan,
            value_binary.left,
            condition_binary.right,
        ) && expressions_equivalent_for_proof(
            proof_plan,
            value_binary.right,
            condition_binary.left,
        ) {
            match condition_binary.operator {
                BinaryOperator::Less => Some(1),
                BinaryOperator::LessOrEqual => Some(0),
                _ => None,
            }
        } else {
            None
        };

    let Some(lower_bound) = lower_bound else {
        return range;
    };

    IntegerRange {
        minimum: range.minimum.max(lower_bound),
        maximum: range.maximum,
    }
}

fn unwrap_true_guard_condition(
    proof_plan: &ProofPlan,
    condition: ExpressionHandle,
) -> ExpressionHandle {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return condition;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return binary.left;
        }

        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return binary.right;
        }
    }

    condition
}

fn apply_handle_condition(
    proof_plan: &ProofPlan,
    range: IntegerRange,
    argument: ExpressionHandle,
    condition: ExpressionHandle,
) -> IntegerRange {
    let ExpressionNode::Binary(binary) = proof_plan.program.expression_table.expression(condition)
    else {
        return range;
    };

    if binary.operator == BinaryOperator::Equal {
        if matches!(
            proof_plan.program.expression_table.expression(binary.right),
            ExpressionNode::Boolean(true)
        ) {
            return apply_handle_condition(proof_plan, range, argument, binary.left);
        }

        if matches!(
            proof_plan.program.expression_table.expression(binary.left),
            ExpressionNode::Boolean(true)
        ) {
            return apply_handle_condition(proof_plan, range, argument, binary.right);
        }
    }

    if binary.operator == BinaryOperator::And {
        let range = apply_handle_condition(proof_plan, range, argument, binary.left);
        return apply_handle_condition(proof_plan, range, argument, binary.right);
    }

    if expressions_equivalent_for_proof(proof_plan, binary.left, argument) {
        return apply_right_literal_guard(proof_plan, range, binary.operator, binary.right);
    }

    if expressions_equivalent_for_proof(proof_plan, binary.right, argument) {
        return apply_left_literal_guard(proof_plan, range, binary.left, binary.operator);
    }

    range
}

fn expressions_equivalent_for_proof(
    proof_plan: &ProofPlan,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (
        proof_plan.program.expression_table.expression(left),
        proof_plan.program.expression_table.expression(right),
    ) {
        (ExpressionNode::Mutable(left), _) => {
            expressions_equivalent_for_proof(proof_plan, *left, right)
        }
        (_, ExpressionNode::Mutable(right)) => {
            expressions_equivalent_for_proof(proof_plan, left, *right)
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            proof_plan
                .program
                .expression_table
                .name_path_members(left.members)
                == proof_plan
                    .program
                    .expression_table
                    .name_path_members(right.members)
        }
        (ExpressionNode::Call(left), ExpressionNode::Call(right)) => {
            left.target == right.target
                && left.target_symbol == right.target_symbol
                && left.arguments.count() == right.arguments.count()
                && match (left.receiver.is_valid(), right.receiver.is_valid()) {
                    (true, true) => {
                        expressions_equivalent_for_proof(proof_plan, left.receiver, right.receiver)
                    }
                    (false, false) => true,
                    _ => false,
                }
                && proof_plan
                    .program
                    .expression_table
                    .expression_handles(left.arguments)
                    .iter()
                    .zip(
                        proof_plan
                            .program
                            .expression_table
                            .expression_handles(right.arguments),
                    )
                    .all(|(left_argument, right_argument)| {
                        expressions_equivalent_for_proof(
                            proof_plan,
                            *left_argument,
                            *right_argument,
                        )
                    })
        }
        (ExpressionNode::Member(left), ExpressionNode::Member(right)) => {
            left.member == right.member
                && left.member_symbol == right.member_symbol
                && expressions_equivalent_for_proof(proof_plan, left.receiver, right.receiver)
        }
        _ => false,
    }
}

fn apply_right_literal_guard(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    operator: BinaryOperator,
    right: ExpressionHandle,
) -> IntegerRange {
    let Some(value) = integer_literal_handle(proof_plan, right) else {
        return range;
    };

    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value);
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.minimum = range.minimum.max(value.saturating_add(1)),
        BinaryOperator::GreaterOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Less => range.maximum = range.maximum.min(value.saturating_sub(1)),
        BinaryOperator::LessOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => {}
    }

    range
}

fn apply_left_literal_guard(
    proof_plan: &ProofPlan,
    mut range: IntegerRange,
    left: ExpressionHandle,
    operator: BinaryOperator,
) -> IntegerRange {
    let Some(value) = integer_literal_handle(proof_plan, left) else {
        return range;
    };

    match operator {
        BinaryOperator::Equal => {
            range.minimum = range.minimum.max(value);
            range.maximum = range.maximum.min(value);
        }
        BinaryOperator::Greater => range.maximum = range.maximum.min(value.saturating_sub(1)),
        BinaryOperator::GreaterOrEqual => range.maximum = range.maximum.min(value),
        BinaryOperator::Less => range.minimum = range.minimum.max(value.saturating_add(1)),
        BinaryOperator::LessOrEqual => range.minimum = range.minimum.max(value),
        BinaryOperator::Add
        | BinaryOperator::And
        | BinaryOperator::Divide
        | BinaryOperator::Modulo
        | BinaryOperator::Multiply
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight
        | BinaryOperator::Subtract => {}
    }

    range
}

fn check_call_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.argument,
            obligation.argument_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_call_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_assignment_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !assignment_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_assignment_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_transition_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !transition_argument_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_transition_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_return_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.value,
            obligation.value_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_return_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn check_initializer_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !initializer_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_initializer_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn named_constraints(constraints: &[ProofConstraint]) -> impl Iterator<Item = &str> {
    constraints.iter().filter_map(|constraint| {
        let ProofConstraint::Named(name) = constraint else {
            return None;
        };

        Some(name.as_str())
    })
}

fn argument_handle_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    argument: ExpressionHandle,
    argument_constraints: HandleSpan<ProofConstraint>,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, argument_constraints);

    constraints_satisfy_named_constraint(constraints, constraint)
        || match (
            constraint,
            proof_plan.program.expression_table.expression(argument),
        ) {
            ("exact", ExpressionNode::Integer(_)) => true,
            ("finite", ExpressionNode::Float(value)) => finite_float_literal(*value).is_some(),
            ("finite", ExpressionNode::Integer(_)) => true,
            ("non_negative", ExpressionNode::Integer(value)) => *value >= 0,
            ("positive", ExpressionNode::Integer(value)) => *value > 0,
            ("wrapping", ExpressionNode::Integer(_)) => true,
            _ => false,
        }
}

fn transition_argument_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, obligation.argument_constraints);

    if constraints_satisfy_named_constraint(constraints, constraint) {
        return true;
    }

    if matches!(constraint, "positive" | "non_negative") {
        let range = guarded_integer_range_for_transition_argument(proof_plan, obligation);
        return match constraint {
            "positive" => range.minimum > 0,
            "non_negative" => range.minimum >= 0,
            _ => false,
        };
    }

    if matches!(constraint, "exact")
        && integer_range_for_transition_argument(proof_plan, obligation)
            .is_some_and(|range| range.minimum == range.maximum)
    {
        return true;
    }

    argument_handle_satisfies_named_constraint(
        proof_plan,
        obligation.argument,
        obligation.argument_constraints,
        constraint,
    )
}

fn assignment_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, obligation.value_constraints);

    if constraints_satisfy_named_constraint(constraints, constraint) {
        return true;
    }

    if matches!(constraint, "positive" | "non_negative")
        && let Some(range) = guarded_integer_range_for_assignment(proof_plan, obligation)
    {
        return match constraint {
            "positive" => range.minimum > 0,
            "non_negative" => range.minimum >= 0,
            _ => false,
        };
    }

    if matches!(constraint, "exact")
        && guarded_integer_range_for_assignment(proof_plan, obligation)
            .is_some_and(|range| range.minimum == range.maximum)
    {
        return true;
    }

    argument_handle_satisfies_named_constraint(
        proof_plan,
        obligation.value,
        obligation.value_constraints,
        constraint,
    )
}

fn initializer_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    constraint: &str,
) -> bool {
    match (
        constraint,
        proof_plan
            .program
            .expression_table
            .expression(obligation.value),
    ) {
        ("finite", ExpressionNode::Float(value)) => finite_float_literal(*value).is_some(),
        ("exact", ExpressionNode::Integer(_)) => true,
        ("non_negative", ExpressionNode::Integer(value)) => *value >= 0,
        ("positive", ExpressionNode::Integer(value)) => *value > 0,
        ("wrapping", ExpressionNode::Integer(_)) => true,
        _ => false,
    }
}

fn constraints_satisfy_named_constraint(constraints: &[ProofConstraint], constraint: &str) -> bool {
    if constraints.iter().any(|argument_constraint| {
        matches!(
            argument_constraint,
            ProofConstraint::Named(argument_constraint)
                if argument_constraint.as_str() == constraint
        )
    }) {
        return true;
    }

    match constraint {
        "exact" => integer_range_from_constraints(constraints).is_some(),
        "finite" => {
            integer_range_from_constraints(constraints).is_some()
                || float_range_from_constraints(constraints).is_some()
        }
        "non_negative" => {
            integer_range_from_constraints(constraints).is_some_and(|range| range.minimum >= 0)
        }
        "positive" => {
            integer_range_from_constraints(constraints).is_some_and(|range| range.minimum > 0)
        }
        "wrapping" => false,
        _ => false,
    }
}

fn cannot_prove_bounded_transition_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_assignment_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.value),
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_return_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.value),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_initializer_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies bounded value `{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.value),
        obligation.owner,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_transition_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_assignment_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.value),
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_return_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.value),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_initializer_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies bounded value `{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.value),
        obligation.owner,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_transition_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies `{}` for bounded parameter `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.argument),
        constraint,
        obligation.parameter,
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_assignment_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies `{}` for bounded target `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        expression_display_name(proof_plan, obligation.target),
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_return_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies `{}` for bounded return type in `{}.{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_initializer_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies `{}` for bounded value `{}`",
        expression_display_name(proof_plan, obligation.value),
        constraint,
        obligation.owner
    ))
}

fn expression_display_name(proof_plan: &ProofPlan, expression: ExpressionHandle) -> String {
    proof_plan.program.expression_table.display_name(expression)
}

fn cannot_prove_bounded_call_integer(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.to_string());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_call_float(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.to_string());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected range<{}, {}>",
        expression_display_name(proof_plan, obligation.argument),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_call_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.to_string());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies `{}` for bounded parameter `{}` for `{}` in `{}.{}`",
        expression_display_name(proof_plan, obligation.argument),
        constraint,
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state
    ))
}
