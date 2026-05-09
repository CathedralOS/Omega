use omega_native::plan::NativePlan;
use omega_native::runtime_storage::RuntimeStorageWrite;
use omega_runtime_text::places::expression_place_eq;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_storage::StateMutationLowering;

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

        let source_name = state_name(native_plan, local.source_key);
        blockers.insert(blocker(
            "state storage",
            &format!(
                "{} statement {} local `{}`: {} needs stack/local storage lowering",
                source_name, local.statement_index, local.name, local.type_name
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

        let source_name = state_name(native_plan, mutation.source_key);
        blockers.insert(blocker(
            "state mutation",
            &format!(
                "{} statement {} {:?}/{:?} `{}` = `{}` needs mutation lowering",
                source_name,
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

        let source_name = state_name(native_plan, slot.source_key);
        blockers.insert(blocker(
            "state storage",
            &format!(
                "#{} {} statement {} local `{}`: {} needs runtime frame slot layout",
                slot.dispatch_index, source_name, slot.statement_index, slot.name, slot.type_name
            ),
        ));
    }

    for (_, write) in native_plan.runtime_storage.writes.iter() {
        if runtime_storage_write_has_planned_text_write(native_plan, write) {
            continue;
        }

        let source_name = state_name(native_plan, write.source_key);
        blockers.insert(blocker(
            "state mutation",
            &format!(
                "#{} {} statement {} {:?}/{:?} `{}` = `{}` needs runtime storage write lowering",
                write.dispatch_index,
                source_name,
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
            expression_place_eq(&text_write.target, &write.target)
                && runtime_text_write_is_planned(native_plan, text_write)
        })
}

fn state_name(native_plan: &NativePlan, key: StateKey) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
