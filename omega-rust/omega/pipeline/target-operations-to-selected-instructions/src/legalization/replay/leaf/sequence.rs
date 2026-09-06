//! Independent replay of every retained ordered scalar instruction.
use super::*;
use legalized_operations::{
    LegalizedExactIntegerOperator as Operator, LegalizedExactIntegerSequence, LegalizedIntegerStep,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn replay<'a>(
    function: usize,
    arm_edge: EdgeId,
    expression: &TargetIntegerExpression,
    result: semantic_vocabulary::ValueId,
    nodes: &'a [optimization_unit::OptimizationNode],
    optimized: &optimization_unit::PsiOptimizationFunction,
    accepted_facts: &[optimization_unit::AcceptedObligationFact],
    proposed: &LegalizedExactIntegerSequence,
    scalar: ScalarType,
) -> Result<(&'a optimization_unit::OptimizationNode, Vec<OperationId>), LegalizationError> {
    if !crate::legalization::integer_sequence_input::validate(
        expression,
        result,
        nodes,
        &optimized.parameters,
    ) {
        return Err(Error::UnsupportedSourceShape { function });
    }
    let (returned, body) = nodes.split_last().ok_or(Error::NonCanonicalLegalizedPlan)?;
    let parameters = optimized
        .parameters
        .iter()
        .filter(|parameter| parameter.scalar_type == scalar)
        .map(|parameter| parameter.value)
        .collect::<Vec<_>>();
    if body.len() != proposed.steps.len() || proposed.validate_shape(&parameters, result).is_err() {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    let mut operations = Vec::new();
    for (node, step) in body.iter().zip(&proposed.steps) {
        let operation = match (&node.operation, step) {
            (
                AbstractOperation::IntegerConstant {
                    psi_operation,
                    result,
                    value,
                    ..
                },
                LegalizedIntegerStep::Immediate(proposed),
            ) => {
                if proposed.source_value != *result || proposed.value != *value {
                    return Err(Error::NonCanonicalLegalizedPlan);
                }
                replay_constant(
                    function,
                    arm_edge,
                    *result,
                    *value,
                    node,
                    proposed.constant_operation,
                    proposed.definition_site,
                    &proposed.fuel,
                    scalar,
                )?;
                *psi_operation
            }
            (
                AbstractOperation::ExactIntegerAdd {
                    psi_operation,
                    obligation,
                    result,
                    left,
                    right,
                    ..
                }
                | AbstractOperation::ExactIntegerSubtract {
                    psi_operation,
                    obligation,
                    result,
                    left,
                    right,
                    ..
                },
                LegalizedIntegerStep::ExactBinary(proposed),
            ) => {
                let operator = match node.operation {
                    AbstractOperation::ExactIntegerAdd { .. } => Operator::Add,
                    AbstractOperation::ExactIntegerSubtract { .. } => Operator::Subtract,
                    _ => unreachable!("matched integer operation"),
                };
                let fact = accepted_facts
                    .iter()
                    .find(|fact| {
                        fact.machine == optimized.machine
                            && fact.operation == *psi_operation
                            && fact.obligation == *obligation
                    })
                    .ok_or(Error::SourceCustodyMismatch)?;
                if !optimized.facts.iter().any(|fact| matches!(fact,
                    OptimizationFact::OperationObligationReference { obligation: referenced, support }
                    if referenced == obligation && support == psi_operation)) {
                    return Err(Error::SourceCustodyMismatch);
                }
                if proposed.operator != operator
                    || proposed.operation != *psi_operation
                    || proposed.obligation != *obligation
                    || proposed.accepted_fact != fact.identity
                    || proposed.source_value != *result
                    || proposed.left != *left
                    || proposed.right != *right
                    || proposed.definition_site != node.definitions[0].site
                {
                    return Err(Error::NonCanonicalLegalizedPlan);
                }
                replay_operation_fuel(function, *psi_operation, &node.fuel, &proposed.fuel)?;
                *psi_operation
            }
            _ => return Err(Error::NonCanonicalLegalizedPlan),
        };
        operations.push(operation);
    }
    Ok((returned, operations))
}
