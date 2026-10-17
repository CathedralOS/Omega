//! integers slice of scalar_lowering.

use super::element_selection::*;
use super::integer_widen_is_total;
use crate::values::scalar::expression_facts::{
    checked_integer_binary_kind, combine_arithmetic_domains, is_integer, scalar_expression_type,
};
use checked_trees::{CheckedIntegerRange, CheckedScalarExpression};
use numerics::arithmetic::ArithmeticDomain;
use numerics::literals::{IntegerLanding, LandedIntegerType};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::types::PrimitiveType;

/// An anonymous offset beside an address denotes a count, not another address.
/// Named operands keep their checked carrier, so address differences remain distinct.
pub(crate) fn subtraction_count_context(
    operator: BinaryOperator,
    peer: PrimitiveType,
) -> PrimitiveType {
    if operator == BinaryOperator::Subtract && peer == PrimitiveType::Addr {
        PrimitiveType::U64
    } else {
        peer
    }
}

/// Construct one selected builtin operation from operands whose evaluation
/// order has already been retained by either the pure tree or computation plan.
pub(crate) fn construct_integer_binary(
    operator: BinaryOperator,
    mut left: CheckedScalarExpression,
    left_domain: ArithmeticDomain,
    mut right: CheckedScalarExpression,
    right_domain: ArithmeticDomain,
    formation_decided: bool,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let shift = matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    let domain = if shift {
        left_domain
    } else {
        combine_arithmetic_domains(left_domain, right_domain)?
    };
    let kind = checked_integer_binary_kind(operator, domain)?;
    let mut left_type = scalar_expression_type(&left);
    let mut right_type = scalar_expression_type(&right);
    // Unary `-value` is parsed as a compiler-generated anonymous
    // `0 - value`. That zero has no parse-site suffix from which the
    // ordinary literal stamper can learn a carrier, so retain the
    // binary expression's already-checked operand carrier here. This
    // is contextual literal landing, not a new negation meaning.
    if operator == BinaryOperator::Subtract && left_type.is_none() {
        left = land_anonymous_zero(left, right_type?)?;
        left_type = scalar_expression_type(&left);
    }
    if left_type.is_none() {
        left = land_contextual_integer_literal(left, right_type?)?;
        left_type = scalar_expression_type(&left);
    }
    if right_type.is_none() {
        right = land_contextual_integer_literal(
            right,
            subtraction_count_context(operator, left_type?),
        )?;
        right_type = scalar_expression_type(&right);
    }
    let left_type = left_type?;
    let right_type = right_type?;
    let primitive_type = if operator == BinaryOperator::Subtract && left_type == PrimitiveType::Addr
    {
        if domain != ArithmeticDomain::Exact {
            return None;
        }
        match right_type {
            PrimitiveType::Addr => PrimitiveType::U64,
            PrimitiveType::U64 => PrimitiveType::Addr,
            _ => return None,
        }
    } else {
        if !shift && right_type != left_type {
            return None;
        }
        left_type
    };
    if !is_integer(primitive_type) || !is_integer(right_type) {
        return None;
    }
    Some((
        CheckedScalarExpression::IntegerBinary {
            kind,
            primitive_type,
            left: Box::new(left),
            right: Box::new(right),
            formation_decided,
        },
        domain,
    ))
}

pub(crate) fn construct_integer_bitwise_not(
    operand: CheckedScalarExpression,
    domain: ArithmeticDomain,
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let primitive_type = scalar_expression_type(&operand)?;
    is_integer(primitive_type).then_some((
        CheckedScalarExpression::IntegerBitwiseNot {
            primitive_type,
            operand: Box::new(operand),
        },
        domain,
    ))
}

/// Retain the exact carrier conversion at its original source occurrence,
/// independently of the policy qualifying later operations on the result.
/// Completing the operand through a computation plan preserves that proof.
pub(crate) fn construct_integer_cast(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
    operand: CheckedScalarExpression,
    exact_integer_casts: &[validation::ExactIntegerCastFact],
) -> Option<(CheckedScalarExpression, ArithmeticDomain)> {
    let cast = program.expression_table.cast_node(expression)?;
    if cast.form.is_recast() || !cast.semantic_domain.is_empty() {
        return None;
    }
    let target_type = program.primitive_type_reference(cast.target_type)?;
    if !is_integer(target_type) {
        return None;
    }
    let source_type = scalar_expression_type(&operand);
    if source_type.is_none() {
        // Anonymous literals land once at the cast destination, including the
        // distinct address carrier. Contextual landing checks the value; a
        // typed conversion below still requires its own exact-cast evidence.
        return land_contextual_integer_literal(operand, target_type)
            .map(|literal| (literal, cast.domain));
    }
    let source_type = source_type?;
    if source_type == target_type {
        return Some((operand, cast.domain));
    }
    // A compile-known exact conversion does not need a runtime cast
    // operation or a carried flow assumption: validation has already
    // proved the spelling denotes a target value, and the checked
    // carrier can retain that value directly at its new landing. Keep
    // address conversions out of this fixed-integer slice; addr is a
    // distinct carrier even when its current representation is u64.
    if source_type != PrimitiveType::Addr
        && target_type != PrimitiveType::Addr
        && let Some(literal) = retag_exact_integer_literal(&operand, target_type)
    {
        return Some((literal, cast.domain));
    }
    // A full-carrier inclusion needs no occurrence proof. Preserve it
    // as widening even when validation also retained a bounded range
    // for this spelling; exact-cast obligations are only necessary for
    // partial fixed-integer conversions.
    if integer_widen_is_total(source_type, target_type) {
        return Some((
            CheckedScalarExpression::IntegerWiden {
                primitive_type: target_type,
                operand: Box::new(operand),
            },
            cast.domain,
        ));
    }
    if let Some(fact) = exact_integer_casts
        .iter()
        .find(|fact| fact.expression == expression)
        && fact.source_type == source_type
        && fact.target_type == target_type
    {
        return Some((
            CheckedScalarExpression::IntegerExactCast {
                primitive_type: target_type,
                operand: Box::new(operand),
                range: CheckedIntegerRange {
                    minimum: fact.minimum.clone(),
                    maximum: fact.maximum.clone(),
                },
                formation_decided: true,
            },
            cast.domain,
        ));
    }
    // All remaining cast shapes fail closed at this source-independent
    // boundary: no total conversion and no retained occurrence proof.
    None
}

/// Reclose the one source-level slice view whose length is already fixed by
/// two literal bounds. This is a value fact, not a backend fold: retaining the
/// landed `u64` here lets later checked control replay the exact initializer
/// without consulting a target representation.
pub(crate) fn exact_inline_literal_subslice_length(
    program: &dyn ::validation::CheckingInput,
    expression: ExpressionHandle,
) -> Option<CheckedScalarExpression> {
    let receiver = program
        .expression_table
        .authored_length_receiver(expression)?;
    let indexed = program.expression_table.indexed_node(receiver)?;
    let range = program.expression_table.range_node(indexed.index)?;
    if !range.start.is_valid() || !range.end.is_valid() {
        return None;
    }
    let start = program.expression_table.integer_node(range.start)?;
    let end = program.expression_table.integer_node(range.end)?;
    let length = end
        .value_u64()?
        .checked_add(u64::from(range.end_inclusive))?
        .checked_sub(start.value_u64()?)?;
    let length = i64::try_from(length).ok()?;
    Some(CheckedScalarExpression::IntegerLiteral {
        literal: numerics::literals::IntegerLiteral::from_value(length).with_landing(
            IntegerLanding {
                landed_type: LandedIntegerType::U64,
                domain: ArithmeticDomain::Exact,
            },
        ),
    })
}

pub(crate) fn land_anonymous_zero(
    expression: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    if literal.landing().is_some() || literal.value_i64() != Some(0) {
        return None;
    }
    let landed_type = primitive_type.landed_landing()?;
    Some(CheckedScalarExpression::IntegerLiteral {
        literal: literal.with_landing(IntegerLanding {
            landed_type,
            domain: ArithmeticDomain::Exact,
        }),
    })
}

pub(crate) fn retag_exact_integer_literal(
    expression: &CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    let landed_type = landed_for_primitive(primitive_type)?;
    landed_integer_literal_fits(literal, landed_type)
}

pub(crate) fn land_contextual_integer_literal(
    expression: CheckedScalarExpression,
    primitive_type: PrimitiveType,
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    if literal.landing().is_some() {
        return None;
    }
    let landed_type = match primitive_type {
        PrimitiveType::Addr => LandedIntegerType::Addr,
        primitive_type => landed_for_primitive(primitive_type)?,
    };
    landed_integer_literal_fits(&literal, landed_type)
}

pub(crate) fn landed_for_primitive(primitive_type: PrimitiveType) -> Option<LandedIntegerType> {
    primitive_type.landed_landing_fixed()
}
