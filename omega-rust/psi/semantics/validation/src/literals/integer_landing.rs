//! Chapter 5 anonymous numeric arithmetic at integer and Boolean destinations.
//! The authored expression remains intact; consumers retain the rendered value
//! at its actual destination, not widths on the anonymous intermediate nodes.
//! A comparison between two anonymous trees consumes their exact rational values
//! directly: its Boolean result creates no numeric landing or integer-warning
//! obligation. Both operations share the same value traversal, including rejection
//! of zero divisors and previously landed operands. The caller retains selection
//! custody and must admit every operator before interpreting its value.

use diagnostics::Diagnostic;
use numerics::{
    arithmetic::ArithmeticDomain,
    bignum::{BigInt, BigRational},
    literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType},
};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    types::PrimitiveType,
};

mod destinations;
pub(crate) use destinations::anonymous_integer_landing_warnings;
pub(super) use destinations::append_destination_literals;

#[cfg(test)]
mod tests;

/// Render a wholly anonymous numeric expression at its first fixed-integer
/// destination. The caller owns operator-selection evidence. No named value,
/// cast, call, prior landing, or target-semantic observation is evaluated here.
pub fn land_anonymous_integer_expression(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    mut builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Option<IntegerLiteral> {
    let value = anonymous_numeric_value(program, expression, &mut builtin)?;
    land_integer_value(&value.value.to_integer_exact()?, destination)
}

/// Land one selected anonymous subtree and retain its first fractional
/// intermediate warning. Callers publish the warning only after full admission.
pub fn land_anonymous_integer_expression_with_warning(
    program: &TypedTrees,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    mut builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Option<(IntegerLiteral, Option<Diagnostic>)> {
    let value = anonymous_numeric_value(program, expression, &mut builtin)?;
    let integer = value.value.to_integer_exact()?;
    let literal = land_integer_value(&integer, destination)?;
    let warning = integer_landing_warning(program, &value, &integer, &mut builtin);
    Some((literal, warning))
}

/// Compare two wholly anonymous numeric trees without choosing a fixed carrier.
/// The callback must establish builtin meaning for the comparison and every
/// arithmetic node. A Boolean result grants no declaration-selection authority;
/// callers retain that evidence separately. Non-comparisons, prior landings,
/// invalid trees and anonymous division by zero return None.
pub fn evaluate_anonymous_numeric_comparison(
    program: &TypedTrees,
    expression: ExpressionHandle,
    mut builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Option<bool> {
    use std::cmp::Ordering;

    if !program.expression_table.expression_is_valid(expression) {
        return None;
    }
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return None;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) || !builtin(expression)
    {
        return None;
    }
    let left = anonymous_numeric_value(program, binary.left, &mut builtin)?;
    let right = anonymous_numeric_value(program, binary.right, &mut builtin)?;
    let ordering = left.value.cmp_value(&right.value);
    Some(match binary.operator {
        BinaryOperator::Equal => ordering == Ordering::Equal,
        BinaryOperator::NotEqual => ordering != Ordering::Equal,
        BinaryOperator::Less => ordering == Ordering::Less,
        BinaryOperator::LessOrEqual => ordering != Ordering::Greater,
        BinaryOperator::Greater => ordering == Ordering::Greater,
        BinaryOperator::GreaterOrEqual => ordering != Ordering::Less,
        _ => return None,
    })
}

/// Compare exact anonymous values without inventing a numeric destination.
/// The caller must establish builtin meaning for each arithmetic node.
pub(crate) fn evaluate_anonymous_numeric_equality(
    program: &TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
    mut builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Option<bool> {
    let left = anonymous_numeric_value(program, left, &mut builtin)?;
    let right = anonymous_numeric_value(program, right, &mut builtin)?;
    Some(left.value.cmp_value(&right.value).is_eq())
}

/// Select an authored Match result using only total anonymous numeric values.
/// No runtime carrier or destination is introduced. Even a leading wildcard
/// needs an evaluable subject: a call, prior landing, or undefined arithmetic
/// cannot disappear merely because its value is unused. Tested patterns retain
/// authored order; later patterns and result bodies are not evaluated here.
/// Consumers must independently establish builtin arithmetic selection, and
/// ordinary source checking still owns all-arm typing and coverage.
pub fn select_anonymous_numeric_match_arm(
    program: &TypedTrees,
    dispatch: &typed_trees::expression::TableMatchExpression,
    mut builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Option<ExpressionHandle> {
    let subject = anonymous_numeric_value(program, dispatch.subject, &mut builtin)?;
    let arms = program.expression_table.match_arms(dispatch.arms);
    if arms.len() != dispatch.arms.len() {
        return None;
    }
    for arm in arms {
        match arm.pattern {
            typed_trees::expression::MatchPattern::Wildcard => return Some(arm.value),
            typed_trees::expression::MatchPattern::Value(pattern) => {
                let pattern = anonymous_numeric_value(program, pattern, &mut builtin)?;
                if subject.value.cmp_value(&pattern.value).is_eq() {
                    return Some(arm.value);
                }
            }
        }
    }
    None
}

fn integer_landing_warning(
    program: &TypedTrees,
    evaluated: &AnonymousNumericValue,
    integer: &BigInt,
    builtin: &mut impl FnMut(ExpressionHandle) -> bool,
) -> Option<Diagnostic> {
    if !evaluated.fractional_origin.is_valid() {
        return None;
    }
    let fractional = anonymous_numeric_value(program, evaluated.fractional_origin, builtin)?;
    Some(Diagnostic::warning(format!(
        "anonymous arithmetic preserves the exact fractional intermediate `{}` before landing as integer `{integer}`; type an operand if typed integer division was intended",
        fractional.value,
    )).with_source_span(program.expression_table.source_span(evaluated.fractional_origin)))
}

pub(crate) fn land_integer_value(
    value: &BigInt,
    destination: PrimitiveType,
) -> Option<IntegerLiteral> {
    let landed_type = match destination {
        PrimitiveType::I8 => LandedIntegerType::I8,
        PrimitiveType::I16 => LandedIntegerType::I16,
        PrimitiveType::I32 => LandedIntegerType::I32,
        PrimitiveType::I64 => LandedIntegerType::I64,
        PrimitiveType::U8 => LandedIntegerType::U8,
        PrimitiveType::U16 => LandedIntegerType::U16,
        PrimitiveType::U32 => LandedIntegerType::U32,
        PrimitiveType::U64 => LandedIntegerType::U64,
        // Address width and meaning require target authority, not a fixed
        // integer destination guessed from today's native representation.
        _ => return None,
    };
    let width = usize::try_from(landed_type.bit_width()).ok()?;
    let bound = BigInt::from_u64(1).shl_bits(width - usize::from(landed_type.is_signed()));
    let minimum = if landed_type.is_signed() {
        bound.negate()
    } else {
        BigInt::zero()
    };
    let maximum = bound.sub(&BigInt::from_u64(1));
    if value < &minimum || value > &maximum {
        return None;
    }
    Some(
        IntegerLiteral::from_parts(
            value.is_negative(),
            IntegerRadix::Decimal,
            &value.abs().to_string(),
        )
        .ok()?
        .with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    )
}

pub(crate) struct AnonymousNumericValue {
    pub(crate) value: BigRational,
    /// First authored fractional intermediate, retained even after cancellation.
    /// A zero handle means every intermediate remained integral.
    pub(crate) fractional_origin: ExpressionHandle,
}

pub(crate) fn anonymous_numeric_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    builtin: &mut impl FnMut(ExpressionHandle) -> bool,
) -> Option<AnonymousNumericValue> {
    evaluate_anonymous_value::<true>(program, expression, builtin)
}

/// Absence of a discovered type is not evidence of anonymity. Result joins
/// inherit a destination only when every leaf is an actual anonymous number;
/// named values, typed operations and suffixed floats remain conversions.
pub(crate) fn has_anonymous_numeric_results(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    fn visit(
        program: &TypedTrees,
        expression: ExpressionHandle,
        active: &mut Vec<ExpressionHandle>,
    ) -> bool {
        if !program.expression_table.expression_is_valid(expression) || active.contains(&expression)
        {
            return false;
        }
        if let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) {
            active.push(expression);
            let arms = program.expression_table.match_arms(dispatch.arms);
            let anonymous =
                !arms.is_empty() && arms.iter().all(|arm| visit(program, arm.value, active));
            active.pop();
            anonymous
        } else {
            anonymous_numeric_value(program, expression, &mut |expression| {
                has_anonymous_operator_meaning(program, expression)
            })
            .is_some()
        }
    }
    visit(program, expression, &mut Vec::new())
}

/// Float landing keeps its existing integer-spelling path separate from
/// decimal arithmetic, whose ExactFloat evaluator preserves signed-zero rules.
pub(super) fn anonymous_integer_literal_tree_value(
    program: &TypedTrees,
    expression: ExpressionHandle,
    builtin: &mut impl FnMut(ExpressionHandle) -> bool,
) -> Option<AnonymousNumericValue> {
    evaluate_anonymous_value::<false>(program, expression, builtin)
}

fn evaluate_anonymous_value<const ALLOW_DECIMAL_LITERALS: bool>(
    program: &TypedTrees,
    expression: ExpressionHandle,
    builtin: &mut impl FnMut(ExpressionHandle) -> bool,
) -> Option<AnonymousNumericValue> {
    enum Step {
        Enter(ExpressionHandle),
        Leave(ExpressionHandle),
        Binary(ExpressionHandle, BinaryOperator),
    }
    let mut pending = vec![Step::Enter(expression)];
    let mut active = Vec::new();
    let mut values: Vec<BigRational> = Vec::new();
    let mut fractional_origin = ExpressionHandle::invalid();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => {
                if !program.expression_table.expression_is_valid(expression)
                    || active.contains(&expression)
                {
                    return None;
                }
                match program.expression_table.expression(expression) {
                    ExpressionNode::Integer(literal) if literal.landing().is_none() => {
                        values.push(BigRational::from_integer(literal.value_bignum()?))
                    }
                    ExpressionNode::Float(literal)
                        if ALLOW_DECIMAL_LITERALS && literal.landing().is_none() =>
                    {
                        let value = BigRational::from_decimal_str(literal.text())?;
                        if !fractional_origin.is_valid() && value.to_integer_exact().is_none() {
                            fractional_origin = expression;
                        }
                        values.push(value);
                    }
                    ExpressionNode::Binary(binary) if builtin(expression) => {
                        active.push(expression);
                        pending.push(Step::Leave(expression));
                        pending.push(Step::Binary(expression, binary.operator));
                        pending.push(Step::Enter(binary.right));
                        pending.push(Step::Enter(binary.left));
                    }
                    _ => return None,
                }
            }
            Step::Leave(expression) => {
                if active.pop() != Some(expression) {
                    return None;
                }
            }
            Step::Binary(expression, operator) => {
                let right = values.pop()?;
                let left = values.pop()?;
                let value = match operator {
                    BinaryOperator::Add => left.add(&right),
                    BinaryOperator::Subtract => left.sub(&right),
                    BinaryOperator::Multiply => left.mul(&right),
                    BinaryOperator::Divide => left.div(&right)?,
                    _ => return None,
                };
                if !fractional_origin.is_valid() && value.to_integer_exact().is_none() {
                    fractional_origin = expression;
                }
                values.push(value);
            }
        }
    }
    (values.len() == 1).then(|| AnonymousNumericValue {
        value: values.pop().expect("one evaluated anonymous value"),
        fractional_origin,
    })
}

pub fn has_anonymous_operator_meaning(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return false;
    };
    use language_core::OperatorSpelling;
    let spelling = match binary.operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        _ => return false,
    };
    has_builtin_anonymous_operands(program, expression, spelling)
}

pub(super) fn has_builtin_anonymous_operands(
    program: &TypedTrees,
    expression: ExpressionHandle,
    spelling: language_core::OperatorSpelling,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    typed_trees::operator::resolve_spelling_for_operands(program, spelling, &[None, None])
        .is_empty()
        && program
            .expression_table
            .authored_selection_occurrences(expression)
            .all(|occurrence| {
                program
                    .authored_declaration_selections()
                    .get(occurrence)
                    .is_some_and(|selection| {
                        matches!(
                            selection.target(),
                            Target::Intrinsic(Intrinsic::BuiltinOperator)
                                | Target::LateBound(LateBinding::CheckedOperator)
                        )
                    })
            })
}
