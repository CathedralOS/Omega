//! Compatibility entrances that close newer custody lanes with empty inputs.

use omega_abstract_operations::AbstractOperationPlan;
use omega_installation_evidence::ProviderInstallationEvidence;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

use crate::{AdmittedBoundarySettlement, LoweringError};

use super::{
    lower_to_target_operations_with_provider_executions_and_installation,
    lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks,
};

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

/// Lower with all ordinary provider/install settlements plus exact retained
/// nearest-FMA occurrence custody. This is the consuming entrance used by
/// Terminal native re-entry; simpler callers remain fail-closed by supplying
/// no FMA settlements through the compatibility entrances above.
pub fn lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: Option<&dyn ProviderInstallationEvidence>,
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
) -> Result<TargetOperationPlan, LoweringError> {
    Ok(
        lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks(
            plan,
            target,
            settlements,
            installation,
            ieee_float_fma,
            &[],
        )?
        .plan,
    )
}
