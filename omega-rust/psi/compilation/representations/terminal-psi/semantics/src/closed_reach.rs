//! Source-independent call-closure queries for retained closed-reach applications.

use semantic_vocabulary::MachineId;
use terminal_psi::{
    ClosedReachMachineBinding, ClosedReachParameter, OperationKind, TerminalModule,
};

/// Whether an ordinary call closure contains an exact selected schema application.
pub fn has_schema_application_in_call_closure(
    module: &TerminalModule,
    owner: MachineId,
    selected: &ClosedReachMachineBinding,
) -> bool {
    let mut reachable = vec![owner];
    let mut next_machine_position = 0;
    while let Some(machine_id) = reachable.get(next_machine_position).copied() {
        next_machine_position += 1;
        let Some(machine) = module
            .machines
            .iter()
            .find(|machine| machine.id == machine_id)
        else {
            continue;
        };
        if let Some(application) = &machine.closed_reach_application {
            for call in &application.calls {
                let Some(ClosedReachParameter::Machine(binding)) =
                    application.telescope.get(call.binder as usize)
                else {
                    continue;
                };
                if call.application.is_some()
                    && binding.selected_identity == selected.selected_identity
                    && binding.selected_contract_commitment == selected.selected_contract_commitment
                    && binding.selected_reach == selected.selected_reach
                    && binding.schema == selected.schema
                {
                    return true;
                }
            }
        }
        for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
            let callee = match operation.kind {
                OperationKind::Call { callee, .. }
                | OperationKind::CallUnit { callee, .. }
                | OperationKind::CallStructuralScalar { callee, .. }
                | OperationKind::CallStructural { callee, .. }
                | OperationKind::CallStructuralWithScalarArguments { callee, .. } => callee,
                _ => continue,
            };
            if !reachable.contains(&callee) {
                reachable.push(callee);
            }
        }
    }
    false
}
