use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_platform_interface::HostCallPlan;
use omega_target::NativeTarget;
use omega_target_operations::{TargetOperationFunction, TargetOperationPlan};

use crate::boundary_policy::validate_boundary_policies;
use crate::host;
use crate::instructions::translate_instruction;
use crate::operands::translate_operand;
use crate::remap;
use crate::values::translate_runtime_value_operand;

pub(crate) fn build_target_operation_plan(
    target: NativeTarget,
    host_abi: &HostAbiPlan,
    host_calls: &HostCallPlan,
    abstract_operations: &AbstractOperationPlan,
) -> TargetOperationPlan {
    let mut target_operations = TargetOperationPlan::with_capacity(
        target,
        abstract_operations.code.functions.len(),
        abstract_operations.code.instructions.len(),
        abstract_operations.code.operands.len(),
        abstract_operations.code.runtime_value_operands.len(),
    );

    for (_, operand) in abstract_operations.code.operands.iter() {
        target_operations
            .code
            .operands
            .insert(translate_operand(operand));
    }
    for (_, operand) in abstract_operations.code.runtime_value_operands.iter() {
        target_operations
            .code
            .runtime_value_operands
            .insert(translate_runtime_value_operand(operand));
    }

    for (_, instruction) in abstract_operations.code.instructions.iter() {
        target_operations
            .code
            .instructions
            .insert(translate_instruction(host_calls, instruction));
    }

    for (_, function) in abstract_operations.code.functions.iter() {
        target_operations
            .code
            .functions
            .insert(TargetOperationFunction {
                symbol: std::sync::Arc::clone(&function.symbol),
                source_key: function.source_key,
                instructions: remap::instruction_span(function.instructions),
            });
    }

    host::copy_runtime_text_host_bindings(host_abi, abstract_operations, &mut target_operations);
    target_operations.semantics = abstract_operations.semantics.clone();
    validate_boundary_policies(host_abi, &mut target_operations.semantics.boundaries);

    target_operations
}
