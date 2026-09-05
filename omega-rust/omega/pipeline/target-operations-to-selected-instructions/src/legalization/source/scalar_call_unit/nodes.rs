use super::super::leaves::{exact_edge_fuel, exact_operation_fuel};
use super::super::shared::*;

pub(super) fn validate_value_node(
    function: usize,
    block: semantic_vocabulary::BlockId,
    index: u32,
    node: &optimization_unit::OptimizationNode,
    operation: OperationId,
    result: ValueId,
    scalar_type: IntegerType,
) -> Result<(), LegalizationError> {
    let expected_definition = optimization_unit::ValueDefinition {
        value: result,
        scalar_type: ScalarType::Integer(scalar_type),
        site: optimization_unit::ValueDefinitionSite::Node { block, node: index },
    };
    if node.provenance != [PsiProvenance::Operation(operation)]
        || node.definitions != [expected_definition]
        || !node.successors.is_empty()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    exact_operation_fuel(node, operation, function)?;
    Ok(())
}

pub(super) fn validate_return_node(
    function: usize,
    node: &optimization_unit::OptimizationNode,
    edge: EdgeId,
) -> Result<(), LegalizationError> {
    if node.provenance != [PsiProvenance::Edge(edge)]
        || !node.definitions.is_empty()
        || !node.uses.is_empty()
        || !node.successors.is_empty()
    {
        return Err(Error::UnsupportedSourceShape { function });
    }
    exact_edge_fuel(node, edge, function)?;
    Ok(())
}
