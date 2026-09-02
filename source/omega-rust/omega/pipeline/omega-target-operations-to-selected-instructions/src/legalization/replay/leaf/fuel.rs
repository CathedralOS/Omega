//! Operation and edge logical-fuel replay.

use super::*;

pub(in crate::legalization::replay) fn replay_operation_fuel(
    function: usize,
    operation: OperationId,
    source: &[omega_optimization_unit::FuelSettlement],
    proposed: &[omega_optimization_unit::FuelSettlement],
) -> Result<(), LegalizationError> {
    if source.is_empty()
        || source
            .iter()
            .any(|settlement| settlement.site != PsiProvenance::Operation(operation))
    {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed != source {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}

pub(in crate::legalization::replay) fn replay_edge_fuel(
    function: usize,
    edge: EdgeId,
    source: &[omega_optimization_unit::FuelSettlement],
    proposed: &[omega_optimization_unit::FuelSettlement],
) -> Result<(), LegalizationError> {
    if source.is_empty()
        || source
            .iter()
            .any(|settlement| settlement.site != PsiProvenance::Edge(edge))
    {
        return Err(Error::MissingFuelProvenance { function });
    }
    if proposed != source {
        return Err(Error::NonCanonicalLegalizedPlan);
    }
    Ok(())
}
