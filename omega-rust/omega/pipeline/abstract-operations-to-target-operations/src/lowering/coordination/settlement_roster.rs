//! The boundary-settlement roster contract beside the lowering
//! coordinator: index the caller-supplied settlement bindings, then rejoin
//! them fail-closed against the plan's boundary calls and the
//! installation's admitted provider calls — every required boundary settled
//! exactly once, none settled twice, none unused, and no overlap with an
//! installed provider.

use super::super::shared::*;
use super::installed_provider_calls::{BoundaryCallsByKey, InstalledCallsByCall};

/// Caller-supplied settlements indexed by the boundary they settle.
pub(super) type SettlementsByBoundary = BTreeMap<BoundaryMachineId, BoundarySettlementBinding>;

/// Index the supplied settlement bindings: each boundary settles at most
/// once and must name a boundary machine the plan declares.
pub(super) fn index_settlement_bindings(
    plan: &AbstractOperationPlan,
    settlement_bindings: &[BoundarySettlementBinding],
) -> Result<SettlementsByBoundary, LoweringError> {
    let mut settlements_by_boundary = SettlementsByBoundary::new();
    for binding in settlement_bindings {
        if settlements_by_boundary
            .insert(binding.boundary, binding.clone())
            .is_some()
        {
            return Err(LoweringError::DuplicateBoundarySettlement(binding.boundary));
        }
        if !plan
            .boundary_machines
            .iter()
            .any(|boundary| boundary.id == binding.boundary)
        {
            return Err(LoweringError::UnknownBoundarySettlement(binding.boundary));
        }
    }
    Ok(settlements_by_boundary)
}

/// Rejoin the indexed roster against the boundary calls: an installed
/// provider owns its boundary outright, every remaining boundary call needs
/// a settlement, and no settlement may go unused.
pub(super) fn validate_settlement_roster(
    settlements_by_boundary: &SettlementsByBoundary,
    installed_by_call: &InstalledCallsByCall,
    boundary_calls: &BoundaryCallsByKey<'_>,
) -> Result<(), LoweringError> {
    let installed_boundaries = installed_by_call
        .keys()
        .map(|(_, _, boundary)| *boundary)
        .collect::<BTreeSet<_>>();
    if let Some(boundary) = settlements_by_boundary
        .keys()
        .find(|boundary| installed_boundaries.contains(boundary))
    {
        return Err(LoweringError::BoundarySettlementOverlapsInstalledProvider(
            *boundary,
        ));
    }
    if let Some((machine, operation, boundary)) = boundary_calls
        .keys()
        .find(|key| installed_boundaries.contains(&key.2) && !installed_by_call.contains_key(key))
        .copied()
    {
        return Err(LoweringError::PartialInstalledProviderBoundary {
            machine,
            operation,
            boundary,
        });
    }
    let required_settlements = boundary_calls
        .keys()
        .filter_map(|key| (!installed_by_call.contains_key(key)).then_some(key.2))
        .collect::<BTreeSet<_>>();
    for boundary in &required_settlements {
        if !settlements_by_boundary.contains_key(boundary) {
            return Err(LoweringError::MissingBoundarySettlement(*boundary));
        }
    }
    if let Some(extra) = settlements_by_boundary
        .keys()
        .find(|boundary| !required_settlements.contains(boundary))
    {
        return Err(LoweringError::UnusedBoundarySettlement(*extra));
    }
    Ok(())
}
