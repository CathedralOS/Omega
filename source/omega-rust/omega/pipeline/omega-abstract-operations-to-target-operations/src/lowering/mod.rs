//! Optimizer module role: executable entrance. Stage entrance: bind admitted settlements, validate the complete roster, and lower each function.

mod boundary_settlements;
mod cleanup;
mod compatibility;
mod conditional_cleanup;
mod coordination;
mod function;
mod provider_evidence;
mod ranked_countdown;
mod scalar;
mod scalar_abi;
mod shared;
mod structural;
pub(super) mod structural_layout;
mod unit;

use omega_abstract_operations::AbstractOperationPlan;
use omega_installation_evidence::ProviderInstallationEvidence;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;
use omega_target_operations::TargetOperationPlanWithNativeCallbacks;

use crate::{AdmittedBoundarySettlement, LoweringError};
use coordination::{
    lower_to_target_operations_with_settlements,
    lower_to_target_operations_with_settlements_and_installation,
};

pub use compatibility::{
    lower_to_target_operations_with_provider_executions,
    lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma,
};

#[cfg(test)]
pub(crate) use coordination::lower_to_target_operations_with_settlements as lower_with_settlements_for_tests;
#[cfg(test)]
pub(crate) use coordination::{
    bind_native_callback_arguments as bind_native_callback_arguments_for_tests,
    validate_native_callback_target_rows as validate_native_callback_target_rows_for_tests,
};
#[cfg(test)]
pub(crate) use structural_layout::structural_shape as structural_shape_for_tests;

pub fn lower_to_target_operations(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
) -> Result<TargetOperationPlan, LoweringError> {
    lower_to_target_operations_with_settlements(plan, target, &[])
}

/// Lower the separately admitted exact native-ranked countdown without
/// widening the ordinary Unit or acyclic conditional-control families.
pub fn lower_ranked_to_target_operations(
    ranked: &omega_abstract_operations::RankedNativeAbstractOperationPlan,
    target: NativeTarget,
) -> Result<TargetOperationPlan, LoweringError> {
    ranked_countdown::lower(ranked, target)
}

/// Lower with checked-provider installation evidence and any remaining
/// external boundary settlements.
pub fn lower_to_target_operations_with_provider_executions_and_installation(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: Option<&dyn ProviderInstallationEvidence>,
) -> Result<TargetOperationPlan, LoweringError> {
    compatibility::lower_to_target_operations_with_provider_executions_installation_and_ieee_float_fma(
        plan, target, settlements, installation, &[],
    )
}

/// Lower the ordinary source-free plan while consuming exact target-owned
/// native callback argument admissions. Compatibility entrances above pass an
/// empty slice and therefore preserve their prior result type and behavior.
#[allow(clippy::too_many_arguments)]
pub fn lower_to_target_operations_with_provider_executions_installation_ieee_float_fma_and_native_callbacks(
    plan: &AbstractOperationPlan,
    target: NativeTarget,
    settlements: &[AdmittedBoundarySettlement<'_>],
    installation: Option<&dyn ProviderInstallationEvidence>,
    ieee_float_fma: &[crate::AdmittedIeeeFloatFmaSettlement<'_>],
    native_callbacks: &[crate::AdmittedNativeCallbackArgument],
) -> Result<TargetOperationPlanWithNativeCallbacks, LoweringError> {
    let bindings = provider_evidence::bind_provider_executions(plan, settlements)?;
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        &bindings,
        installation,
        ieee_float_fma,
        native_callbacks,
    )
}
