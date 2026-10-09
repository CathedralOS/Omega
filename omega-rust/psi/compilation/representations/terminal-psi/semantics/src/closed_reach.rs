//! Source-independent call-closure queries for retained closed-reach applications.

use semantic_vocabulary::MachineId;
use terminal_psi::{ClosedReachMachineBinding, ClosedReachParameter, TerminalModule};

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
        let Some(machine) = module.machine(machine_id) else {
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
        for operation in machine.operations() {
            let Some(callee) = operation.kind.direct_callee() else {
                continue;
            };
            if !reachable.contains(&callee) {
                reachable.push(callee);
            }
        }
    }
    false
}
