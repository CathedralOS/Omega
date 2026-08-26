use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

use crate::code::build_target_operation_code;
use crate::semantics::build_target_semantic_summary;

pub(crate) fn build_target_operation_plan(
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    host_calls: &HostCallPlan,
    abstract_operations: &AbstractOperationPlan,
) -> Result<TargetOperationPlan, psi_diagnostics::Diagnostic> {
    Ok(TargetOperationPlan::with_roots(
        target,
        build_target_operation_code(host_abi, host_calls, abstract_operations)?,
        build_target_semantic_summary(host_abi, abstract_operations),
    ))
}
