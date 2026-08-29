//! Stage entrance: bind admitted settlements, validate the complete roster, and lower each function.

mod boundary_settlements;
mod cleanup;
mod conditional_cleanup;
mod coordination;
mod function;
mod provider_evidence;
mod scalar;
mod shared;
mod structural;
pub(super) mod structural_layout;
mod unit;

use omega_abstract_operations::AbstractOperationPlan;
use omega_installation_evidence::ProviderInstallationEvidence;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

use crate::{AdmittedBoundarySettlement, LoweringError};
use coordination::{
    lower_to_target_operations_with_settlements,
    lower_to_target_operations_with_settlements_and_installation,
};

#[cfg(test)]
pub(crate) use coordination::lower_to_target_operations_with_settlements as lower_with_settlements_for_tests;
#[cfg(test)]
pub(crate) use structural_layout::structural_shape as structural_shape_for_tests;

pub fn lower_to_target_operations(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements(plan, target, &[])
}

/// Lower an effectful terminal plan using exact provider executions already
/// admitted by the external-root ledger.
pub fn lower_to_target_operations_with_provider_executions(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_provider_executions_and_installation(
        plan,
        target,
        settlements,
        None,
    )
}

/// Lower with checked-provider installation evidence and any remaining
/// external boundary settlements.
pub fn lower_to_target_operations_with_provider_executions_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: Option<&dyn ProviderInstallationEvidence>,
) -> Result<TargetOperationPlan, LoweringError> {
    let bindings = provider_evidence::bind_provider_executions(plan, settlements)?;
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        &bindings,
        installation,
    )
}
