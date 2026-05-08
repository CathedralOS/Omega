use crate::plan::NativePlan;
use crate::runtime_storage::RuntimeStorageWrite;
use crate::state_storage::StateMutationLowering;
use omega_core::arena::Arena;

use super::runtime_text_blockers::{
    runtime_text_write_for_statement, runtime_text_write_is_planned,
};
use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_storage_blockers(
    native_plan: &NativePlan,
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if needs_runtime_dispatch {
        collect_runtime_body_storage_blockers(native_plan, blockers);
        return;
    }

    for (_, local) in native_plan.state_storage.locals.iter() {
        if !local.required {
            continue;
        }

        blockers.insert(blocker(
            "state storage",
            &format!(
                "{}.{} statement {} local `{}`: {} needs stack/local storage lowering",
                local.machine, local.state, local.statement_index, local.name, local.type_name
            ),
        ));
    }

    for (_, mutation) in native_plan.state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        if mutation.lowering == StateMutationLowering::AlreadyLowered {
            continue;
        }

        blockers.insert(blocker(
            "state mutation",
            &format!(
                "{}.{} statement {} {:?}/{:?} `{}` = `{}` needs mutation lowering",
                mutation.machine,
                mutation.state,
                mutation.statement_index,
                mutation.mutation_kind,
                mutation.lowering,
                mutation.target.display_name(),
                mutation.value.display_name()
            ),
        ));
    }
}

fn collect_runtime_body_storage_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, slot) in native_plan.runtime_storage.frame_slots.iter() {
        if slot.byte_size > 0 {
            continue;
        }

        blockers.insert(blocker(
            "state storage",
            &format!(
                "#{} {}.{} statement {} local `{}`: {} needs runtime frame slot layout",
                slot.dispatch_index,
                slot.source_machine,
                slot.source_state,
                slot.statement_index,
                slot.name,
                slot.type_name
            ),
        ));
    }

    for (_, write) in native_plan.runtime_storage.writes.iter() {
        if runtime_storage_write_has_planned_text_write(native_plan, write) {
            continue;
        }

        blockers.insert(blocker(
            "state mutation",
            &format!(
                "#{} {}.{} statement {} {:?}/{:?} `{}` = `{}` needs runtime storage write lowering",
                write.dispatch_index,
                write.source_machine,
                write.source_state,
                write.statement_index,
                write.mutation_kind,
                write.lowering,
                write.target.display_name(),
                write.value.display_name()
            ),
        ));
    }
}

fn runtime_storage_write_has_planned_text_write(
    native_plan: &NativePlan,
    write: &RuntimeStorageWrite,
) -> bool {
    runtime_text_write_for_statement(native_plan, write.source_key, write.statement_index)
        .is_some_and(|text_write| {
            text_write.target.display_name() == write.target.display_name()
                && runtime_text_write_is_planned(native_plan, text_write)
        })
}
