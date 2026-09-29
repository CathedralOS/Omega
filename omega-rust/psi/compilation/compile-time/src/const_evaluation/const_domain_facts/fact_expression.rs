use typed_trees::TypedTrees;
use typed_trees::data::{DataDefinition, DataField, DataMember};
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode, UnaryOperator};
use typed_trees::types::PrimitiveType;

#[derive(Clone, Copy)]
pub(super) enum ConstProofValue {
    Integer(i64),
    Boolean(bool),
}

#[derive(Clone, Copy)]
enum FactBindings<'a> {
    DomainSelf(i64),
    StaticData(&'a DataDefinition),
    ZeroStorage(&'a DataDefinition),
}

pub(super) fn data_fact_is_static_true(
    typed: &TypedTrees,
    data: &DataDefinition,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        evaluate_fact_expression(typed, expression, FactBindings::StaticData(data)),
        Ok(Some(ConstProofValue::Boolean(true)))
    )
}

/// Recheck the still-authored field predicates after a static membership has
/// discharged. Only exact scalar field bindings, constants and builtin Boolean
/// comparisons can establish zero here. Arithmetic needs its declared-width
/// evaluation, and unknown names or operators remain gated.
pub(super) fn data_fact_holds_at_zero(
    typed: &TypedTrees,
    data: &DataDefinition,
    expression: ExpressionHandle,
) -> bool {
    matches!(
        evaluate_fact_expression(typed, expression, FactBindings::ZeroStorage(data)),
        Ok(Some(ConstProofValue::Boolean(true)))
    )
}

pub(super) fn evaluate_domain_fact_expression(
    typed: &TypedTrees,
    expression: ExpressionHandle,
    self_value: i64,
) -> Result<Option<ConstProofValue>, String> {
    evaluate_fact_expression(typed, expression, FactBindings::DomainSelf(self_value))
}

fn evaluate_fact_expression(
    typed: &TypedTrees,
    expression: ExpressionHandle,
    bindings: FactBindings<'_>,
) -> Result<Option<ConstProofValue>, String> {
    match typed.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => value
            .value_i64()
            .map(ConstProofValue::Integer)
            .map(Some)
            .ok_or_else(|| {
                format!(
                    "proof operand `{value}` does not fit the build-time evaluator's signed integer boundary"
                )
            }),
        ExpressionNode::Boolean(value) => Ok(Some(ConstProofValue::Boolean(*value))),
        ExpressionNode::Name(path) => {
            if matches!(bindings, FactBindings::StaticData(_)) {
                return Ok(None);
            }
            if let FactBindings::ZeroStorage(data) = bindings {
                return Ok(zero_storage_field(typed, data, expression).and_then(|field| {
                    match typed.primitive_type_reference(field.type_reference)? {
                        PrimitiveType::Bool => Some(ConstProofValue::Boolean(false)),
                        PrimitiveType::I8 | PrimitiveType::I16 | PrimitiveType::I32
                        | PrimitiveType::I64 | PrimitiveType::U8 | PrimitiveType::U16
                        | PrimitiveType::U32 | PrimitiveType::U64 => Some(ConstProofValue::Integer(0)),
                        _ => None,
                    }
                }));
            }
            let FactBindings::DomainSelf(self_value) = bindings else {
                return Ok(None);
            };
            let [name] = typed.expression_table.name_path_members(path.members) else {
                return Ok(None);
            };
            Ok((name.is_self_receiver())
                .then_some(ConstProofValue::Integer(self_value)))
        }
        ExpressionNode::Binary(binary) => {
            if let FactBindings::ZeroStorage(data) | FactBindings::StaticData(data) = bindings
                && !zero_storage_binary_is_builtin(typed, data, expression, binary)
            {
                return Ok(None);
            }
            let Some(left) = evaluate_fact_expression(typed, binary.left, bindings)? else {
                return Ok(None);
            };
            let Some(right) = evaluate_fact_expression(typed, binary.right, bindings)?
            else {
                return Ok(None);
            };
            evaluate_domain_fact_binary(binary.operator, left, right).map(Some)
        }
        ExpressionNode::Unary(unary) => {
            if matches!(bindings, FactBindings::ZeroStorage(_) | FactBindings::StaticData(_))
                && unary.operator != UnaryOperator::LogicalNot
            {
                return Ok(None);
            }
            let Some(operand) = evaluate_fact_expression(typed, unary.operand, bindings)?
            else {
                return Ok(None);
            };
            match (unary.operator, operand) {
                (UnaryOperator::BitwiseNot, ConstProofValue::Integer(value)) => {
                    Ok(Some(ConstProofValue::Integer(!value)))
                }
                (UnaryOperator::BitwiseNot, ConstProofValue::Boolean(_)) => {
                    Err("bitwise complement requires an integer proof operand".to_string())
                }
                (UnaryOperator::LogicalNot, ConstProofValue::Boolean(value)) => {
                    Ok(Some(ConstProofValue::Boolean(!value)))
                }
                (UnaryOperator::LogicalNot, ConstProofValue::Integer(_)) => {
                    Err("logical negation requires a boolean proof operand".to_string())
                }
            }
        }
        _ => Ok(None),
    }
}

fn zero_storage_field<'program>(
    typed: &'program TypedTrees,
    data: &DataDefinition,
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    let ExpressionNode::Name(path) = typed.expression_table.expression(expression) else {
        return None;
    };
    if path.members.count() != 1 || !path.symbol.is_valid() || path.head_symbol != path.symbol {
        return None;
    }
    typed
        .data_members(data)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field)
                if field.symbol == path.symbol && !field.relevance.is_erased() =>
            {
                Some(field)
            }
            _ => None,
        })
}

fn zero_storage_binary_is_builtin(
    typed: &TypedTrees,
    data: &DataDefinition,
    expression: ExpressionHandle,
    binary: &typed_trees::expression::TableBinaryExpression,
) -> bool {
    use language_core::OperatorSpelling;
    let spelling = match binary.operator {
        BinaryOperator::And | BinaryOperator::Or => return true,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        _ => return false,
    };
    let operands = [binary.left, binary.right];
    // Two anonymous literals have no operator home. A named/landed operand
    // instead owes the normal source-owned operator selection, including
    // overloads elsewhere in the program that really match its carrier.
    if operands.iter().all(|operand| {
        matches!(typed.expression_table.expression(*operand),
        ExpressionNode::Integer(literal) if literal.landing().is_none())
    }) {
        return true;
    }
    let operand_types = operands.map(|operand| {
        zero_storage_field(typed, data, operand)
            .map(|field| field.type_reference)
            .or_else(|| validation::landed_integer_literal_type_reference(typed, operand))
    });
    typed_trees::operator::has_builtin_spelled_expression_meaning(
        typed,
        data.symbol,
        expression,
        spelling,
        &operand_types,
    )
}

fn evaluate_domain_fact_binary(
    operator: BinaryOperator,
    left: ConstProofValue,
    right: ConstProofValue,
) -> Result<ConstProofValue, String> {
    use BinaryOperator::*;
    match (left, right) {
        (ConstProofValue::Integer(left), ConstProofValue::Integer(right)) => match operator {
            Add => left
                .checked_add(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof addition overflows `i64`".to_string()),
            Subtract => left
                .checked_sub(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof subtraction overflows `i64`".to_string()),
            Multiply => left
                .checked_mul(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof multiplication overflows `i64`".to_string()),
            Divide => left
                .checked_div(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof division is invalid".to_string()),
            Modulo => left
                .checked_rem(right)
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof remainder is invalid".to_string()),
            ShiftLeft => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shl(amount))
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof left shift exceeds the `i64` width".to_string()),
            ShiftRight => u32::try_from(right)
                .ok()
                .and_then(|amount| left.checked_shr(amount))
                .map(ConstProofValue::Integer)
                .ok_or_else(|| "proof right shift exceeds the `i64` width".to_string()),
            BitwiseAnd => Ok(ConstProofValue::Integer(left & right)),
            BitwiseOr => Ok(ConstProofValue::Integer(left | right)),
            BitwiseXor => Ok(ConstProofValue::Integer(left ^ right)),
            Equal => Ok(ConstProofValue::Boolean(left == right)),
            NotEqual => Ok(ConstProofValue::Boolean(left != right)),
            Greater => Ok(ConstProofValue::Boolean(left > right)),
            GreaterOrEqual => Ok(ConstProofValue::Boolean(left >= right)),
            Less => Ok(ConstProofValue::Boolean(left < right)),
            LessOrEqual => Ok(ConstProofValue::Boolean(left <= right)),
            And | Or => Err("logical proof operators require boolean operands".to_string()),
            CaseMembership => {
                Err("case membership requires a nominal subject and case classifier".to_string())
            }
        },
        (ConstProofValue::Boolean(left), ConstProofValue::Boolean(right)) => match operator {
            And => Ok(ConstProofValue::Boolean(left && right)),
            Or => Ok(ConstProofValue::Boolean(left || right)),
            Equal => Ok(ConstProofValue::Boolean(left == right)),
            NotEqual => Ok(ConstProofValue::Boolean(left != right)),
            _ => Err("arithmetic proof operators require integer operands".to_string()),
        },
        _ => Err("const proof operands have incompatible types".to_string()),
    }
}
