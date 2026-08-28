use crate::MachineEmissionContext;
use crate::emitter::MachineEmissionInput;
use crate::instruction_bytes::emit_function_bytes;
use omega_machine_bytes::{EncodedMachineCode, EncodedMachineFunction, EncodedMachinePlan};
use psi_diagnostics::Diagnostic;

pub(crate) fn build_encoded_machine_code(
    input: &MachineEmissionInput<'_, '_>,
) -> Result<EncodedMachineCode, Diagnostic> {
    let EncodedMachinePlan { mut code, .. } = EncodedMachinePlan::with_capacity(
        input.target,
        input.machine_instructions.code.functions.len(),
        input.machine_instructions.code.instructions.len(),
        0,
    );
    code.runtime_value_operands.reserve(
        input
            .assigned_target_operations
            .code
            .runtime_value_operands
            .len(),
    );
    for (_, operand) in input
        .assigned_target_operations
        .code
        .runtime_value_operands
        .iter()
    {
        code.runtime_value_operands.insert(operand.kind.clone());
    }

    for (_, function) in input.machine_instructions.code.functions.iter() {
        let byte_offset = code.bytes.len();
        let instructions = emit_function_bytes(
            MachineEmissionContext {
                target: input.target,
                assigned_target_operations: input.assigned_target_operations,
                host_abi: input.host_abi,
                data: input.data,
                terminal_dispatch_index: input.terminal_dispatch_index,
            },
            input.machine_instructions,
            &mut code,
            function.instructions,
        )?;
        let byte_count = code.bytes.len() - byte_offset;
        code.functions.insert(EncodedMachineFunction {
            symbol: std::sync::Arc::clone(&function.symbol),
            identity: function.identity,
            byte_offset,
            byte_count,
            instructions,
        });
    }

    code.byte_count = code.bytes.len();
    Ok(code)
}
