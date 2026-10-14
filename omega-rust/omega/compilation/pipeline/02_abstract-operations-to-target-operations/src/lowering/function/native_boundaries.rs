//! Scalar native-boundary route classification before result-shape dispatch.

use super::{
    AbstractFunction, BTreeMap, BoundaryMachineId, BoundarySettlementBinding, MachineId,
    OperationId, SelectedBoundaryCall,
};
use abstract_operations::AbstractOperation;
use target_operations::BoundaryRealization;
pub(super) fn unsupported_scalar_call(
    function: &AbstractFunction,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    selected_calls: &BTreeMap<(MachineId, OperationId, BoundaryMachineId), SelectedBoundaryCall>,
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
            && !selected_calls.contains_key(&(function.machine, *psi_operation, *boundary))
            && !matches!(
                settlements
                    .get(boundary)
                    .map(|binding| &binding.realization),
                Some(target_operations::BoundarySettlementRealization::Builtin(
                    BoundaryRealization::HostedExitProcessI32(_)
                        | BoundaryRealization::HostedWriteByteI32(_)
                        | BoundaryRealization::NearestIeeeFloatArithmetic(_)
                        | BoundaryRealization::PlacedScalarAccess(_)
                )) | Some(
                    target_operations::BoundarySettlementRealization::NormalizedForeignCall(_)
                )
            ))
        .then_some((*psi_operation, *boundary))
    })
}
