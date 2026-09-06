//! Input-only semantic correspondence for ordered exact integer expressions.
//! No legalized rows, register assignments, or proof evidence are constructed.
use abstract_operations::AbstractOperation;
use optimization_unit::{OptimizationNode, PsiProvenance, ValueDefinition};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType, ValueId};
use target_operations::TargetIntegerExpression;

pub(in crate::legalization) fn expression_shape(expression: &TargetIntegerExpression) -> bool {
    match expression {
        TargetIntegerExpression::Immediate { .. } | TargetIntegerExpression::Parameter { .. } => {
            true
        }
        TargetIntegerExpression::ExactAdd { left, right, .. }
        | TargetIntegerExpression::ExactSubtract { left, right, .. } => {
            expression_shape(left) && expression_shape(right)
        }
        _ => false,
    }
}

pub(in crate::legalization) fn validate(
    expression: &TargetIntegerExpression,
    result: ValueId,
    nodes: &[OptimizationNode],
    parameters: &[ValueDefinition],
) -> bool {
    let Some((returned, body)) = nodes.split_last() else {
        return false;
    };
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).expect("U64");
    let scalar = ScalarType::Integer(integer);
    if !matches!(&returned.operation, AbstractOperation::Return { value, scalar_type, cleanup_actions, .. }
        if *value == result && *scalar_type == scalar && cleanup_actions.is_empty())
    {
        return false;
    }
    let mut values = parameters
        .iter()
        .filter(|parameter| parameter.scalar_type == scalar)
        .map(|parameter| parameter.value)
        .collect::<Vec<_>>();
    let mut operations = Vec::new();
    for node in body {
        let (operation, value, operands) = match &node.operation {
            AbstractOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            } if *scalar_type == scalar
                && matches!(value, semantic_vocabulary::IntegerValue::Unsigned(value) if *value <= u128::from(u64::MAX)) =>
            {
                (*psi_operation, *result, Vec::new())
            }
            AbstractOperation::ExactIntegerAdd {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            }
            | AbstractOperation::ExactIntegerSubtract {
                psi_operation,
                result,
                scalar_type,
                left,
                right,
                ..
            } if *scalar_type == integer => (*psi_operation, *result, vec![*left, *right]),
            _ => return false,
        };
        if values.contains(&value)
            || operations.contains(&operation)
            || operands.iter().any(|operand| !values.contains(operand))
            || node.definitions.len() != 1
            || node.definitions[0].value != value
            || node.definitions[0].scalar_type != scalar
            || !node.successors.is_empty()
            || node.provenance != [PsiProvenance::Operation(operation)]
        {
            return false;
        }
        values.push(value);
        operations.push(operation);
    }
    values.contains(&result) && matches_value(expression, result, body, parameters)
}

fn matches_value(
    expression: &TargetIntegerExpression,
    value: ValueId,
    nodes: &[OptimizationNode],
    parameters: &[ValueDefinition],
) -> bool {
    match expression {
        TargetIntegerExpression::Immediate { source_value, value: literal } =>
            *source_value == value && nodes.iter().any(|node| matches!(&node.operation,
                AbstractOperation::IntegerConstant { result, value: actual, .. } if *result == value && literal == actual)),
        TargetIntegerExpression::Parameter { source_value, parameter_index, .. } =>
            *source_value == value && parameters.get(*parameter_index).is_some_and(|parameter| parameter.value == value),
        TargetIntegerExpression::ExactAdd { psi_operation, obligation, left, right }
        | TargetIntegerExpression::ExactSubtract { psi_operation, obligation, left, right } => {
            let Some(node) = nodes.iter().find(|node| matches!(&node.operation,
                AbstractOperation::ExactIntegerAdd { psi_operation: operation, .. }
                | AbstractOperation::ExactIntegerSubtract { psi_operation: operation, .. } if operation == psi_operation)) else { return false; };
            let (result, source_obligation, source_left, source_right) = match (&node.operation, expression) {
                (AbstractOperation::ExactIntegerAdd { result, obligation, left, right, .. }, TargetIntegerExpression::ExactAdd { .. })
                | (AbstractOperation::ExactIntegerSubtract { result, obligation, left, right, .. }, TargetIntegerExpression::ExactSubtract { .. }) =>
                    (*result, *obligation, *left, *right),
                _ => return false,
            };
            result == value && source_obligation == *obligation
                && matches_value(left, source_left, nodes, parameters)
                && matches_value(right, source_right, nodes, parameters)
        },
        _ => false,
    }
}
