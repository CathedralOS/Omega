//! Exact scalar evaluation after declaration selection.
//!
//! Boolean leaves and comparison results share the same value path so nested
//! equality keeps its operands' meaning. Each binary node is admitted against
//! its exact selected operator before evaluation; equal canonical results do
//! not replace the separate authored selection custody retained by the probe.
//! Equality is strict, so both operands use the existing left-to-right traversal.
//! Boolean logic defers its right operand until the left result selects it, as
//! required by the language expression schedule. The enclosing probe already
//! checked selection and retained custody across both operands: skipping execution
//! cannot hide a runtime name or unauthorized operator in the unselected branch.
//! A separate shape pass establishes types and complete anonymous-rational
//! landings across both operands before selective evaluation runs landed operations.

use diagnostics::Diagnostic;
use language_semantics::const_value::{CanonicalConstIdentity, CanonicalConstValue};
use numerics::{
    arithmetic::ArithmeticDomain,
    literals::{IntegerLiteral, LandedIntegerType},
};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
    machine::Machine,
    state::State,
    types::PrimitiveType,
};

#[derive(Clone, Copy)]
enum Value {
    Anonymous(ExpressionHandle),
    Boolean(bool),
    Landed(LandedIntegerType, IntegerValue),
}

/// Only a wholly anonymous final value uses `destination`. A previously
/// landed result retains its carrier for the caller's destination check.
pub(super) fn evaluate(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: PrimitiveType,
) -> Result<(CanonicalConstValue, Vec<Diagnostic>), String> {
    if !program.expression_table.expression_is_valid(expression) {
        return Err("invalid constant expression".to_owned());
    }
    enum Step {
        Enter(ExpressionHandle),
        Binary(ExpressionHandle, BinaryOperator),
        LogicalLeft(ExpressionHandle, BinaryOperator, ExpressionHandle),
        LogicalRight(ExpressionHandle),
    }
    let mut pending = vec![Step::Enter(expression)];
    let mut active = Vec::new();
    let mut values = Vec::new();
    let mut warnings = validate_shapes(program, machine, state, expression)?;
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => {
                if !program.expression_table.expression_is_valid(expression)
                    || active.contains(&expression)
                {
                    return Err("invalid or cyclic constant expression".into());
                }
                match program.expression_table.expression(expression) {
                    ExpressionNode::Boolean(value) => values.push(Value::Boolean(*value)),
                    ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                        values.push(landed_literal(literal)?);
                    }
                    ExpressionNode::Integer(_) => values.push(Value::Anonymous(expression)),
                    ExpressionNode::Float(literal) if literal.landing().is_none() => {
                        values.push(Value::Anonymous(expression));
                    }
                    ExpressionNode::Binary(binary) => {
                        if !validation::has_builtin_binary_expression_meaning(
                            program,
                            machine,
                            Some(state),
                            expression,
                        ) {
                            return Err(
                                "constant expression has no selected builtin operator meaning"
                                    .into(),
                            );
                        }
                        active.push(expression);
                        if matches!(binary.operator, BinaryOperator::And | BinaryOperator::Or) {
                            pending.push(Step::LogicalLeft(
                                expression,
                                binary.operator,
                                binary.right,
                            ));
                        } else {
                            pending.push(Step::Binary(expression, binary.operator));
                            pending.push(Step::Enter(binary.right));
                        }
                        pending.push(Step::Enter(binary.left));
                    }
                    _ => return Err("unsupported node in exact integer constant expression".into()),
                }
            }
            Step::LogicalLeft(expression, operator, right) => {
                let Some(Value::Boolean(left)) = values.pop() else {
                    return Err("Boolean logic requires a Boolean left operand".into());
                };
                if left == (operator == BinaryOperator::And) {
                    pending.push(Step::LogicalRight(expression));
                    pending.push(Step::Enter(right));
                } else {
                    if active.pop() != Some(expression) {
                        return Err("invalid constant expression traversal".into());
                    }
                    values.push(Value::Boolean(left));
                }
            }
            Step::LogicalRight(expression) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant expression traversal".into());
                }
                if !matches!(values.last(), Some(Value::Boolean(_))) {
                    return Err("Boolean logic requires a Boolean right operand".into());
                }
            }
            Step::Binary(expression, operator) => {
                if active.pop() != Some(expression) {
                    return Err("invalid constant expression traversal".into());
                }
                let right = values.pop().ok_or("missing right constant operand")?;
                let left = values.pop().ok_or("missing left constant operand")?;
                let value = match (left, right) {
                    (Value::Anonymous(_), Value::Anonymous(_)) => {
                        if !matches!(
                            operator,
                            BinaryOperator::Add
                                | BinaryOperator::Subtract
                                | BinaryOperator::Multiply
                                | BinaryOperator::Divide
                        ) {
                            return Err(
                                "anonymous constant operation requires a selected fixed carrier"
                                    .into(),
                            );
                        }
                        Value::Anonymous(expression)
                    }
                    (Value::Landed(carrier, left), Value::Anonymous(right)) => {
                        let right_destination = if matches!(
                            operator,
                            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                        ) {
                            PrimitiveType::U64
                        } else {
                            primitive(carrier)?
                        };
                        let right = land_anonymous(
                            program,
                            machine,
                            state,
                            right,
                            right_destination,
                            &mut warnings,
                        )?;
                        apply(operator, Value::Landed(carrier, left), right)?
                    }
                    (Value::Anonymous(left), Value::Landed(carrier, right)) => {
                        if matches!(
                            operator,
                            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
                        ) {
                            return Err("shift value requires a selected fixed carrier".into());
                        }
                        let left = land_anonymous(
                            program,
                            machine,
                            state,
                            left,
                            primitive(carrier)?,
                            &mut warnings,
                        )?;
                        apply(operator, left, Value::Landed(carrier, right))?
                    }
                    (left, right) => apply(operator, left, right)?,
                };
                values.push(value);
            }
        }
    }
    if values.len() != 1 {
        return Err("constant expression did not produce one value".into());
    }
    let value = match values.pop().ok_or("missing constant expression value")? {
        Value::Anonymous(expression) => land_anonymous(
            program,
            machine,
            state,
            expression,
            destination,
            &mut warnings,
        )?,
        value => value,
    };
    if let Value::Boolean(value) = value {
        return Ok((CanonicalConstValue::boolean(value), warnings));
    }
    let Value::Landed(carrier, value) = value else {
        return Err("constant expression has no integer landing".into());
    };
    let value = match value {
        IntegerValue::Signed(value) => value,
        IntegerValue::Unsigned(value) => i128::try_from(value)
            .map_err(|_| "constant value exceeds canonical integer encoding")?,
    };
    let identity = CanonicalConstIdentity::integer(carrier.name(), value);
    Ok((
        CanonicalConstValue::new(identity.type_name, identity.encoding, value.to_string()),
        warnings,
    ))
}

// Typed lowering and the ordinary partial operand validators do not establish
// exact scalar shapes after constant substitution. Visit all operands to retain
// static type and anonymous-rational landing obligations, even in skipped arms.
// Only anonymous trees are evaluated here: numeric_values.md requires their
// compile-time landing before a typed operation and supplies no runtime rational
// arithmetic. Landed operations are never executed by this pass, so a skipped
// divide, overflow or shift still follows the selective expression schedule.
fn validate_shapes(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    root: ExpressionHandle,
) -> Result<Vec<Diagnostic>, String> {
    #[derive(Clone, Copy)]
    enum Shape {
        Anonymous(ExpressionHandle),
        Boolean,
        Integer(LandedIntegerType),
    }
    let mut pending = vec![(root, false)];
    let mut active = Vec::new();
    let mut shapes = Vec::new();
    let mut warnings = Vec::new();
    while let Some((expression, finish)) = pending.pop() {
        if !program.expression_table.expression_is_valid(expression) {
            return Err("invalid constant expression".into());
        }
        if !finish {
            if active.contains(&expression) {
                return Err("cyclic constant expression".into());
            }
            match program.expression_table.expression(expression) {
                ExpressionNode::Boolean(_) => shapes.push(Shape::Boolean),
                ExpressionNode::Integer(literal) if literal.landing().is_some() => {
                    let Value::Landed(carrier, _) = landed_literal(literal)? else {
                        unreachable!()
                    };
                    shapes.push(Shape::Integer(carrier));
                }
                ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                    shapes.push(Shape::Anonymous(expression))
                }
                ExpressionNode::Binary(binary) => {
                    if !validation::has_builtin_binary_expression_meaning(
                        program,
                        machine,
                        Some(state),
                        expression,
                    ) {
                        return Err(
                            "constant expression has no selected builtin operator meaning".into(),
                        );
                    }
                    active.push(expression);
                    pending.push((expression, true));
                    pending.push((binary.right, false));
                    pending.push((binary.left, false));
                }
                _ => return Err("unsupported node in exact scalar constant expression".into()),
            }
            continue;
        }
        if active.pop() != Some(expression) {
            return Err("invalid constant expression traversal".into());
        }
        let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
            unreachable!()
        };
        let operator = binary.operator;
        let right = shapes.pop().ok_or("missing right constant operand type")?;
        let left = shapes.pop().ok_or("missing left constant operand type")?;
        let shift = matches!(
            operator,
            BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
        );
        let (left, right) = match (left, right) {
            (Shape::Anonymous(_), Shape::Anonymous(_)) => {
                if !matches!(
                    operator,
                    BinaryOperator::Add
                        | BinaryOperator::Subtract
                        | BinaryOperator::Multiply
                        | BinaryOperator::Divide
                ) {
                    return Err(
                        "anonymous constant operation requires a selected fixed carrier".into(),
                    );
                }
                shapes.push(Shape::Anonymous(expression));
                continue;
            }
            (Shape::Integer(carrier), Shape::Anonymous(right)) => {
                let destination = if shift {
                    PrimitiveType::U64
                } else {
                    primitive(carrier)?
                };
                let Value::Landed(right_carrier, _) =
                    land_anonymous(program, machine, state, right, destination, &mut warnings)?
                else {
                    unreachable!()
                };
                (Shape::Integer(carrier), Shape::Integer(right_carrier))
            }
            (Shape::Anonymous(left), Shape::Integer(carrier)) => {
                if shift {
                    return Err("shift value requires a selected fixed carrier".into());
                }
                land_anonymous(
                    program,
                    machine,
                    state,
                    left,
                    primitive(carrier)?,
                    &mut warnings,
                )?;
                (Shape::Integer(carrier), Shape::Integer(carrier))
            }
            operands => operands,
        };
        let result = match (left, right) {
            (Shape::Boolean, Shape::Boolean)
                if matches!(
                    operator,
                    BinaryOperator::And
                        | BinaryOperator::Or
                        | BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                ) =>
            {
                Shape::Boolean
            }
            (Shape::Integer(left), Shape::Integer(right)) => {
                if matches!(operator, BinaryOperator::And | BinaryOperator::Or) {
                    return Err("Boolean logic requires Boolean operands".into());
                }
                if !shift && left != right {
                    return Err(
                        "constant operands have incompatible landed integer carriers".into(),
                    );
                }
                if matches!(
                    operator,
                    BinaryOperator::Equal
                        | BinaryOperator::NotEqual
                        | BinaryOperator::Less
                        | BinaryOperator::LessOrEqual
                        | BinaryOperator::Greater
                        | BinaryOperator::GreaterOrEqual
                ) {
                    Shape::Boolean
                } else {
                    Shape::Integer(left)
                }
            }
            _ => return Err("constant operator has incompatible operand types".into()),
        };
        shapes.push(result);
    }
    if shapes.len() != 1 {
        return Err("constant expression did not produce one type".into());
    }
    Ok(warnings)
}

fn land_anonymous(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    destination: PrimitiveType,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Value, String> {
    let (literal, warning) = validation::land_anonymous_integer_expression_with_warning(
        program,
        expression,
        destination,
        |expression| {
            validation::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            )
        },
    )
    .ok_or("anonymous constant expression cannot land exactly at the selected integer carrier")?;
    if let Some(warning) = warning
        && !warnings.contains(&warning)
    {
        warnings.push(warning);
    }
    landed_literal(&literal)
}

fn landed_literal(literal: &IntegerLiteral) -> Result<Value, String> {
    let landing = literal.landing().ok_or("missing integer landing")?;
    if landing.domain != ArithmeticDomain::Exact {
        return Err("constant generic arithmetic requires Exact integer policy".into());
    }
    let carrier = integer_type(landing.landed_type)?;
    let value = if landing.landed_type.is_signed() {
        IntegerValue::Signed(i128::from(
            literal
                .value_i64()
                .ok_or("invalid signed constant literal")?,
        ))
    } else {
        IntegerValue::Unsigned(u128::from(
            literal
                .value_u64()
                .ok_or("invalid unsigned constant literal")?,
        ))
    };
    if !carrier.admits(value) {
        return Err("constant literal is outside its selected integer carrier".into());
    }
    Ok(Value::Landed(landing.landed_type, value))
}

fn apply(operator: BinaryOperator, left: Value, right: Value) -> Result<Value, String> {
    if let (Value::Boolean(left), Value::Boolean(right)) = (left, right) {
        return match operator {
            BinaryOperator::Equal => Ok(Value::Boolean(left == right)),
            BinaryOperator::NotEqual => Ok(Value::Boolean(left != right)),
            _ => Err("unsupported builtin Boolean constant operator".into()),
        };
    }
    let (Value::Landed(left_carrier, left), Value::Landed(right_carrier, right)) = (left, right)
    else {
        return Err("integer operation requires landed operands".into());
    };
    let integer = integer_type(left_carrier)?;
    let shifts = matches!(
        operator,
        BinaryOperator::ShiftLeft | BinaryOperator::ShiftRight
    );
    if !shifts && left_carrier != right_carrier {
        return Err("constant operands have incompatible landed integer carriers".into());
    }
    if matches!(
        operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
    ) {
        let ordering = integer
            .compare(left, right)
            .ok_or("constant comparison operands are outside their selected integer carrier")?;
        let result = match operator {
            BinaryOperator::Equal => ordering.is_eq(),
            BinaryOperator::NotEqual => ordering.is_ne(),
            BinaryOperator::Less => ordering.is_lt(),
            BinaryOperator::LessOrEqual => ordering.is_le(),
            BinaryOperator::Greater => ordering.is_gt(),
            BinaryOperator::GreaterOrEqual => ordering.is_ge(),
            _ => unreachable!(),
        };
        return Ok(Value::Boolean(result));
    }
    let result = match operator {
        BinaryOperator::Add => integer.exact_add(left, right),
        BinaryOperator::Subtract => integer.exact_sub(left, right),
        BinaryOperator::Multiply => integer.exact_mul(left, right),
        BinaryOperator::Divide => integer.exact_div(left, right),
        BinaryOperator::Modulo => integer.exact_rem(left, right),
        BinaryOperator::BitwiseAnd => integer.bitwise_and(left, right),
        BinaryOperator::BitwiseOr => integer.bitwise_or(left, right),
        BinaryOperator::BitwiseXor => integer.bitwise_xor(left, right),
        BinaryOperator::ShiftLeft => integer.exact_shift_left(left, integer_type(right_carrier)?, right),
        BinaryOperator::ShiftRight => integer.exact_shift_right(left, integer_type(right_carrier)?, right),
        _ => return Err("unsupported builtin integer constant operator".into()),
    }.ok_or("Exact integer constant operation overflows, divides by zero, or has an invalid shift count")?;
    Ok(Value::Landed(left_carrier, result))
}

fn integer_type(carrier: LandedIntegerType) -> Result<IntegerType, String> {
    if carrier == LandedIntegerType::Addr {
        return Err("address constant evaluation requires target authority".into());
    }
    let sign = if carrier.is_signed() {
        IntegerSign::Signed
    } else {
        IntegerSign::Unsigned
    };
    IntegerType::new(sign, carrier.bit_width() as u16).map_err(|_| "invalid integer carrier".into())
}

fn primitive(carrier: LandedIntegerType) -> Result<PrimitiveType, String> {
    Ok(match carrier {
        LandedIntegerType::I8 => PrimitiveType::I8,
        LandedIntegerType::I16 => PrimitiveType::I16,
        LandedIntegerType::I32 => PrimitiveType::I32,
        LandedIntegerType::I64 => PrimitiveType::I64,
        LandedIntegerType::U8 => PrimitiveType::U8,
        LandedIntegerType::U16 => PrimitiveType::U16,
        LandedIntegerType::U32 => PrimitiveType::U32,
        LandedIntegerType::U64 => PrimitiveType::U64,
        LandedIntegerType::Addr => {
            return Err("address constant evaluation requires target authority".into());
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{Value, apply, landed_literal};
    use numerics::{
        arithmetic::ArithmeticDomain,
        literals::{IntegerLanding, IntegerLiteral, IntegerRadix, LandedIntegerType},
    };
    use semantic_vocabulary::IntegerValue;
    use typed_trees::expression::BinaryOperator;

    #[test]
    fn exact_nodes_reject_overflow_before_later_cancellation() {
        let maximum = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(255));
        let one = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(1));
        assert!(apply(BinaryOperator::Add, maximum, one).is_err());
    }

    #[test]
    fn remainder_rejects_signed_minimum_divided_by_negative_one() {
        let minimum = Value::Landed(LandedIntegerType::I8, IntegerValue::Signed(-128));
        let negative_one = Value::Landed(LandedIntegerType::I8, IntegerValue::Signed(-1));
        assert!(apply(BinaryOperator::Modulo, minimum, negative_one).is_err());
    }

    #[test]
    fn shift_uses_value_width_and_independent_count_carrier() {
        let one = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(1));
        let invalid_count = Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(8));
        assert!(apply(BinaryOperator::ShiftLeft, one, invalid_count).is_err());
        let count = Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(7));
        assert!(matches!(
            apply(BinaryOperator::ShiftLeft, one, count),
            Ok(Value::Landed(
                LandedIntegerType::U8,
                IntegerValue::Unsigned(128)
            ))
        ));
    }

    #[test]
    fn arithmetic_never_relands_incompatible_carriers() {
        let left = Value::Landed(LandedIntegerType::U8, IntegerValue::Unsigned(1));
        let right = Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(1));
        assert!(apply(BinaryOperator::Add, left, right).is_err());
    }

    #[test]
    fn unsigned_literal_preserves_values_above_signed_maximum() {
        let literal =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, "18446744073709551615")
                .expect("u64 maximum literal")
                .with_landing(IntegerLanding {
                    landed_type: LandedIntegerType::U64,
                    domain: ArithmeticDomain::Exact,
                });
        assert!(matches!(
            landed_literal(&literal),
            Ok(Value::Landed(LandedIntegerType::U64, IntegerValue::Unsigned(value)))
                if value == u128::from(u64::MAX)
        ));
    }
}
