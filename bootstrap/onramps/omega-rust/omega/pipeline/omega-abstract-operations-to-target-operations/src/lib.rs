use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;
use omega_target_operations::TargetOperationPlan;

mod boundary_policy;
mod code;
mod host;
mod instructions;
mod operands;
mod remap;
mod semantics;
#[cfg(test)]
mod tests;
mod translator;
mod values;

pub fn build_target_operation_plan(
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    host_calls: &HostCallPlan,
    abstract_operations: &AbstractOperationPlan,
) -> TargetOperationPlan {
    translator::build_target_operation_plan(target, host_abi, host_calls, abstract_operations)
}
