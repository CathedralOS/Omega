//! Closed and parameter-backed guards admitted by composed Unit control.

use super::super::{
    CheckedScalarExpression, CheckedScalarExpressionRole, CheckedStructuralScalarParameterPlan,
    PrimitiveType, SymbolHandle,
};
use checked_trees::CheckedScalarExpressionPlans;

/// The deepest a chain of immutable local initializers the guard inliner
/// follows: each bound expression may only name earlier locals, so a small
/// bound rejects runaway recursion on malformed input.
const LOCAL_INLINE_DEPTH: u32 = 8;

pub(super) fn exact_guard(
    expression: &CheckedScalarExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    expressions: &CheckedScalarExpressionPlans,
    state: SymbolHandle,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::Boolean(boolean) = expression else {
        return None;
    };
    // Immutable locals evaluate the same pure initializer the caller bound
    // before the transition; inline each `Local` leaf so the admitted
    // grammar below sees only operand roots the joined caller can evaluate.
    let resolved = inline_guard_locals(
        boolean,
        scalar_parameters.len(),
        expressions,
        state,
        LOCAL_INLINE_DEPTH,
    )?;
    if admitted_guard_shape(&resolved, scalar_parameters)
        || (scalar_parameters.is_empty() && closed_boolean(&resolved))
    {
        return Some(CheckedScalarExpression::Boolean(Box::new(resolved)));
    }
    None
}

/// Rewrite every `Local` leaf of a Boolean guard to the immutable local's
/// bound initializer, so a `let b = ...; transition b` caller joins on the
/// value it computed rather than on a local slot the joined caller does not
/// carry. Unresolvable or cyclic references decline the whole guard.
fn inline_guard_locals(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameter_count: usize,
    expressions: &CheckedScalarExpressionPlans,
    state: SymbolHandle,
    depth: u32,
) -> Option<checked_trees::CheckedBooleanExpression> {
    if depth == 0 {
        return None;
    }
    match expression {
        checked_trees::CheckedBooleanExpression::Local { position } => {
            let bound =
                bound_local_initializer(expressions, state, *position, scalar_parameter_count)?;
            let CheckedScalarExpression::Boolean(inner) = bound else {
                return None;
            };
            inline_guard_locals(inner, scalar_parameter_count, expressions, state, depth - 1)
        }
        checked_trees::CheckedBooleanExpression::Not(inner) => {
            Some(checked_trees::CheckedBooleanExpression::Not(Box::new(
                inline_guard_locals(inner, scalar_parameter_count, expressions, state, depth)?,
            )))
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            Some(checked_trees::CheckedBooleanExpression::Equal {
                left: Box::new(inline_guard_locals(
                    left,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
                right: Box::new(inline_guard_locals(
                    right,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
            })
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            Some(checked_trees::CheckedBooleanExpression::IntegerComparison {
                kind: *kind,
                left: Box::new(inline_scalar_locals(
                    left,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
                right: Box::new(inline_scalar_locals(
                    right,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
            })
        }
        checked_trees::CheckedBooleanExpression::And { left, right } => {
            Some(checked_trees::CheckedBooleanExpression::And {
                left: Box::new(inline_guard_locals(
                    left,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
                right: Box::new(inline_guard_locals(
                    right,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
            })
        }
        checked_trees::CheckedBooleanExpression::Or { left, right } => {
            Some(checked_trees::CheckedBooleanExpression::Or {
                left: Box::new(inline_guard_locals(
                    left,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
                right: Box::new(inline_guard_locals(
                    right,
                    scalar_parameter_count,
                    expressions,
                    state,
                    depth,
                )?),
            })
        }
        _ => Some(expression.clone()),
    }
}

/// The same `Local` inlining for a scalar operand of a comparison: a bound
/// integer local contributes its initializer, and composite integer forms
/// recurse so a `let x = ...; transition x < 5` caller joins on the value.
fn inline_scalar_locals(
    expression: &CheckedScalarExpression,
    scalar_parameter_count: usize,
    expressions: &CheckedScalarExpressionPlans,
    state: SymbolHandle,
    depth: u32,
) -> Option<CheckedScalarExpression> {
    if depth == 0 {
        return None;
    }
    match expression {
        CheckedScalarExpression::Local { position, .. } => {
            let bound =
                bound_local_initializer(expressions, state, *position, scalar_parameter_count)?;
            inline_scalar_locals(bound, scalar_parameter_count, expressions, state, depth - 1)
        }
        CheckedScalarExpression::Boolean(inner) => {
            Some(CheckedScalarExpression::Boolean(Box::new(
                inline_guard_locals(inner, scalar_parameter_count, expressions, state, depth)?,
            )))
        }
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left,
            right,
        } => Some(CheckedScalarExpression::IntegerBinary {
            kind: *kind,
            primitive_type: *primitive_type,
            left: Box::new(inline_scalar_locals(
                left,
                scalar_parameter_count,
                expressions,
                state,
                depth,
            )?),
            right: Box::new(inline_scalar_locals(
                right,
                scalar_parameter_count,
                expressions,
                state,
                depth,
            )?),
        }),
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand,
        } => Some(CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type: *primitive_type,
            operand: Box::new(inline_scalar_locals(
                operand,
                scalar_parameter_count,
                expressions,
                state,
                depth,
            )?),
        }),
        CheckedScalarExpression::IntegerWiden {
            primitive_type,
            operand,
        } => Some(CheckedScalarExpression::IntegerWiden {
            primitive_type: *primitive_type,
            operand: Box::new(inline_scalar_locals(
                operand,
                scalar_parameter_count,
                expressions,
                state,
                depth,
            )?),
        }),
        _ => Some(expression.clone()),
    }
}

/// The checked initializer of one immutable local binding in this state —
/// `Local` positions index the dense scalar namespace (parameters, then
/// immutable locals), and the `binding_ordinal` key records the same
/// local index.
fn bound_local_initializer(
    expressions: &CheckedScalarExpressionPlans,
    state: SymbolHandle,
    position: usize,
    scalar_parameter_count: usize,
) -> Option<&CheckedScalarExpression> {
    let binding_ordinal = u32::try_from(position.checked_sub(scalar_parameter_count)?).ok()?;
    expressions
        .expressions
        .iter()
        .find(|expression| {
            expression.state == state
                && matches!(
                    expression.role,
                    CheckedScalarExpressionRole::LocalInitializer {
                        binding_ordinal: ordinal
                    } if ordinal == binding_ordinal
                )
        })
        .map(|expression| &expression.expression)
}

/// Guard shapes the joined caller can evaluate itself: a Boolean parameter
/// operand, a retained `self` field, equality or integer comparisons over
/// the admitted operand roots, and negations of those. Everything else —
/// locals and calls — still declines.
fn admitted_guard_shape(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::Parameter { position } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool),
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => retained_field_subject(*parameter_position, path),
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            (boolean_subject(left) || boolean_subject(right))
                && boolean_operand(left, scalar_parameters)
                && boolean_operand(right, scalar_parameters)
        }
        checked_trees::CheckedBooleanExpression::Not(inner) => {
            admitted_guard_shape(inner, scalar_parameters)
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            (integer_subject(left) || integer_subject(right))
                && integer_operand(left, scalar_parameters)
                && integer_operand(right, scalar_parameters)
        }
        _ => false,
    }
}

/// A retained-field read rooted at an authored structural parameter no
/// deeper than the implicit `self` slot, walking record fields — or the
/// same walk crossing one literal index, mirroring the field-binding
/// resolution grammar: the index may end the carrier before its field leaf
/// or terminate the path as an inline element read. Case segments, deeper
/// indexes, and indexes mid-carrier decline. Authored structural parameters
/// still decline at the unit-topology roster, so position 0 is what reaches
/// here today.
fn retained_field_subject(
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> bool {
    parameter_position <= 1
        && !path.is_empty()
        && path
            .iter()
            .filter(|segment| {
                matches!(
                    segment,
                    checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_)
                )
            })
            .count()
            <= 1
        && path
            .iter()
            .enumerate()
            .all(|(ordinal, segment)| match segment {
                checked_trees::CheckedStructuralPredicatePathSegment::Field(_) => true,
                checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(_) => {
                    ordinal + 2 == path.len() || ordinal + 1 == path.len()
                }
                _ => false,
            })
}

/// One Boolean operand of a joined equality: a Boolean parameter, a
/// retained `self` field, or a Boolean literal.
fn boolean_operand(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::Parameter { position } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool),
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => retained_field_subject(*parameter_position, path),
        _ => false,
    }
}

/// One integer operand of a joined comparison: a scalar parameter (typed
/// to match it), a retained `self` field, or an integer literal.
fn integer_operand(
    expression: &CheckedScalarExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == *primitive_type),
        CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } => retained_field_subject(*parameter_position, path),
        _ => false,
    }
}

/// Whether a comparison operand names a runtime subject — a joined
/// parameter or a retained field — rather than a pair of literals.
fn boolean_subject(expression: &checked_trees::CheckedBooleanExpression) -> bool {
    matches!(
        expression,
        checked_trees::CheckedBooleanExpression::Parameter { .. }
            | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
    )
}

fn integer_subject(expression: &CheckedScalarExpression) -> bool {
    matches!(
        expression,
        CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::StructuralParameterField { .. }
    )
}

/// Whether an immutable local's checked initializer is a pure value the
/// joined caller can re-evaluate — or drop when unreferenced — without
/// changing the program's effects: parameter and retained-field reads,
/// literals, and pure compositions of those. Locals, calls, storage reads,
/// indexed reads, short-circuit booleans, and trapping casts decline.
pub(super) fn evaluatable_scalar(expression: &CheckedScalarExpression) -> bool {
    match expression {
        CheckedScalarExpression::Parameter { .. }
        | CheckedScalarExpression::IntegerLiteral { .. }
        | CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterField { .. } => true,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            evaluatable_scalar(left) && evaluatable_scalar(right)
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerWrappingCast { operand, .. }
        | CheckedScalarExpression::IntegerSaturatingCast { operand, .. } => {
            evaluatable_scalar(operand)
        }
        CheckedScalarExpression::Boolean(inner) => evaluatable_boolean(inner),
        _ => false,
    }
}

fn evaluatable_boolean(expression: &checked_trees::CheckedBooleanExpression) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::Constant(_)
        | checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => true,
        checked_trees::CheckedBooleanExpression::Not(inner) => evaluatable_boolean(inner),
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            evaluatable_boolean(left) && evaluatable_boolean(right)
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. }
        | checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison {
            left, right, ..
        } => evaluatable_scalar(left) && evaluatable_scalar(right),
        _ => false,
    }
}

fn closed_boolean(expression: &checked_trees::CheckedBooleanExpression) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            matches!(
                left.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            ) && matches!(
                right.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            )
        }
        _ => false,
    }
}
