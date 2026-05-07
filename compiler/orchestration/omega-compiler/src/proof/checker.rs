use crate::diagnostics::Diagnostic;
use crate::ir::expression::{BinaryOperator, Expression};
use crate::ir::statement::TransitionGuard;
use crate::ir::types::TypeConstraint;
use crate::proof::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, ProofObligation, ProofPlan,
};
use omega_core::arena::HandleSpan;

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

    for obligation in &proof_plan.obligations {
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
        let Some(value_range) = integer_range_for_assignment(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_assignment_integer(
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_assignment_integer(
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
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_assignment_float(
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
        let Some(value_range) = integer_range_for_initializer(obligation) else {
            diagnostics.push(cannot_prove_bounded_initializer_integer(
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_initializer_integer(
                obligation,
                target_range,
            ));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_initializer(obligation) else {
            diagnostics.push(cannot_prove_bounded_initializer_float(
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_initializer_float(
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
                obligation,
                target_range,
            ));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_return_integer(
                obligation,
                target_range,
            ));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(value_range) = float_range_for_return_value(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_return_float(obligation, target_range));
            return;
        };

        if value_range.minimum < target_range.minimum || value_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_return_float(obligation, target_range));
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
            diagnostics.push(cannot_prove_bounded_call_integer(obligation, target_range));
            return;
        };

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_call_integer(obligation, target_range));
        }
    }

    if let Some(target_range) =
        float_range_from_constraints(type_constraints(proof_plan, obligation.constraints))
    {
        let Some(argument_range) = float_range_for_call_argument(proof_plan, obligation) else {
            diagnostics.push(cannot_prove_bounded_call_float(obligation, target_range));
            return;
        };

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_call_float(obligation, target_range));
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
        let Some(mut argument_range) =
            integer_range_for_transition_argument(proof_plan, obligation)
        else {
            diagnostics.push(cannot_prove_bounded_transition_integer(
                obligation,
                target_range,
            ));
            return;
        };

        argument_range = apply_guard(argument_range, &obligation.argument, &obligation.guard);

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_transition_integer(
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
                obligation,
                target_range,
            ));
            return;
        };

        if argument_range.minimum < target_range.minimum
            || argument_range.maximum > target_range.maximum
        {
            diagnostics.push(cannot_prove_bounded_transition_float(
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
    match &obligation.argument {
        Expression::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

fn float_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<FloatRange> {
    match &obligation.argument {
        Expression::Float(value) => {
            let value = float_literal(value)?;
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
    match &obligation.value {
        Expression::Float(value) => {
            let value = float_literal(value)?;
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
    match &obligation.argument {
        Expression::Float(value) => {
            let value = float_literal(value)?;
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
    match &obligation.value {
        Expression::Float(value) => {
            let value = float_literal(value)?;
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

fn float_range_for_initializer(obligation: &BoundedInitializerObligation) -> Option<FloatRange> {
    match &obligation.value {
        Expression::Float(value) => {
            let value = float_literal(value)?;
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
    match &obligation.argument {
        Expression::Integer(value) => Some(IntegerRange {
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
    match &obligation.value {
        Expression::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => integer_range_from_constraints(type_constraints(
            proof_plan,
            obligation.value_constraints,
        )),
    }
}

fn integer_range_for_return_value(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> Option<IntegerRange> {
    match &obligation.value {
        Expression::Integer(value) => Some(IntegerRange {
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
    obligation: &BoundedInitializerObligation,
) -> Option<IntegerRange> {
    match &obligation.value {
        Expression::Integer(value) => Some(IntegerRange {
            minimum: *value,
            maximum: *value,
        }),
        _ => None,
    }
}

fn integer_range_from_constraints(constraints: &[TypeConstraint]) -> Option<IntegerRange> {
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            return None;
        };

        Some(IntegerRange {
            minimum: integer_literal(minimum)?,
            maximum: integer_literal(maximum)?,
        })
    })
}

fn type_constraints(
    proof_plan: &ProofPlan,
    constraints: HandleSpan<TypeConstraint>,
) -> &[TypeConstraint] {
    proof_plan.type_constraints.span(constraints).unwrap_or(&[])
}

fn float_range_from_constraints(constraints: &[TypeConstraint]) -> Option<FloatRange> {
    constraints.iter().find_map(|constraint| {
        let TypeConstraint::Range { minimum, maximum } = constraint else {
            return None;
        };
        if !matches!(minimum, Expression::Float(_)) && !matches!(maximum, Expression::Float(_)) {
            return None;
        }

        Some(FloatRange {
            minimum: float_literal_expression(minimum)?,
            maximum: float_literal_expression(maximum)?,
        })
    })
}

fn integer_literal(expression: &Expression) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        _ => None,
    }
}

fn float_literal_expression(expression: &Expression) -> Option<f64> {
    match expression {
        Expression::Float(value) => float_literal(value),
        Expression::Integer(value) => Some(*value as f64),
        _ => None,
    }
}

fn float_literal(value: &str) -> Option<f64> {
    let trimmed = value.trim_end_matches(['f', 'F']);
    let parsed = trimmed.parse::<f64>().ok()?;

    parsed.is_finite().then_some(parsed)
}

fn apply_guard(
    range: IntegerRange,
    argument: &Expression,
    guard: &TransitionGuard,
) -> IntegerRange {
    match guard {
        TransitionGuard::Always => range,
        TransitionGuard::When(condition) => apply_condition(range, argument, condition),
    }
}

fn apply_condition(
    range: IntegerRange,
    argument: &Expression,
    condition: &Expression,
) -> IntegerRange {
    let Expression::Binary(binary) = condition else {
        return range;
    };

    if binary.operator == BinaryOperator::And {
        let range = apply_condition(range, argument, &binary.left);
        return apply_condition(range, argument, &binary.right);
    }

    if binary.left == *argument {
        return apply_right_literal_guard(range, binary.operator, &binary.right);
    }

    if binary.right == *argument {
        return apply_left_literal_guard(range, &binary.left, binary.operator);
    }

    range
}

fn apply_right_literal_guard(
    mut range: IntegerRange,
    operator: BinaryOperator,
    right: &Expression,
) -> IntegerRange {
    let Some(value) = integer_literal(right) else {
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
        | BinaryOperator::NotEqual
        | BinaryOperator::Or => {}
    }

    range
}

fn apply_left_literal_guard(
    mut range: IntegerRange,
    left: &Expression,
    operator: BinaryOperator,
) -> IntegerRange {
    let Some(value) = integer_literal(left) else {
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
        | BinaryOperator::NotEqual
        | BinaryOperator::Or => {}
    }

    range
}

fn check_call_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_satisfies_named_constraint(
            proof_plan,
            &obligation.argument,
            obligation.argument_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_call_named_constraint(obligation, constraint));
        }
    }
}

fn check_assignment_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_satisfies_named_constraint(
            proof_plan,
            &obligation.value,
            obligation.value_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_assignment_named_constraint(
                obligation, constraint,
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
        if !argument_satisfies_named_constraint(
            proof_plan,
            &obligation.argument,
            obligation.argument_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_transition_named_constraint(
                obligation, constraint,
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
        if !argument_satisfies_named_constraint(
            proof_plan,
            &obligation.value,
            obligation.value_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_return_named_constraint(obligation, constraint));
        }
    }
}

fn check_initializer_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !expression_is_finite_literal(&obligation.value) || constraint != "finite" {
            diagnostics.push(cannot_prove_initializer_named_constraint(
                obligation, constraint,
            ));
        }
    }
}

fn named_constraints(constraints: &[TypeConstraint]) -> impl Iterator<Item = &str> {
    constraints.iter().filter_map(|constraint| {
        let TypeConstraint::Named(name) = constraint else {
            return None;
        };

        Some(name.as_str())
    })
}

fn argument_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    argument: &Expression,
    argument_constraints: HandleSpan<TypeConstraint>,
    constraint: &str,
) -> bool {
    type_constraints(proof_plan, argument_constraints)
        .iter()
        .any(|argument_constraint| {
            matches!(
                argument_constraint,
                TypeConstraint::Named(argument_constraint) if argument_constraint == constraint
            )
        })
        || (constraint == "finite" && expression_is_finite_literal(argument))
}

fn expression_is_finite_literal(expression: &Expression) -> bool {
    match expression {
        Expression::Float(value) => float_literal(value).is_some(),
        Expression::Integer(_) => true,
        _ => false,
    }
}

fn cannot_prove_bounded_transition_integer(
    obligation: &BoundedTransitionArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected range<{}, {}>",
        obligation.argument.display_name(),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_assignment_integer(
    obligation: &BoundedAssignmentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected range<{}, {}>",
        obligation.value.display_name(),
        obligation.target.display_name(),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_return_integer(
    obligation: &BoundedStateReturnObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected range<{}, {}>",
        obligation.value.display_name(),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_initializer_integer(
    obligation: &BoundedInitializerObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies bounded value `{}`; expected range<{}, {}>",
        obligation.value.display_name(),
        obligation.owner,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_transition_float(
    obligation: &BoundedTransitionArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies bounded parameter `{}` in `{}.{}`; expected range<{}, {}>",
        obligation.argument.display_name(),
        obligation.parameter,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_assignment_float(
    obligation: &BoundedAssignmentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies bounded target `{}` in `{}.{}`; expected range<{}, {}>",
        obligation.value.display_name(),
        obligation.target.display_name(),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_return_float(
    obligation: &BoundedStateReturnObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies bounded return type in `{}.{}`; expected range<{}, {}>",
        obligation.value.display_name(),
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_initializer_float(
    obligation: &BoundedInitializerObligation,
    target_range: FloatRange,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies bounded value `{}`; expected range<{}, {}>",
        obligation.value.display_name(),
        obligation.owner,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_transition_named_constraint(
    obligation: &BoundedTransitionArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove transition argument `{}` satisfies `{}` for bounded parameter `{}` in `{}.{}`",
        obligation.argument.display_name(),
        constraint,
        obligation.parameter,
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_assignment_named_constraint(
    obligation: &BoundedAssignmentObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove assignment value `{}` satisfies `{}` for bounded target `{}` in `{}.{}`",
        obligation.value.display_name(),
        constraint,
        obligation.target.display_name(),
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_return_named_constraint(
    obligation: &BoundedStateReturnObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove return value `{}` satisfies `{}` for bounded return type in `{}.{}`",
        obligation.value.display_name(),
        constraint,
        obligation.machine,
        obligation.state
    ))
}

fn cannot_prove_initializer_named_constraint(
    obligation: &BoundedInitializerObligation,
    constraint: &str,
) -> Diagnostic {
    Diagnostic::error(format!(
        "cannot prove initializer `{}` satisfies `{}` for bounded value `{}`",
        obligation.value.display_name(),
        constraint,
        obligation.owner
    ))
}

fn cannot_prove_bounded_call_integer(
    obligation: &BoundedCallArgumentObligation,
    target_range: IntegerRange,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.clone());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected range<{}, {}>",
        obligation.argument.display_name(),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_bounded_call_float(
    obligation: &BoundedCallArgumentObligation,
    target_range: FloatRange,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.clone());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies bounded parameter `{}` for `{}` in `{}.{}`; expected range<{}, {}>",
        obligation.argument.display_name(),
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state,
        target_range.minimum,
        target_range.maximum
    ))
}

fn cannot_prove_call_named_constraint(
    obligation: &BoundedCallArgumentObligation,
    constraint: &str,
) -> Diagnostic {
    let target = obligation
        .receiver
        .as_ref()
        .map(|receiver| format!("{receiver}.{}", obligation.target))
        .unwrap_or_else(|| obligation.target.clone());

    Diagnostic::error(format!(
        "cannot prove call argument `{}` satisfies `{}` for bounded parameter `{}` for `{}` in `{}.{}`",
        obligation.argument.display_name(),
        constraint,
        obligation.parameter,
        target,
        obligation.machine,
        obligation.state
    ))
}
