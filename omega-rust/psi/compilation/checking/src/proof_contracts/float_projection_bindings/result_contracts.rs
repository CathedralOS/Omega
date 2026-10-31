//! The raw denotation supplied by one selected float-producing requirement.
//!
//! A result policy runs after the provider's result. Its failure cannot be
//! excluded by the adapted result's normal-return finiteness. Instead retain
//! the sealed semantic term equated to that requirement's result projection,
//! with only its own formals and closed catalog values as inputs.

use crate::frozen_program::expression_table;
use numerics::float_semantics_catalog::FloatSemanticValueKind;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::operator::{OperatorDefinition, resolve_named_expression_call};
use typed_trees::types::PrimitiveType;

/// Read the selected callable's result equation without granting semantics
/// from its name, result qualification, or a post-adapter observation.
pub fn float_result_semantic_contract(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<ExpressionHandle> {
    let target = program.primitive_type_reference(operator.return_type)?;
    if !matches!(target, PrimitiveType::F32 | PrimitiveType::F64) {
        return None;
    }
    for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
        program,
        program.operator_contracts(operator),
    ) {
        let Some(equality) = expression_table(program).binary_node(expression) else {
            continue;
        };
        if equality.operator != BinaryOperator::Equal {
            continue;
        }
        for (result, meaning) in [
            (equality.left, equality.right),
            (equality.right, equality.left),
        ] {
            let Some(result_call) = expression_table(program).call_node(result) else {
                continue;
            };
            let Some((projection, primitive, _, _)) =
                crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                    program,
                    result_call,
                )
            else {
                continue;
            };
            let [result_value] =
                expression_table(program).expression_handles(result_call.arguments)
            else {
                continue;
            };
            let Some(projection) = program.operator_definition_by_symbol(projection) else {
                continue;
            };
            if primitive != target
                || crate::reserved_result_owner(program, *result_value).map(|owner| owner.0)
                    != Some(operator.symbol)
                || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                    program,
                    operator.symbol,
                    expression,
                    language_core::OperatorSpelling::Equal,
                    &[Some(projection.return_type), Some(projection.return_type)],
                )
            {
                continue;
            }
            let Some(call) = expression_table(program).call_node(meaning) else {
                continue;
            };
            let Some(semantic) =
                super::semantic_operations::float_semantic_application_for_parameters(
                    program,
                    call,
                    operator.symbol,
                    program.operator_parameters(operator),
                )
                .map(|(_, row, _)| row)
            else {
                continue;
            };
            if semantic.result != FloatSemanticValueKind::Meaning {
                continue;
            }
            // Every formatted raw operation must name the result carrier's
            // actual format. A pair operation without a format preserves the
            // selected requirement's existing float carrier.
            let expected = match target {
                PrimitiveType::F32 => semantic_vocabulary::IeeeFloatFormat::Binary32,
                PrimitiveType::F64 => semantic_vocabulary::IeeeFloatFormat::Binary64,
                _ => unreachable!("float result checked above"),
            };
            let arguments = expression_table(program).expression_handles(call.arguments);
            if arguments.len() != semantic.parameters.len()
                || arguments
                    .iter()
                    .zip(semantic.parameters)
                    .any(|(argument, kind)| {
                        *kind == FloatSemanticValueKind::Format
                            && super::semantic_values::exact_toolchain_float_format_const(
                                program, *argument,
                            ) != Some(expected)
                    })
            {
                continue;
            }
            let mut remaining_work = 1024;
            if semantic_term_uses_only_inputs(program, operator, meaning, &mut remaining_work) {
                return Some(meaning);
            }
        }
    }
    None
}

/// The overflow adapter's zero-divisor exception, selected from the sealed raw
/// semantic equation. This catalog identity never comes from the source token
/// or the public callable's name. Every other operation uses all finite inputs.
pub fn float_saturation_divisor_parameter(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<Option<usize>> {
    let raw = float_result_semantic_contract(program, operator)?;
    let call = expression_table(program).call_node(raw)?;
    let selected = resolve_named_expression_call(program, call)?;
    let (row, _) =
        super::semantic_operations::exact_toolchain_float_semantic_contract(program, selected)?;
    if !matches!(
        row.name,
        "divide" | "divide_toward_zero" | "divide_toward_positive" | "divide_toward_negative"
    ) {
        return Some(None);
    }
    let divisor = *expression_table(program)
        .expression_handles(call.arguments)
        .last()?;
    let projection = expression_table(program).call_node(divisor)?;
    crate::proof_contracts::float_projection_invocations::exact_projection_operation(
        program, projection,
    )?;
    let [source] = expression_table(program).expression_handles(projection.arguments) else {
        return None;
    };
    let ExpressionNode::Name(path) = expression_table(program).expression(*source) else {
        return None;
    };
    program
        .operator_parameters(operator)
        .iter()
        .position(|parameter| parameter.symbol == path.symbol)
        .map(Some)
}

fn semantic_term_uses_only_inputs(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
    expression: ExpressionHandle,
    remaining_work: &mut usize,
) -> bool {
    let Some(next) = remaining_work.checked_sub(1) else {
        return false;
    };
    *remaining_work = next;
    if super::semantic_values::exact_toolchain_float_format_const(program, expression).is_some() {
        return true;
    }
    match expression_table(program).expression(expression) {
        ExpressionNode::Float(_) | ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Name(path) => {
            program
                .operator_parameters(operator)
                .iter()
                .any(|parameter| parameter.symbol == path.symbol)
                || super::semantic_values::exact_toolchain_float_format_const(program, expression)
                    .is_some()
        }
        ExpressionNode::Call(call) => {
            let admitted =
                crate::proof_contracts::float_projection_invocations::exact_projection_operation(
                    program, call,
                )
                .is_some()
                    || super::semantic_operations::float_semantic_application_for_parameters(
                        program,
                        call,
                        operator.symbol,
                        program.operator_parameters(operator),
                    )
                    .is_some();
            admitted
                && expression_table(program)
                    .expression_handles(call.arguments)
                    .iter()
                    .all(|argument| {
                        semantic_term_uses_only_inputs(program, operator, *argument, remaining_work)
                    })
        }
        _ => false,
    }
}

/// A public wellness query may expose a sealed Boolean semantic predicate in
/// its normal-return result equation. Consumers can transport an observed
/// Boolean polarity through that equation without exposing private projections
/// as package-callable APIs.
pub fn float_boolean_semantic_contract(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<ExpressionHandle> {
    (program.primitive_type_reference(operator.return_type) == Some(PrimitiveType::Bool))
        .then(|| float_scalar_semantic_contract(program, operator))
        .flatten()
}

/// Rejoin a selected scalar result equation to one closed semantic catalog
/// row. Bool, class and integer results share exact source/result/formal
/// custody; their carrier kind does not authorize another row's denotation.
pub(crate) fn float_scalar_semantic_contract(
    program: &dyn crate::CheckingInput,
    operator: &OperatorDefinition,
) -> Option<ExpressionHandle> {
    let result_kind = super::semantic_operations::typed_value_kind(program, operator.return_type)?;
    if !matches!(
        result_kind,
        FloatSemanticValueKind::Bool
            | FloatSemanticValueKind::Class
            | FloatSemanticValueKind::Integer(_)
    ) {
        return None;
    }
    for expression in crate::proof_contracts::proof_facts::ensures_expression_facts(
        program,
        program.operator_contracts(operator),
    ) {
        let Some(equality) = expression_table(program).binary_node(expression) else {
            continue;
        };
        if equality.operator != BinaryOperator::Equal
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                operator.symbol,
                expression,
                language_core::OperatorSpelling::Equal,
                &[Some(operator.return_type), Some(operator.return_type)],
            )
        {
            continue;
        }
        for (result, predicate) in [
            (equality.left, equality.right),
            (equality.right, equality.left),
        ] {
            if crate::reserved_result_owner(program, result).map(|owner| owner.0)
                != Some(operator.symbol)
            {
                continue;
            }
            let Some(call) = expression_table(program).call_node(predicate) else {
                continue;
            };
            let Some((_, row, _)) =
                super::semantic_operations::float_semantic_application_for_parameters(
                    program,
                    call,
                    operator.symbol,
                    program.operator_parameters(operator),
                )
            else {
                continue;
            };
            let mut remaining_work = 1024;
            if row.result == result_kind
                && semantic_term_uses_only_inputs(program, operator, predicate, &mut remaining_work)
            {
                return Some(predicate);
            }
        }
    }
    None
}
