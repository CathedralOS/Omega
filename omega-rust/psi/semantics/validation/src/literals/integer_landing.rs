//! Chapter 5 anonymous integer arithmetic: exact values before one landing.
//! The authored expression remains intact; consumers retain the rendered value
//! at its actual destination, not widths on the anonymous intermediate nodes.

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

/// Render a wholly anonymous fixed-integer expression at its first typed
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
