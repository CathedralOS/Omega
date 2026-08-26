use crate::EmissionPlanningInput;
use omega_calling_conventions::HostBindingMechanism;
use omega_target::ObjectFormat;
use psi_arena::Arena;

use super::{
    EmissionBlocker, blocker,
    selected_instruction_queries::host_read_operation_key,
    semantic_scope::{proof_scope_suffix, state_name},
};

pub(super) fn collect_host_binding_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if input.target.object_format != ObjectFormat::MachO {
        return;
    }

    for (_, instruction) in input.instructions.code.instructions.iter() {
        let Some(operation_key) = host_read_operation_key(&instruction.kind) else {
            continue;
        };
        if !matches!(
            input
                .instructions
                .host_binding(operation_key)
                .map(|binding| &binding.mechanism),
            Some(HostBindingMechanism::Syscall { .. })
        ) {
            continue;
        }

        blockers.insert(blocker(
            "host binding",
            &format!(
                "{} statement {} uses direct Darwin stdin reads; route this through libSystem or compiler-owned line buffering before emitting a Mach-O executable{}",
                state_name(input, instruction.source_key),
                instruction.source_statement,
                proof_scope_suffix(input, instruction.source_key)
            ),
        ));
    }
}
