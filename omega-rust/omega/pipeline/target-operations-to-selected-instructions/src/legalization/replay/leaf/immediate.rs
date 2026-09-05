//! Immediate and source-constant replay.

use super::*;

pub(super) fn replay_immediate(
    function: usize,
    arm_edge: EdgeId,
    target: &TargetIntegerExpression,
    node: &optimization_unit::OptimizationNode,
    proposed: &LegalizedImmediate,
    expected_type: ScalarType,
) -> Result<(), LegalizationError> {
    let TargetIntegerExpression::Immediate {
        source_value,
        value,
    } = target
    else {
        return Err(Error::UnsupportedSourceShape { function });
    };
    replay_constant(
        function,
        arm_edge,
        *source_value,
        *value,
        node,
        proposed.constant_operation,
        proposed.definition_site,
        &proposed.fuel,
        expected_type,
    )?;
    if proposed.source_value != *source_value || proposed.value != *value {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_constant(
    function: usize,
    arm_edge: EdgeId,
    source_value: semantic_vocabulary::ValueId,
    target_value: semantic_vocabulary::IntegerValue,
    node: &optimization_unit::OptimizationNode,
    proposed_operation: OperationId,
    proposed_site: optimization_unit::ValueDefinitionSite,
    proposed_fuel: &[optimization_unit::FuelSettlement],
    expected_type: ScalarType,
) -> Result<OperationId, LegalizationError> {
    let AbstractOperation::IntegerConstant {
        psi_operation,
        result,
        scalar_type,
        value,
    } = &node.operation
    else {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    };
    if *result != source_value
        || *value != target_value
        || *scalar_type != expected_type
        || node.definitions.len() != 1
        || node.definitions[0].value != source_value
        || node.provenance != vec![PsiProvenance::Operation(*psi_operation)]
    {
        return Err(Error::MissingConstantDefinition { function, arm_edge });
    }
    if proposed_operation != *psi_operation || proposed_site != node.definitions[0].site {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, *psi_operation, &node.fuel, proposed_fuel)?;
    Ok(*psi_operation)
}
