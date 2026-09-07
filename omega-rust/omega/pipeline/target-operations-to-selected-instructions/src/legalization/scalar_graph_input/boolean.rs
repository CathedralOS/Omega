//! Boolean results remain a sole-use suffix consumed by the current branch.
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
        {
            return Err(invalid);
        }
    }
    for node in body {
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
