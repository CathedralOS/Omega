//! Optimizer module role: executable entrance. Stage entrance: bind admitted settlements, validate the complete roster, and lower each function.

mod borrowed_storage;
mod boundary_settlements;
mod control_flow;
mod coordination;
pub(crate) mod descriptor_input_guard;
mod function;
mod function_signature;
pub(crate) mod model;
pub(crate) mod optimized;
pub mod placed_view_inputs;
mod provider_evidence;
mod scalar;
mod scalar_abi;
pub(crate) mod selected_boundary_calls;
pub(super) mod structural_layout;
pub(crate) mod structural_signature;
mod structural_type_lookup;
mod unit;

use abstract_operations::AbstractOperationPlan;
use installation_evidence::ProviderInstallationEvidence;
use target::NativeTarget;
use target_operations::TargetOperationPlan;

use crate::{AdmittedBoundarySettlement, LoweringError};
use coordination::lower_to_target_operations_with_settlements_and_installation;

/// The admitted settlements a target lowering consumes beyond the plan and
/// its target. A source-free plan carries none; every roster is fail-closed.
pub(crate) struct TargetLoweringRequest<'a> {
    pub verified_input:
        Option<&'a terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput>,
    pub target: NativeTarget,
    /// Exact provider executions already admitted by the external-root ledger.
    pub settlements: &'a [AdmittedBoundarySettlement<'a>],
    /// Checked-provider installation evidence for the remaining boundaries.
    pub installation: Option<&'a dyn ProviderInstallationEvidence>,
    /// Exact target-owned native callback argument admissions. The admitted
    /// roster is retained on the returned plan itself in
    /// `native_callback_arguments`, joined to each consuming row by its
    /// Terminal operation.
    pub native_callbacks: &'a [crate::AdmittedNativeCallbackArgument],
}

/// Lower one abstract plan to target operations.
pub(crate) fn lower_to_target_operations(
    plan: &AbstractOperationPlan,
    request: TargetLoweringRequest<'_>,
) -> Result<TargetOperationPlan, LoweringError> {
    let TargetLoweringRequest {
        verified_input,
        target,
        settlements,
        installation,
        native_callbacks,
    } = request;
    let bindings = provider_evidence::bind_provider_executions(plan, settlements)?;
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        &bindings,
        installation,
        native_callbacks,
        verified_input,
    )
}
