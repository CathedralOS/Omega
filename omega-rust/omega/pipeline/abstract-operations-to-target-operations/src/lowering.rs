//! Optimizer module role: executable entrance. Stage entrance: bind admitted settlements, validate the complete roster, and lower each function.

mod boundary_settlements;
mod control_flow;
mod coordination;
mod function;
mod function_signature;
pub(crate) mod model;
pub(crate) mod optimized;
pub(crate) mod placed_view_inputs;
mod provider_evidence;
mod scalar;
mod scalar_abi;
mod shared;
pub(super) mod structural_layout;
pub(crate) mod structural_signature;
mod structural_type_lookup;
mod unit;
mod unobserved_owned;

use abstract_operations::AbstractOperationPlan;
use installation_evidence::ProviderInstallationEvidence;
use target::NativeTarget;
use target_operations::TargetOperationPlan;

use crate::{AdmittedBoundarySettlement, LoweringError};
use coordination::lower_to_target_operations_with_settlements_and_installation;

#[cfg(test)]
pub(crate) use coordination::{
    lower_to_target_operations_with_settlements as lower_with_settlements_for_tests,
    native_callbacks::{
        bind_native_callback_arguments as bind_native_callback_arguments_for_tests,
        validate_native_callback_target_rows as validate_native_callback_target_rows_for_tests,
    },
};

/// The admitted settlements a target lowering consumes beyond the plan and
/// its target. A source-free plan carries none; every roster is fail-closed.
pub struct TargetLoweringRequest<'a> {
    pub target: NativeTarget,
    /// Exact provider executions already admitted by the external-root ledger.
    pub settlements: &'a [AdmittedBoundarySettlement<'a>],
    /// Checked-provider installation evidence for the remaining boundaries.
    pub installation: Option<&'a dyn ProviderInstallationEvidence>,
    /// Retained nearest-FMA occurrence custody.
    pub ieee_float_fma: &'a [crate::AdmittedIeeeFloatFmaSettlement<'a>],
    /// Exact target-owned native callback argument admissions. The admitted
    /// roster is retained on the returned plan itself in
    /// `native_callback_arguments`, joined to each consuming row by its
    /// Terminal operation.
    pub native_callbacks: &'a [crate::AdmittedNativeCallbackArgument],
}

impl TargetLoweringRequest<'_> {
    /// A lowering with no admitted settlements.
    pub fn new(target: NativeTarget) -> Self {
        Self {
            target,
            settlements: &[],
            installation: None,
            ieee_float_fma: &[],
            native_callbacks: &[],
        }
    }
}

/// Lower one abstract plan to target operations.
pub fn lower_to_target_operations(
    plan: &AbstractOperationPlan,
    request: TargetLoweringRequest<'_>,
) -> Result<TargetOperationPlan, LoweringError> {
    let TargetLoweringRequest {
        target,
        settlements,
        installation,
        ieee_float_fma,
        native_callbacks,
    } = request;
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
