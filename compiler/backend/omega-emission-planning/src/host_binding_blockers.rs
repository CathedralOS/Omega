use crate::EmissionPlanningInput;
use omega_core::arena::Arena;
use omega_target::ObjectFormat;
use omega_target_program::{RuntimeTextReadSource, SelectedInstructionKind};

use super::{EmissionBlocker, blocker};

pub(super) fn collect_host_binding_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if input.target.object_format != ObjectFormat::MachO {
        return;
    }

    for (_, instruction) in input.instructions.instructions.iter() {
        if !matches!(
            instruction.kind,
            SelectedInstructionKind::ReadRuntimeTextLine {
                source: RuntimeTextReadSource::Syscall { .. },
                ..
            }
        ) {
            continue;
        }

        blockers.insert(blocker(
            "host binding",
            &format!(
                "{} statement {} uses direct Darwin stdin reads; route this through libSystem or compiler-owned line buffering before emitting a Mach-O executable",
                state_name(input, instruction.source_key),
                instruction.source_statement
            ),
        ));
    }
}

fn state_name(input: &EmissionPlanningInput<'_>, key: omega_control_flow::StateKey) -> String {
    input
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
