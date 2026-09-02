use super::super::shared::*;

pub(in crate::legalization::source) fn exact_edge_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    edge: EdgeId,
    function: usize,
) -> Result<Vec<FuelSettlement>, LegalizationError> {
    let custody = node
        .successors
        .iter()
        .find(|successor| successor.psi_edge == edge)
        .map_or(node.fuel.as_slice(), |successor| successor.fuel.as_slice());
    let fuel = custody
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Edge(edge))
        .collect::<Vec<_>>();
    if fuel.is_empty() || fuel.len() != custody.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}

pub(in crate::legalization::source) fn exact_operation_fuel(
    node: &omega_optimization_unit::OptimizationNode,
    operation: OperationId,
    function: usize,
) -> Result<Vec<FuelSettlement>, LegalizationError> {
    let fuel = node
        .fuel
        .iter()
        .copied()
        .filter(|settlement| settlement.site == PsiProvenance::Operation(operation))
        .collect::<Vec<_>>();
    if fuel.is_empty() || fuel.len() != node.fuel.len() {
        return Err(Error::MissingFuelProvenance { function });
    }
    Ok(fuel)
}
