use crate::plan::NativePlan;
use omega_core::arena::Arena;
use omega_target::ObjectFormat;
use omega_target_program::SelectedInstructionKind;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_host_binding_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if native_plan.target.object_format != ObjectFormat::MachO {
        return;
    }

    for (_, instruction) in native_plan.instructions.instructions.iter() {
        if !matches!(
            instruction.kind,
            SelectedInstructionKind::ReadRuntimeTextLine { .. }
        ) {
            continue;
        }

        blockers.insert(blocker(
            "host binding",
            &format!(
                "{} statement {} uses direct Darwin stdin reads; route this through libSystem or compiler-owned line buffering before emitting a Mach-O executable",
                state_name(native_plan, instruction.source_key),
                instruction.source_statement
            ),
        ));
    }
}

fn state_name(native_plan: &NativePlan, key: omega_control_flow::StateKey) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
