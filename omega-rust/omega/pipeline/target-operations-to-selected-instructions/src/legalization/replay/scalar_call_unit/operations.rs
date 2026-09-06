//! Independent legalized constant, call, and return custody checks.

use super::super::shared::*;

pub(super) fn replay_constant(
    function: usize,
    block: semantic_vocabulary::BlockId,
    index: u32,
    target: &TargetUnitOperation,
    node: &optimization_unit::OptimizationNode,
    operation: OperationId,
    result: ValueId,
    proposed: &LegalizedScalarCallUnitConstant,
) -> Result<(), LegalizationError> {
    let TargetUnitOperation::IntegerConstant {
        scalar_type, value, ..
    } = target
    else {
        unreachable!()
    };
    let definition = optimization_unit::ValueDefinition {
        value: result,
        scalar_type: ScalarType::Integer(*scalar_type),
        site: optimization_unit::ValueDefinitionSite::Node { block, node: index },
    };
    if node.provenance != [PsiProvenance::Operation(operation)]
        || node.definitions != [definition]
        || !node.successors.is_empty()
        || proposed.operation != operation
        || proposed.result != result
        || proposed.scalar_type != *scalar_type
        || proposed.value != *value
        || proposed.definition_site != definition.site
        || proposed.fuel != node.fuel
        || proposed.effect != node.effect
        || proposed.ownership != node.ownership
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &node.fuel, &proposed.fuel)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn replay_call(
    function: usize,
    block: semantic_vocabulary::BlockId,
    index: u32,
    target: &TargetUnitOperation,
    node: &optimization_unit::OptimizationNode,
    operation: OperationId,
    result: ValueId,
    proposed: &LegalizedScalarCallUnitCall,
) -> Result<(), LegalizationError> {
    let TargetUnitOperation::ScalarCall {
        callee,
        call_plan,
        result_home,
        arguments,
        requirement_obligations,
        crash_continuations,
        ..
    } = target
    else {
        unreachable!()
    };
    let definition = optimization_unit::ValueDefinition {
        value: result,
        scalar_type: result_home.scalar_type,
        site: optimization_unit::ValueDefinitionSite::Node { block, node: index },
    };
    if node.provenance != [PsiProvenance::Operation(operation)]
        || node.definitions != [definition]
        || !node.successors.is_empty()
        || proposed.operation != operation
        || proposed.callee != *callee
        || proposed.call_plan != *call_plan
        || proposed.result_home != *result_home
        || proposed.result_definition_site != definition.site
        || proposed.arguments.len() != arguments.len()
        || proposed
            .arguments
            .iter()
            .zip(arguments)
            .any(|(proposed, target)| {
                proposed.parameter_index != target.parameter_index
                    || proposed.source != target.source
                    || proposed.placement != target.placement
            })
        || proposed.validate_shape().is_err()
        || proposed.requirement_obligations != *requirement_obligations
        || proposed.crash_continuations != *crash_continuations
        || proposed.fuel != node.fuel
        || proposed.effect != node.effect
        || proposed.ownership != node.ownership
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_operation_fuel(function, operation, &node.fuel, &proposed.fuel)
}

pub(super) fn replay_return(
    function: usize,
    node: &optimization_unit::OptimizationNode,
    edge: EdgeId,
    proposed: &LegalizedScalarCallUnitFunction,
) -> Result<(), LegalizationError> {
    if node.provenance != [PsiProvenance::Edge(edge)]
        || !node.definitions.is_empty()
        || !node.uses.is_empty()
        || !node.successors.is_empty()
        || proposed.return_fuel != node.fuel
        || proposed.return_effect != node.effect
        || proposed.return_ownership != node.ownership
    {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    replay_edge_fuel(function, edge, &node.fuel, &proposed.return_fuel)
}
