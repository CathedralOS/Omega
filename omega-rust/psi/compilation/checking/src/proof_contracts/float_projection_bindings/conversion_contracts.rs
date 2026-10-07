//! Read a selected callable's IEEE conversion guarantee, not its leaf name.
//!
//! The result projection, source parameter and format constant must join the
//! same ensures equality through the sealed float catalogs. No conversion
//! meaning comes from a destination annotation or a callable named `from_f64`.
//! Float and integer inputs share the same result/format/parameter join. The
//! integer semantic family has eight carriers, so its proof-position overload
//! is selected by that exact parameter kind before checking sealed custody.

use typed_trees::TypedTrees;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::OperatorDefinition;
use typed_trees::signature::SignatureContractKind;
use typed_trees::types::PrimitiveType;

/// Return the carriers of a one-argument callable whose normal-return
/// contract states a sealed format conversion. Admission of the call and
/// proof of that contract remain with their ordinary owners.
pub fn float_format_conversion_contract(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> Option<(PrimitiveType, PrimitiveType)> {
    numeric_float_conversion_contract(program, operator)
        .filter(|(source, _)| matches!(source, PrimitiveType::F32 | PrimitiveType::F64))
}

/// A sealed conversion to a float carrier, from either a float projection or
/// the selected integer parameter. The declaration name selects no semantics.
pub fn numeric_float_conversion_contract(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> Option<(PrimitiveType, PrimitiveType)> {
    use numerics::float_semantics_catalog::{FloatSemanticKernel, FloatSemanticValueKind};
    let [parameter] = program.operator_parameters(operator) else {
        return None;
    };
    let source = program.primitive_type_reference(parameter.type_reference)?;
    let target = program.primitive_type_reference(operator.return_type)?;
    if !(matches!(source, PrimitiveType::F32 | PrimitiveType::F64)
        || numerics::float_semantics_catalog::IntegerCarrier::from_spelling(source.name())
            .is_some())
        || !matches!(target, PrimitiveType::F32 | PrimitiveType::F64)
    {
        return None;
    }
    for contract in program.operator_contracts(operator) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            let ExpressionNode::Binary(equality) = program.expression_table.expression(*expression)
            else {
                continue;
            };
            if equality.operator != BinaryOperator::Equal {
                continue;
            }
            for (result, converted) in [
                (equality.left, equality.right),
                (equality.right, equality.left),
            ] {
                let Some((result, result_primitive, meaning_type)) = projection(program, result)
                else {
                    continue;
                };
                if result_primitive != target
                    || crate::reserved_result_owner(program, result).map(|owner| owner.0)
                        != Some(operator.symbol)
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        operator.symbol,
                        *expression,
                        language_core::OperatorSpelling::Equal,
                        &[Some(meaning_type), Some(meaning_type)],
                    )
                {
                    continue;
                }
                let ExpressionNode::Call(conversion) =
                    program.expression_table.expression(converted)
                else {
                    continue;
                };
                let source_kind = if matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
                    FloatSemanticValueKind::Meaning
                } else {
                    FloatSemanticValueKind::Integer(
                        numerics::float_semantics_catalog::IntegerCarrier::from_spelling(
                            source.name(),
                        )?,
                    )
                };
                let Some(semantic) =
                    super::semantic_operations::exact_toolchain_float_semantic_application(
                        program,
                        conversion,
                        &[FloatSemanticValueKind::Format, source_kind],
                    )
                else {
                    continue;
                };
                let [format, value] = program
                    .expression_table
                    .expression_handles(conversion.arguments)
                else {
                    continue;
                };
                let expected_format = match target {
                    PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
                    PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
                    _ => return None,
                };
                if super::semantic_values::exact_toolchain_float_format_const(program, *format)
                    != Some(expected_format)
                {
                    continue;
                }
                let value = if matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
                    if semantic.name != "convert" {
                        continue;
                    }
                    let Some((value, source_primitive, _)) = projection(program, *value) else {
                        continue;
                    };
                    if source_primitive != source {
                        continue;
                    }
                    value
                } else {
                    let [
                        FloatSemanticValueKind::Format,
                        FloatSemanticValueKind::Integer(carrier),
                    ] = semantic.parameters
                    else {
                        continue;
                    };
                    if semantic.name != "from_integer"
                        || !matches!(semantic.kernel(), FloatSemanticKernel::FromInteger(_))
                        || PrimitiveType::from_name(carrier.spelling()) != Some(source)
                    {
                        continue;
                    }
                    *value
                };
                let ExpressionNode::Name(value) = program.expression_table.expression(value) else {
                    continue;
                };
                if value.symbol.is_valid() && value.symbol == parameter.symbol {
                    return Some((source, target));
                }
            }
        }
    }
    None
}

fn projection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> Option<(
    ExpressionHandle,
    PrimitiveType,
    typed_trees::types::TypeReferenceHandle,
)> {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
    let (selected, primitive, _, _) =
        crate::proof_contracts::float_projection_invocations::exact_projection_operation(
            program, call,
        )?;
    let operator = typed_trees::operator::declaration_by_symbol(program, selected)?;
    let [operand] = program.expression_table.expression_handles(call.arguments) else {
        return None;
    };
    Some((*operand, primitive, operator.return_type))
}

/// A selected integer-result guarantee over the same sealed float projection.
/// Kernel identity establishes truncation or saturation, never the callable's
/// name. The result occurrence and source parameter belong to this operator.
pub fn float_integer_conversion_contract(
    program: &TypedTrees,
    operator: &OperatorDefinition,
) -> Option<(
    PrimitiveType,
    &'static numerics::float_semantics_catalog::FloatSemanticOperation,
)> {
    use numerics::float_semantics_catalog::{FloatSemanticKernel, FloatSemanticValueKind};
    let [parameter] = program.operator_parameters(operator) else {
        return None;
    };
    let source = program.primitive_type_reference(parameter.type_reference)?;
    if !matches!(source, PrimitiveType::F32 | PrimitiveType::F64) {
        return None;
    }
    let target = program.primitive_type_reference(operator.return_type)?;
    for contract in program.operator_contracts(operator) {
        if contract.kind != SignatureContractKind::Ensures {
            continue;
        }
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let ProofFact::Expression(expression) = fact else {
                continue;
            };
            let ExpressionNode::Binary(equality) = program.expression_table.expression(*expression)
            else {
                continue;
            };
            if equality.operator != BinaryOperator::Equal {
                continue;
            }
            for (result, converted) in [
                (equality.left, equality.right),
                (equality.right, equality.left),
            ] {
                if crate::reserved_result_owner(program, result).map(|owner| owner.0)
                    != Some(operator.symbol)
                {
                    continue;
                }
                let ExpressionNode::Call(conversion) =
                    program.expression_table.expression(converted)
                else {
                    continue;
                };
                let Some(selected) =
                    typed_trees::operator::resolve_named_expression_call(program, conversion)
                else {
                    continue;
                };
                let Some((semantic, _)) =
                    super::semantic_operations::exact_toolchain_float_semantic_contract(
                        program, selected,
                    )
                else {
                    continue;
                };
                let FloatSemanticValueKind::Integer(carrier) = semantic.result else {
                    continue;
                };
                if PrimitiveType::from_name(carrier.spelling()) != Some(target)
                    || !matches!(
                        semantic.kernel(),
                        FloatSemanticKernel::ToIntegerExact
                            | FloatSemanticKernel::ToIntegerSaturating
                    )
                    || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                        program,
                        operator.symbol,
                        *expression,
                        language_core::OperatorSpelling::Equal,
                        &[Some(operator.return_type), Some(selected.return_type)],
                    )
                {
                    continue;
                }
                let [value] = program
                    .expression_table
                    .expression_handles(conversion.arguments)
                else {
                    continue;
                };
                let Some((value, primitive, _)) = projection(program, *value) else {
                    continue;
                };
                let ExpressionNode::Name(value) = program.expression_table.expression(value) else {
                    continue;
                };
                if primitive == source
                    && value.symbol.is_valid()
                    && value.symbol == parameter.symbol
                {
                    return Some((source, semantic));
                }
            }
        }
    }
    None
}

/// Both admitted integer-conversion kernels are monotone on ordered inputs.
/// Exact here is the named truncating operation's contract-gated meaning;
/// authored Exact coercions separately owe integrality before using it.
pub fn float_integer_conversion_bounds(
    program: &TypedTrees,
    operator: &OperatorDefinition,
    minimum: f64,
    maximum: f64,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use numerics::float_semantics::FloatMeaning;
    use numerics::float_semantics_catalog::{FloatSemanticOperand, FloatSemanticResult};
    let (_, semantic) = float_integer_conversion_contract(program, operator)?;
    if minimum > maximum || minimum.is_nan() || maximum.is_nan() {
        return None;
    }
    let convert = |value| {
        let discharged = semantic
            .kernel_discharge(&[FloatSemanticOperand::Meaning(FloatMeaning::from_f64(value))])?;
        let FloatSemanticResult::Integer(value) = discharged.result else {
            return None;
        };
        Some(value)
    };
    Some((convert(minimum)?, convert(maximum)?))
}
