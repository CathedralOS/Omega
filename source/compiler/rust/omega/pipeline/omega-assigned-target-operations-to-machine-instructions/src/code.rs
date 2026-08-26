use omega_assigned_target_operations::AssignedTargetOperationPlan;
use omega_machine_instructions::{
    MachineInstructionCode, MachineInstructionFunction, MachineInstructionPlan,
};
use psi_diagnostics::Diagnostic;
use std::collections::HashMap;

use crate::functions;

pub(crate) fn build_machine_instruction_code(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<MachineInstructionCode, Diagnostic> {
    let function_identities = validate_function_identities(assigned_target_operations)?;
    validate_internal_call_targets(assigned_target_operations, &function_identities)?;
    let MachineInstructionPlan { mut code, .. } = MachineInstructionPlan::with_capacity(
        assigned_target_operations.target,
        assigned_target_operations.code.functions.len(),
        assigned_target_operations.code.instructions.len(),
    );

    for (_, function) in assigned_target_operations.code.functions.iter() {
        let function_instructions = functions::append_machine_instructions(
            assigned_target_operations,
            function,
            &mut code.instructions,
        )?;

        code.functions.insert(MachineInstructionFunction {
            symbol: std::sync::Arc::clone(&function.symbol),
            identity: function.identity,
            instructions: function_instructions,
        });
    }

    Ok(code)
}

/// Machine lowering is the first target-instruction boundary. Reject an
/// invalid or aliased compiler-private function role before any instruction
/// body is selected, rather than serializing ambiguous identity into bytes and
/// relying on later object planning to discover it.
fn validate_function_identities(
    assigned_target_operations: &AssignedTargetOperationPlan,
) -> Result<HashMap<omega_control_flow::MachineFunctionIdentity, usize>, Diagnostic> {
    let functions = assigned_target_operations.code.functions.storage_slice();
    let mut identities = HashMap::with_capacity(functions.len());
    for (function_index, function) in functions.iter().enumerate() {
        if !function.identity.is_valid() {
            return Err(Diagnostic::error(format!(
                "assigned function `{}` has invalid compiler-private identity {:?}",
                function.symbol, function.identity
            )));
        }
        if let Some(earlier_index) = identities.insert(function.identity, function_index) {
            let earlier = &functions[earlier_index];
            return Err(Diagnostic::error(format!(
                "assigned functions `{}` and `{}` share compiler-private identity {:?}",
                earlier.symbol, function.symbol, function.identity
            )));
        }
    }
    Ok(identities)
}

fn validate_internal_call_targets(
    assigned_target_operations: &AssignedTargetOperationPlan,
    function_identities: &HashMap<omega_control_flow::MachineFunctionIdentity, usize>,
) -> Result<(), Diagnostic> {
    for (_, function) in assigned_target_operations.code.functions.iter() {
        let Some(instructions) = assigned_target_operations
            .code
            .instructions
            .span(function.instructions)
        else {
            continue;
        };
        for instruction in instructions {
            let omega_assigned_target_operations::AssignedOperationKind::CallInternalFunction {
                target,
            } = &instruction.kind
            else {
                continue;
            };
            if !function_identities.contains_key(target) {
                return Err(Diagnostic::error(format!(
                    "assigned function `{}` calls missing compiler-private function identity {:?}",
                    function.symbol, target
                )));
            }
        }
    }
    Ok(())
}
