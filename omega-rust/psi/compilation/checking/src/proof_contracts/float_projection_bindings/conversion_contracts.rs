//! Read a selected callable's IEEE conversion guarantee, not its leaf name.
//!
//! The result projection, source parameter and format constant must join the
//! same ensures equality through the sealed float catalogs. No conversion
//! meaning comes from a destination annotation or a callable named `from_f64`.

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
    let [parameter] = program.operator_parameters(operator) else {
        return None;
    };
    let source = program.primitive_type_reference(parameter.type_reference)?;
    let target = program.primitive_type_reference(operator.return_type)?;
    if !matches!(source, PrimitiveType::F32 | PrimitiveType::F64)
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
                if semantic.name != "convert" {
                    continue;
                }
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
                let Some((value, source_primitive, _)) = projection(program, *value) else {
                    continue;
                };
                let ExpressionNode::Name(value) = program.expression_table.expression(value) else {
                    continue;
                };
                if source_primitive == source
                    && value.symbol.is_valid()
                    && value.symbol == parameter.symbol
                {
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
