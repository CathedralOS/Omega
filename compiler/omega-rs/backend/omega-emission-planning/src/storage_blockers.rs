use crate::EmissionPlanningInput;
use omega_control_flow::StateKey;
use omega_runtime_storage::RuntimeStorageWrite;
use omega_runtime_text::places::expression_place_eq_across_tables;
use omega_state_storage::StateMutationLowering;
use omega_target_operations::SelectedInstructionKind;
use psi_arena::Arena;
use psi_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

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
    // A slot alone is not delivery: a local whose initializer is ARITHMETIC
    // (a Binary tree after peeling Mutable) needs a planned WRITE anchored at
    // its own (key, statement), or the slot stays ZII and every read is
    // silently wrong -- the inlined `let d: f64 = x - x` whose write planners
    // refused VANISHED with a clean compile (the expansion silent-drop hole).
    // The boundary is CORPUS-CENSUSED (2026-07-18, both dispatch and
    // straight-line paths, 1233 canaries + samples + EFI cross-target):
    // every legitimately-unplanned initializer is CALL-shaped (slice-view
    // builders, host calls, value-machine calls, builtins -- their delivery
    // lowers via call-result/host/descriptor machinery anchored elsewhere)
    // or a pure place path; pure-arithmetic initializers had ZERO unplanned
    // instances, so enforcing on exactly that class has zero false positives
    // and catches the proven silent-drop shape. Runs BEFORE the dispatch
    // early-return below: the dispatch path never walks locals otherwise.
    for (_, local) in input.state_storage.locals.iter() {
        if !local.required || !local.initial_value.is_valid() {
            continue;
        }
        if !initializer_is_arithmetic(&input.state_storage.expressions, local.initial_value) {
            continue;
        }
        // FLOAT-typed locals only: integer arithmetic initializers deliver
        // via folding/substitution (`let sum = arr[0] + a1 + px` is green
        // with no anchored write), while float arithmetic is the class whose
        // planner refusal proved to vanish silently. The corpus census found
        // ZERO unplanned float-arithmetic initializers, so this fires only
        // on true future vanishes.
        if !matches!(
            input
                .state_storage
                .type_references
                .display_name(local.type_reference)
                .as_str(),
            "f32" | "f64"
        ) {
            continue;
        }
        if state_mutation_is_planned(input, local.source_key, local.statement_index)
            || statement_covered_by_asm_storage_write(
                input,
                local.source_key,
                local.statement_index,
            )
        {
            continue;
        }
        let source_name = state_name(input, local.source_key);
        blockers.insert(blocker(
            "state storage",
            &format!(
                "{} statement {} local `{}` = `{}`{}: the float arithmetic initializer                  has no planned write -- the slot would stay zero-initialized and reads                  would be silently wrong; this shape needs its write lowering",
                source_name,
                local.statement_index,
                local.name,
                input
                    .state_storage
                    .expressions
                    .display_name(local.initial_value),
                proof_scope_suffix(input, local.source_key)
            ),
        ));
    }
    if needs_runtime_dispatch {
        collect_runtime_body_storage_blockers(input, blockers);
        return;
    }

    for (_, local) in input.state_storage.locals.iter() {
        if !local.required {
            continue;
        }

        if state_local_is_planned(input, local) {
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

        // #40 stopgap: refuse `arr[i] = arr[j]` (both runtime-indexed) BEFORE the
        // already-lowered skip, since a wrong-but-present write may have marked it
        // lowered. Never silently copy the array base.
        if mutation_is_dual_runtime_indexed(
            &input.state_storage.expressions,
            mutation.target,
            mutation.value,
        ) && !dual_indexed_copy_is_planned(input, mutation.source_key, mutation.statement_index)
        {
            let source_name = state_name(input, mutation.source_key);
            blockers.insert(blocker(
                "state mutation",
                &format!(
                    "{} statement {} `{}` = `{}`{} writes a runtime-indexed element from a \
                     runtime-indexed read, which is not yet supported (it would silently \
                     copy the array base); bind the source to a field temp first",
                    source_name,
                    mutation.statement_index,
                    input
                        .state_storage
                        .expressions
                        .display_name(mutation.target),
                    input.state_storage.expressions.display_name(mutation.value),
                    proof_scope_suffix(input, mutation.source_key)
                ),
            ));
            continue;
        }

        if mutation.lowering == StateMutationLowering::AlreadyLowered {
            continue;
        }

        if state_mutation_is_planned(input, mutation.source_key, mutation.statement_index) {
            continue;
        }

        // An `asm { in <dest>, <port> }` statement is an assignment whose value
        // is the `asm#port_in` call; instruction selection lowers it to a raw
        // PortRead into the destination place, so its residual mutation record
        // is legitimately unlowered.
        if statement_covered_by_asm_storage_write(
            input,
            mutation.source_key,
            mutation.statement_index,
        ) {
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

/// An ARITHMETIC initializer: a Binary tree after peeling Mutable. The
/// corpus-censused enforcement class -- calls, place paths, and literals all
/// deliver through machinery anchored outside the mutation-kind list, but an
/// arithmetic chain must have its own planned write.
fn initializer_is_arithmetic(
    expressions: &psi_checked_trees::expression::ExpressionTable,
    expression: psi_checked_trees::expression::ExpressionHandle,
) -> bool {
    use psi_checked_trees::expression::ExpressionNode;
    let mut current = expression;
    while let ExpressionNode::Mutable(inner) = expressions.expression(current) {
        current = *inner;
    }
    matches!(expressions.expression(current), ExpressionNode::Binary(_))
}

fn state_local_is_planned(
    input: &EmissionPlanningInput<'_>,
    local: &omega_state_storage::StateLocalStorage,
) -> bool {
    input.runtime_storage.frame_slots.iter().any(|(_, slot)| {
        state_key_matches_statement_source(slot.source_key, local.source_key)
            && slot.statement_index == local.statement_index
            && slot.symbol == local.symbol
            && matches!(
                slot.kind,
                omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
            )
    })
}

fn collect_runtime_body_storage_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, slot) in input.runtime_storage.frame_slots.iter() {
        if slot.byte_size > 0 || slot.is_static_boundary_capability {
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

        debug_unplanned_runtime_storage_write(input, write);

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

fn debug_unplanned_runtime_storage_write(
    input: &EmissionPlanningInput<'_>,
    write: &RuntimeStorageWrite,
) {
    if std::env::var_os("OMEGA_DEBUG_STORAGE_BLOCKERS").is_none() {
        return;
    }

    let source_name = state_name(input, write.source_key);
    eprintln!(
        "unplanned runtime storage write: #{} {} statement {} target `{}` value `{}`",
        write.dispatch_index,
        source_name,
        write.statement_index,
        input.runtime_storage.expressions.display_name(write.target),
        input.runtime_storage.expressions.display_name(write.value),
    );
    for (_, instruction) in input.instructions.code.instructions.iter() {
        if !state_key_matches_statement_source(instruction.source_key, write.source_key) {
            continue;
        }
        eprintln!(
            "  nearby selected write: statement {} kind {:?}",
            instruction.source_statement, instruction.kind
        );
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

/// True when statement `(source_key, statement_index)` emitted a raw PortRead
/// instruction (an `asm { in .. }`), whose destination store covers the
/// assignment's mutation record.
fn statement_covered_by_asm_storage_write(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            matches!(
                instruction.kind,
                SelectedInstructionKind::PortRead { .. }
                    | SelectedInstructionKind::FlagsSnapshot { .. }
                    | SelectedInstructionKind::MsrRead { .. }
                    | SelectedInstructionKind::ControlRegisterRead { .. }
            ) && (instruction.source_key == source_key
                || (instruction.source_key.machine == source_key.machine
                    && instruction.source_key.state == source_key.state))
                && instruction.source_statement == statement_index
        })
}

fn state_mutation_is_planned(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            if !state_key_matches_statement_source(instruction.source_key, source_key)
                || instruction.source_statement != statement_index
            {
                return false;
            }

            matches!(
                instruction.kind,
                SelectedInstructionKind::AtomicLoad { .. }
                    | SelectedInstructionKind::AtomicStore { .. }
                    | SelectedInstructionKind::AtomicFetchAdd { .. }
                    | SelectedInstructionKind::AtomicFetchSub { .. }
                    | SelectedInstructionKind::AtomicFetchXor { .. }
                    | SelectedInstructionKind::AtomicFetchOr { .. }
                    | SelectedInstructionKind::AtomicFetchAnd { .. }
                    | SelectedInstructionKind::AtomicSwap { .. }
                    | SelectedInstructionKind::HostOperation { .. }
                    | SelectedInstructionKind::AtomicCompareExchange { .. }
                    | SelectedInstructionKind::WritePlaceInteger { .. }
                    | SelectedInstructionKind::WriteStorageBitField { .. }
                    | SelectedInstructionKind::WritePlaceBinary { .. }
                    | SelectedInstructionKind::WritePlaceString { .. }
                    | SelectedInstructionKind::WritePlaceBoundedBuffer { .. }
                    | SelectedInstructionKind::WritePlaceAddress { .. }
                    | SelectedInstructionKind::WriteRuntimeStorageConvert { .. }
                    | SelectedInstructionKind::WritePlaceConvert { .. }
                    | SelectedInstructionKind::AppendPlaceBoundedBufferSource { .. }
                    | SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { .. }
                    | SelectedInstructionKind::MaterializeTextBufferToPlace { .. }
                    | SelectedInstructionKind::AppendTextStoredToPlace { .. }
                    | SelectedInstructionKind::AppendTextLiteralToPlace { .. }
                    | SelectedInstructionKind::AppendRuntimeTextStoredSuffix { .. }
                    | SelectedInstructionKind::CopyPlaces { .. }
            )
        })
}

/// True if `handle` is a RUNTIME-indexed access `arr[i]` whose index is NOT a compile-
/// time integer literal. A fixed index (`arr[5]`) lowers correctly, so it returns false.
fn expression_is_runtime_indexed(expressions: &ExpressionTable, handle: ExpressionHandle) -> bool {
    let ExpressionNode::Indexed(indexed) = expressions.expression(handle) else {
        return false;
    };
    let mut index = indexed.index;
    while let ExpressionNode::Mutable(inner) = expressions.expression(index) {
        index = *inner;
    }
    !matches!(expressions.expression(index), ExpressionNode::Integer(_))
}

/// True when selection planned the REAL dual-indexed copy instruction for this
/// statement (task #38). Only then may the #40 stopgap below stand down: any
/// dual shape the dual-copy arm does NOT catch would fall to the legacy path
/// that silently copies the array base, so the fence must stay for those.
fn dual_indexed_copy_is_planned(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            state_key_matches_statement_source(instruction.source_key, source_key)
                && instruction.source_statement == statement_index
                && match &instruction.kind {
                    // Rung 2c-x: the pair rides CopyPlaces -- BOTH sides
                    // carry a runtime index, which is exactly the dual
                    // shape this fence guards.
                    SelectedInstructionKind::CopyPlaces { source, target, .. } => {
                        let indexed = |place: &omega_target_operations::Place| {
                            place.steps().iter().any(|step| {
                                matches!(
                                    step,
                                    omega_target_operations::PlaceStep::ScaledIndex { .. }
                                )
                            })
                        };
                        indexed(source) && indexed(target)
                    }
                    _ => false,
                }
        })
}

/// SOUNDNESS STOPGAP (#40): `arr[i] = arr[j]` with BOTH the target AND the value
/// runtime-indexed is not yet correctly lowerable -- the value resolves to the array
/// BASE, so it silently copies `arr[0]` (or no-ops) with no error. A wrong-but-present
/// write instruction is selected, so the planned-check passes and the miscompile ships.
/// Detect the pattern from the expressions and refuse it here so it errors cleanly; the
/// sound workaround is a field temp (`self.t = arr[j]; arr[i] = self.t`).
fn mutation_is_dual_runtime_indexed(
    expressions: &ExpressionTable,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> bool {
    expression_is_runtime_indexed(expressions, target)
        && expression_is_runtime_indexed(expressions, value)
}

fn state_key_matches_statement_source(actual: StateKey, expected: StateKey) -> bool {
    actual == expected || (actual.machine == expected.machine && actual.state == expected.state)
}
