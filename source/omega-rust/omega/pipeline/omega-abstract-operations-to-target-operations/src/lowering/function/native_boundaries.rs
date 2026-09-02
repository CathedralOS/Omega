//! Scalar native-boundary route classification before result-shape dispatch.

use super::*;

pub(super) fn has_installed_scalar_call(
    function: &AbstractFunction,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
) -> bool {
    function.operations.iter().any(|operation| {
        matches!(operation,
            AbstractOperation::BoundaryCall {
                psi_operation,
                boundary,
                arguments,
                ..
            } if !arguments.is_empty()
                && installed_calls.contains_key(&(function.machine, *psi_operation, *boundary)))
    })
}

pub(super) fn unsupported_scalar_call(
    function: &AbstractFunction,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderUnitCallEvidence,
    >,
) -> Option<(OperationId, BoundaryMachineId)> {
    function.operations.iter().find_map(|operation| {
        let AbstractOperation::BoundaryCall {
            psi_operation,
            boundary,
            arguments,
            ..
        } = operation
        else {
            return None;
        };
        (!arguments.is_empty()
            && !installed_calls.contains_key(&(function.machine, *psi_operation, *boundary))
            && !matches!(
                settlements
                    .get(boundary)
                    .map(|binding| &binding.realization),
                Some(
                    omega_target_operations::BoundarySettlementRealization::Builtin(
                        BoundaryRealization::LinuxExitGroupI32(_)
                    )
                ) | Some(
                    omega_target_operations::BoundarySettlementRealization::NormalizedForeignCall(
                        _
                    )
                )
            ))
        .then_some((*psi_operation, *boundary))
    })
}
