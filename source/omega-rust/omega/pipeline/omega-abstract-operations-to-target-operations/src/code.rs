use omega_abstract_operations::AbstractOperationPlan;
use omega_calling_conventions::HostAbiPlan;
use omega_platform_interface::HostCallPlan;
use omega_target_operations::{TargetOperationCode, TargetOperationFunction};
use psi_arena::Arena;

use crate::host;
use crate::instructions::translate_instruction;
use crate::operands::translate_operand;
use crate::remap;
use crate::values::translate_runtime_value_operand;

pub(crate) fn build_target_operation_code(
    host_abi: &HostAbiPlan,
    host_calls: &HostCallPlan,
    abstract_operations: &AbstractOperationPlan,
) -> Result<TargetOperationCode, psi_diagnostics::Diagnostic> {
    let mut code = TargetOperationCode {
        functions: Arena::with_capacity(abstract_operations.code.functions.len()),
        instructions: Arena::with_capacity(abstract_operations.code.instructions.len()),
        operands: Arena::with_capacity(abstract_operations.code.operands.len()),
        runtime_value_operands: Arena::with_capacity(
            abstract_operations.code.runtime_value_operands.len(),
        ),
        host_bindings: Arena::new(),
    };

    for (_, operand) in abstract_operations.code.operands.iter() {
        code.operands.insert(translate_operand(operand));
    }
    for (_, operand) in abstract_operations.code.runtime_value_operands.iter() {
        code.runtime_value_operands
            .insert(translate_runtime_value_operand(operand));
    }

    for (_, instruction) in abstract_operations.code.instructions.iter() {
        code.instructions.insert(translate_instruction(
            host_calls,
            abstract_operations,
            instruction,
        )?);
    }

    for (_, function) in abstract_operations.code.functions.iter() {
        code.functions.insert(TargetOperationFunction {
            symbol: std::sync::Arc::clone(&function.symbol),
            identity: function.identity,
            instructions: remap::instruction_span(function.instructions),
        });
    }

    host::copy_runtime_text_host_bindings(host_abi, abstract_operations, &mut code);
    Ok(code)
}
