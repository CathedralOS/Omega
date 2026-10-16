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
    } = request;
    let bindings = provider_evidence::bind_provider_executions(plan, settlements)?;
    lower_to_target_operations_with_settlements_and_installation(
        plan,
        target,
        &bindings,
        installation,
        verified_input
            .map(|input| input.admitted_native_callbacks())
            .unwrap_or_default(),
        verified_input,
    )
}
