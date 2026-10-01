//! The boundary-settlement roster contract beside the lowering
//! coordinator: index the caller-supplied settlement bindings, then rejoin
//! them fail-closed against the plan's boundary calls and the
//! installation's admitted provider calls — every required boundary settled
//! exactly once, none settled twice, none unused, and no overlap with an
//! installed provider.

use super::installed_provider_calls::{BoundaryCallsByKey, InstalledCallsByCall};
use crate::LoweringError;
use abstract_operations::AbstractOperationPlan;
use semantic_vocabulary::BoundaryMachineId;
use std::collections::{BTreeMap, BTreeSet};
use target_operations::BoundarySettlementBinding;

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
    // An admitted kernel binding is not an imported-call encoding and cannot
    // silently become a no-code settlement. Keep the missing native mechanism
    // explicit for every call shape, including argument-free unit calls.
    if let Some(binding) = settlements_by_boundary.values().find(|binding| {
        matches!(
            binding.realization,
            target_operations::BoundarySettlementRealization::SystemCall(_)
        )
    }) {
        return Err(LoweringError::UnsupportedSystemCallRealization(
            binding.boundary,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
    use calling_conventions::{
        CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
    };
    use semantic_vocabulary::{MachineId, OperationId, ValueId};
    use target_operations::{
        BoundarySettlementRealization, NormalizedSystemCallBinding, ProviderExecutionBinding,
        ProviderPlanReportIdentity,
    };

    #[test]
    fn admitted_syscalls_cannot_fall_through_to_no_code_settlements() {
        let boundary = BoundaryMachineId::new(1).unwrap();
        let operation = OperationId::new(1).unwrap();
        let machine = MachineId::new(1).unwrap();
        for parameter_count in [0, 3] {
            let signature = CallSignature {
                parameters: vec![ValueShape::integer(8, 8); parameter_count],
                result: None,
            };
            let entry = evaluate_ordinary_boundary_entry_plan(
                CallingPolicy::LinuxSyscallX86_64,
                &signature,
            )
            .unwrap();
            let execution = ProviderExecutionBinding::from_execution_record(
                ProviderPlanReportIdentity::new(1).unwrap(),
                2,
                3,
                4,
                5,
            )
            .unwrap();
            let settlement = BoundarySettlementBinding {
                boundary,
                execution: execution.into(),
                realization: BoundarySettlementRealization::SystemCall(
                    NormalizedSystemCallBinding {
                        target: target::TargetProfile::LinuxX64.native_target(),
                        number: 7,
                        boundary_entry_plan: entry.plan().clone(),
                    },
                ),
            };
            let call = AbstractOperation::BoundaryCall {
                psi_operation: operation,
                boundary,
                result: AbstractBoundaryResult::Unit,
                arguments: (0..parameter_count)
                    .map(|position| ValueId::new(position as u64 + 1).unwrap())
                    .collect(),
                structural_arguments: Vec::new(),
                completion_claim_sources: Vec::new(),
                completion_receipts: Vec::new(),
            };
            assert_eq!(
                validate_settlement_roster(
                    &BTreeMap::from([(boundary, settlement)]),
                    &BTreeMap::new(),
                    &BTreeMap::from([((machine, operation, boundary), &call)]),
                ),
                Err(LoweringError::UnsupportedSystemCallRealization(boundary))
            );
        }
    }
}
