use super::*;
use semantic_vocabulary::OperationId;
pub(in crate::legalization) fn instruction(
    node: &OptimizationNode,
) -> Option<(OperationId, ValueId)> {
    match &node.operation {
        AbstractOperation::IntegerConstant {
            psi_operation,
            result,
            scalar_type,
            value,
        } if *scalar_type == ScalarType::Integer(u64_type())
            && matches!(value,semantic_vocabulary::IntegerValue::Unsigned(value) if *value <= u128::from(u64::MAX)) =>
        {
            Some((*psi_operation, *result))
        }
        AbstractOperation::Call {
            psi_operation,
            result,
            scalar_type,
            requirement_obligations,
            crash_continuations,
            ..
        } if *scalar_type == ScalarType::Integer(u64_type())
            && requirement_obligations.is_empty()
            && crash_continuations.is_empty() =>
        {
            Some((*psi_operation, *result))
        }
        AbstractOperation::ExactIntegerAdd {
            psi_operation,
            result,
            scalar_type,
            ..
        }
        | AbstractOperation::ExactIntegerSubtract {
            psi_operation,
            result,
            scalar_type,
            ..
        } if *scalar_type == u64_type() => Some((*psi_operation, *result)),
        _ => None,
    }
}
pub(super) fn validate(
    block: &OptimizationBlock,
    body: &[OptimizationNode],
    returned: &OptimizationNode,
    abstracted: &AbstractFunction,
    optimized: &PsiOptimizationFunction,
) -> Result<(), LegalizationError> {
    let invalid = LegalizationError::SourceCustodyMismatch;
    let mut available = optimized
        .parameters
        .iter()
        .map(|value| value.value)
        .collect::<Vec<_>>();
    let mut operations = Vec::new();
    for (index, node) in body.iter().enumerate() {
        let (operation, result) = instruction(node).ok_or(invalid.clone())?;
        if available.contains(&result)
            || operations.contains(&operation)
            || node.definitions
                != [ValueDefinition {
                    value: result,
                    scalar_type: ScalarType::Integer(u64_type()),
                    site: ValueDefinitionSite::Node {
                        block: block.id,
                        node: index as u32,
                    },
                }]
            || !node.successors.is_empty()
            || node.provenance != [PsiProvenance::Operation(operation)]
            || node.fuel.is_empty()
            || node
                .fuel
                .iter()
                .any(|fuel| fuel.site != PsiProvenance::Operation(operation))
            || node
                .uses
                .iter()
                .any(|value| !available.contains(&value.value))
        {
            return Err(invalid);
        }
        available.push(result);
        operations.push(operation);
    }
    let edge = match (&returned.operation, &abstracted.result) {
        (
            AbstractOperation::ReturnUnit {
                psi_edge,
                cleanup_actions,
            },
            AbstractFunctionResult::Unit,
        ) if cleanup_actions.is_empty() => *psi_edge,
        (
            AbstractOperation::Return {
                psi_edge,
                result,
                value,
                scalar_type,
                cleanup_actions,
            },
            AbstractFunctionResult::Scalar(declared),
        ) if *result == declared.value
            && *scalar_type == declared.scalar_type
            && available.contains(value)
            && cleanup_actions.is_empty() =>
        {
            *psi_edge
        }
        _ => return Err(invalid),
    };
    if !returned.successors.is_empty()
        || returned.provenance != [PsiProvenance::Edge(edge)]
        || returned.fuel.is_empty()
        || returned
            .fuel
            .iter()
            .any(|fuel| fuel.site != PsiProvenance::Edge(edge))
        || returned
            .uses
            .iter()
            .any(|value| !available.contains(&value.value))
    {
        return Err(invalid);
    }
    Ok(())
}
