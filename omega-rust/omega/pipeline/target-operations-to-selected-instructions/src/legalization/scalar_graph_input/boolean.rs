//! Boolean predicates form branch suffixes; materialized values also cross Unit edges.
use super::*;
use optimization_unit::OptimizationBlock;

pub(super) fn validate(
    block: &OptimizationBlock,
    function: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let (terminator, body) = block.nodes.split_last().ok_or(invalid.clone())?;
    let mut suffix = Vec::new();
    if let AbstractOperation::Conditional { condition, .. } = terminator.operation {
        let mut value = condition;
        for node in body.iter().rev() {
            match node.operation {
                AbstractOperation::BooleanNot {
                    result, operand, ..
                } if result == value => {
                    suffix.push(result);
                    value = operand;
                }
                AbstractOperation::IntegerEqual { result, .. }
                | AbstractOperation::IntegerLessThan { result, .. }
                | AbstractOperation::IntegerLessOrEqual { result, .. }
                    if result == value =>
                {
                    suffix.push(result);
                    break;
                }
                _ => break,
            }
        }
        if !suffix.contains(&value)
            && !function.parameters.iter().any(|parameter| {
                parameter.value == value && parameter.scalar_type == ScalarType::Boolean
            })
            && !(function.result == AbstractFunctionResult::Unit
                && function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.parameters)
                    .any(|parameter| {
                        parameter.value == value && parameter.scalar_type == ScalarType::Boolean
                    }))
        {
            return Err(invalid);
        }
    }
    for node in body {
        if let AbstractOperation::BooleanConstant { result, .. } = node.operation {
            let mut consumers = function
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .filter(|consumer| consumer.uses.iter().any(|used| used.value == result))
                .peekable();
            if consumers.peek().is_none()
                || !consumers.all(|consumer| {
                    matches!(&consumer.operation,
                    AbstractOperation::CallUnit { arguments, .. }
                        | AbstractOperation::CallStructuralScalar { arguments, .. }
                        if arguments.contains(&result))
                        || matches!(&consumer.operation,
                            AbstractOperation::StructuralScalarFieldStore { value, .. }
                            | AbstractOperation::WriteOnlyPrimitiveStore { value, .. }
                            | AbstractOperation::EstablishPrimitiveLocal { value, .. }
                            | AbstractOperation::PrimitiveLocalStore { value, .. }
                            if value.value == result && value.scalar_type == ScalarType::Boolean)
                        || (function.result == AbstractFunctionResult::Unit
                            && matches!(
                                &consumer.operation,
                                AbstractOperation::Jump { .. }
                                    | AbstractOperation::Conditional { .. }
                            )
                            && consumer
                                .successors
                                .iter()
                                .flat_map(|successor| &successor.bindings)
                                .any(|binding| {
                                    binding.argument == result
                                        && binding.scalar_type == ScalarType::Boolean
                                }))
                })
            {
                return Err(invalid);
            }
            continue;
        }
        if matches!(node.operation, AbstractOperation::PrimitiveScalarRead { result, .. }
            if result.scalar_type == ScalarType::Boolean)
        {
            continue;
        }
        if let Some(definition) = node.definitions.first()
            && definition.scalar_type == ScalarType::Boolean
            && (!suffix.contains(&definition.value)
                || function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.nodes)
                    .flat_map(|node| &node.uses)
                    .filter(|used| used.value == definition.value)
                    .count()
                    != 1)
        {
            return Err(invalid);
        }
    }
    Ok(())
}
