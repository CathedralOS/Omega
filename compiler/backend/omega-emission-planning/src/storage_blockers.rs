use crate::EmissionPlanningInput;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_runtime_storage::RuntimeStorageWrite;
use omega_runtime_text::places::expression_place_eq_across_tables;
use omega_state_storage::StateMutationLowering;
use omega_target_operations::SelectedInstructionKind;

use super::runtime_text_blockers::{
    runtime_text_write_for_statement, runtime_text_write_is_planned,
};
use super::semantic_scope::{invariant_suffix, proof_scope_suffix, state_name};
use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_storage_blockers(
    input: &EmissionPlanningInput<'_>,
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if needs_runtime_dispatch {
        collect_runtime_body_storage_blockers(input, blockers);
        return;
    }

    for (_, local) in input.state_storage.locals.iter() {
        if !local.required {
            continue;
        }

        let source_name = state_name(input, local.source_key);
        blockers.insert(blocker(
            "state storage",
            &format!(
                "{} statement {} local `{}`: {}{}{} needs stack/local storage lowering",
                source_name,
                local.statement_index,
                local.name,
                input
                    .state_storage
                    .type_references
                    .display_name(local.type_reference),
                invariant_suffix(&input.state_storage.invariant_names, local.invariant_names),
                proof_scope_suffix(input, local.source_key)
            ),
        ));
    }

    for (_, mutation) in input.state_storage.mutations.iter() {
        if !mutation.required {
            continue;
        }

        if mutation.lowering == StateMutationLowering::AlreadyLowered {
            continue;
        }

        if state_mutation_is_planned(input, mutation.source_key, mutation.statement_index) {
            continue;
        }

        let source_name = state_name(input, mutation.source_key);
        blockers.insert(blocker(
            "state mutation",
            &format!(
                "{} statement {} {:?}/{:?} `{}` = `{}`{} needs mutation lowering",
                source_name,
                mutation.statement_index,
                mutation.mutation_kind,
                mutation.lowering,
                input
                    .state_storage
                    .expressions
                    .display_name(mutation.target),
                input.state_storage.expressions.display_name(mutation.value),
                proof_scope_suffix(input, mutation.source_key)
            ),
        ));
    }
}

fn collect_runtime_body_storage_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, slot) in input.runtime_storage.frame_slots.iter() {
        if slot.byte_size > 0 {
            continue;
        }

        let source_name = state_name(input, slot.source_key);
        blockers.insert(blocker(
            "state storage",
            &format!(
                "#{} {} statement {} local `{}`: {}{}{} needs runtime frame slot layout",
                slot.dispatch_index,
                source_name,
                slot.statement_index,
                slot.name,
                slot.type_name,
                invariant_suffix(&input.runtime_storage.invariant_names, slot.invariant_names),
                proof_scope_suffix(input, slot.source_key)
            ),
        ));
    }

    for (_, write) in input.runtime_storage.writes.iter() {
        if runtime_storage_write_is_planned(input, write) {
            continue;
        }

        let source_name = state_name(input, write.source_key);
        blockers.insert(blocker(
            "state mutation",
            &format!(
                "#{} {} statement {} {:?}/{:?} `{}` = `{}`{} needs runtime storage write lowering",
                write.dispatch_index,
                source_name,
                write.statement_index,
                write.mutation_kind,
                write.lowering,
                input.runtime_storage.expressions.display_name(write.target),
                input.runtime_storage.expressions.display_name(write.value),
                proof_scope_suffix(input, write.source_key)
            ),
        ));
    }
}

fn runtime_storage_write_has_planned_text_write(
    input: &EmissionPlanningInput<'_>,
    write: &RuntimeStorageWrite,
) -> bool {
    runtime_text_write_for_statement(input, write.source_key, write.statement_index).is_some_and(
        |text_write| {
            expression_place_eq_across_tables(
                &input.runtime_text.expressions,
                text_write.target,
                &input.runtime_storage.expressions,
                write.target,
            ) && runtime_text_write_is_planned(input, text_write)
        },
    )
}

fn runtime_storage_write_is_planned(
    input: &EmissionPlanningInput<'_>,
    write: &RuntimeStorageWrite,
) -> bool {
    runtime_storage_write_has_planned_text_write(input, write)
        || state_mutation_is_planned(input, write.source_key, write.statement_index)
}

fn state_mutation_is_planned(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input.instructions.instructions.iter().any(|(_, instruction)| {
        if !state_key_matches_statement_source(instruction.source_key, source_key)
            || instruction.source_statement != statement_index
        {
            return false;
        }

            matches!(
                instruction.kind,
                SelectedInstructionKind::WriteRuntimeMachineInteger { .. }
                    | SelectedInstructionKind::WriteRuntimeStorageInteger { .. }
                    | SelectedInstructionKind::WriteRuntimePointeeInteger { .. }
                    | SelectedInstructionKind::WriteRuntimeStorageBinary { .. }
                    | SelectedInstructionKind::WriteRuntimePointeeBinary { .. }
                    | SelectedInstructionKind::WriteRuntimeFrameIndexedInteger { .. }
                    | SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { .. }
                    | SelectedInstructionKind::WriteRuntimeMachineString { .. }
                    | SelectedInstructionKind::WriteRuntimePointeeString { .. }
                    | SelectedInstructionKind::WriteRuntimeFrameIndexedString { .. }
                    | SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame { .. }
                    | SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame { .. }
                    | SelectedInstructionKind::MaterializeRuntimeTextBuffer { .. }
                    | SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimePointee { .. }
                    | SelectedInstructionKind::MaterializeRuntimeTextBufferToRuntimeFrameIndexed { .. }
                    | SelectedInstructionKind::AppendRuntimeTextStoredPlace { .. }
                    | SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimePointee { .. }
                    | SelectedInstructionKind::AppendRuntimeTextStoredPlaceToRuntimeFrameIndexed { .. }
                    | SelectedInstructionKind::AppendRuntimeTextLiteral { .. }
                    | SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimePointee { .. }
                    | SelectedInstructionKind::AppendRuntimeTextLiteralToRuntimeFrameIndexed { .. }
                    | SelectedInstructionKind::AppendRuntimeTextStoredSuffix { .. }
                    | SelectedInstructionKind::CopyRuntimeStorage { .. }
                    | SelectedInstructionKind::CopyRuntimeStorageToRuntimeFrameIndexed { .. }
                    | SelectedInstructionKind::CopyRuntimeFrameIndexedToRuntimeFrame { .. }
                    | SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame { .. }
                    | SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee { .. }
            )
        })
}

fn state_key_matches_statement_source(actual: StateKey, expected: StateKey) -> bool {
    actual == expected || (actual.machine == expected.machine && actual.state == expected.state)
}
