//! Post-selection verification for slice-descriptor (subslice) arguments.
//!
//! A dispatch transition argument like `self.verify(sub[start..])` must
//! materialize a fat descriptor into the callee's parameter slot. When
//! instruction selection cannot lower the subslice (e.g. a computed bound such
//! as `sub[offset + 1..]`), the argument-materialization strategies all decline
//! and the parameter slot silently keeps its previous bytes — the callee then
//! reads a stale descriptor and the failure surfaces as wrong runtime behavior
//! far from the cause. This pass turns that silent drop into a loud blocker.
//!
//! ## Correlation and conservatism
//!
//! Following `required_emission_verification`'s precedent, the check is
//! deliberately conservative: a range-indexed (subslice) argument bound to a
//! sized parameter slot is "covered" as soon as ANY selected instruction with
//! the transition's statement index writes into the slot's byte range. Alias
//! resolution can rewrite the instructions' source KEYS, so the correlation is
//! by statement index + target byte range only — this can under-report (a
//! colliding statement index in another state writing the same slot), never
//! false-positive on a correctly lowered program.

use crate::EmissionPlanningInput;
use crate::semantic_scope::{proof_scope_suffix, state_name};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_control_flow::StateKey;
use omega_core::arena::{Arena, HandleSpan};
use omega_target_operations::{RuntimeStorageRegion, SelectedInstructionKind};

use super::{EmissionBlocker, blocker};

pub(super) fn collect_descriptor_argument_blockers(
    input: &EmissionPlanningInput<'_>,
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    // Subslice arguments only flow through parameter frame slots on the
    // runtime-dispatch lowering path; straight-line schedules inline state
    // bodies and are covered by the storage blockers.
    if !needs_runtime_dispatch {
        return;
    }

    collect_subslice_local_initializer_blockers(input, blockers);

    for (_, case) in input.runtime_dispatch_loop.cases.iter() {
        let Some(edges) = input.runtime_dispatch_loop.edges.span(case.edges) else {
            continue;
        };
        for edge in edges {
            verify_subslice_arguments_materialized(
                input,
                case.key,
                edge.statement_index,
                edge.target_dispatch_index,
                edge.target_arguments,
                blockers,
            );
            verify_subslice_arguments_materialized(
                input,
                case.key,
                edge.statement_index,
                edge.continuation_dispatch_index,
                edge.continuation_arguments,
                blockers,
            );
        }
    }
}

/// The local-slot twin of the argument check: a slice-typed LOCAL initialized
/// from a subslice (`let tail = sub[offset + 1..];`) must write its descriptor
/// slot. When selection declines the shape, the slot keeps garbage and every
/// later read of `tail` is wrong — report it instead.
fn collect_subslice_local_initializer_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let expressions = &input.state_storage.expressions;
    for (_, local) in input.state_storage.locals.iter() {
        if !local.initial_value.is_valid() {
            continue;
        }
        let ExpressionNode::Indexed(indexed) = expressions.expression(local.initial_value) else {
            continue;
        };
        if !matches!(
            expressions.expression(indexed.index),
            ExpressionNode::Range(_)
        ) {
            continue;
        }
        let Some(slot) = input
            .runtime_storage
            .frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ) && state_key_matches(slot.source_key, local.source_key)
                    && slot.symbol == local.symbol)
                    .then_some(slot)
            })
        else {
            continue;
        };
        if slot.byte_size == 0 {
            continue;
        }
        if parameter_slot_write_is_planned(
            input,
            local.statement_index,
            slot.byte_offset,
            slot.byte_size,
        ) {
            continue;
        }

        blockers.insert(blocker(
            "descriptor local",
            &format!(
                "{} statement {} local `{}` = `{}`: subslice descriptor construction \
                 (ptr/len from the base descriptor) is not lowered for this shape yet{}",
                state_name(input, local.source_key),
                local.statement_index,
                local.name,
                expressions.display_name(local.initial_value),
                proof_scope_suffix(input, local.source_key)
            ),
        ));
    }
}

fn state_key_matches(actual: StateKey, expected: StateKey) -> bool {
    actual == expected || (actual.machine == expected.machine && actual.state == expected.state)
}

fn verify_subslice_arguments_materialized(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    target_dispatch_index: u32,
    arguments: HandleSpan<ExpressionHandle>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let expressions = &input.control_flow.expressions;
    let arguments = expressions.expression_handles(arguments);
    if arguments.is_empty() {
        return;
    }
    let Some(target_key) = dispatch_key_for_index(input, target_dispatch_index) else {
        return;
    };
    let Some(target_state) = input.control_flow.state_by_key(target_key) else {
        return;
    };

    for (parameter_index, parameter) in input
        .control_flow
        .state_parameters(target_state)
        .iter()
        .enumerate()
    {
        let Some(argument) = arguments.get(parameter_index).copied() else {
            break;
        };
        if !expression_is_subslice(input, argument) {
            continue;
        }
        let Some(slot) = input
            .runtime_storage
            .frame_slots
            .iter()
            .find_map(|(_, slot)| {
                (slot.dispatch_index == target_dispatch_index && slot.symbol == parameter.symbol)
                    .then_some(slot)
            })
        else {
            continue;
        };
        if slot.byte_size == 0 {
            // A zero-sized slot is reported by the storage blockers already.
            continue;
        }
        if parameter_slot_write_is_planned(input, statement_index, slot.byte_offset, slot.byte_size)
        {
            continue;
        }

        blockers.insert(blocker(
            "descriptor argument",
            &format!(
                "{} statement {} subslice argument `{}` -> {} parameter `{}`: \
                 descriptor construction (ptr/len from the base descriptor) is not \
                 lowered for this shape yet{}",
                state_name(input, source_key),
                statement_index,
                expressions.display_name(argument),
                state_name(input, target_key),
                slot.name,
                proof_scope_suffix(input, source_key)
            ),
        ));
    }
}

/// A range-indexed expression (`base[a..b]`, `base[a..]`, `base[..b]`): the
/// only argument shape that REQUIRES descriptor construction rather than a
/// place copy.
fn expression_is_subslice(input: &EmissionPlanningInput<'_>, argument: ExpressionHandle) -> bool {
    let expressions = &input.control_flow.expressions;
    let ExpressionNode::Indexed(indexed) = expressions.expression(argument) else {
        return false;
    };
    matches!(
        expressions.expression(indexed.index),
        ExpressionNode::Range(_)
    )
}

/// Whether any selected instruction at this statement writes into the
/// parameter slot's byte range (directly or via the staged scratch copy, whose
/// phase-B `CopyRuntimeStorage` targets the real slot).
fn parameter_slot_write_is_planned(
    input: &EmissionPlanningInput<'_>,
    statement_index: usize,
    slot_offset: usize,
    slot_size: usize,
) -> bool {
    input
        .instructions
        .code
        .instructions
        .iter()
        .any(|(_, instruction)| {
            instruction.source_statement == statement_index
                && instruction_frame_write_range(&instruction.kind).is_some_and(
                    |(offset, length)| {
                        offset < slot_offset + slot_size && slot_offset < offset + length
                    },
                )
        })
}

/// The runtime-frame byte range a selected instruction writes, for the kinds
/// argument materialization emits. Returns `None` for non-frame writes.
fn instruction_frame_write_range(kind: &SelectedInstructionKind) -> Option<(usize, usize)> {
    match kind {
        SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset,
            byte_size,
            ..
        } => Some((*byte_offset, *byte_size)),
        SelectedInstructionKind::WriteRuntimeStorageBinary {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset,
            byte_size,
            ..
        } => Some((*target_offset, *byte_size)),
        SelectedInstructionKind::CopyRuntimeStorage {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            target_offset,
            byte_count,
            ..
        } => Some((*target_offset, *byte_count)),
        SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            target_offset, ..
        }
        | SelectedInstructionKind::WriteRuntimePointeeAddressToRuntimeFrame {
            target_offset, ..
        }
        | SelectedInstructionKind::WriteRuntimeFrameIndexedAddressToRuntimeFrame {
            target_offset,
            ..
        }
        | SelectedInstructionKind::WriteRuntimeFrameFixedIndexedAddressToRuntimeFrame {
            target_offset,
            ..
        }
        | SelectedInstructionKind::WriteRuntimeFrameBaseIndexedAddressToRuntimeFrame {
            target_offset,
            ..
        } => Some((*target_offset, 8)),
        SelectedInstructionKind::CopyRuntimePointeeToRuntimeFrame {
            target_region: _,
            target_offset,
            byte_count,
            ..
        }
        | SelectedInstructionKind::CopyRuntimeFrameFixedIndexedToRuntimeFrame {
            target_offset,
            byte_count,
            ..
        } => Some((*target_offset, *byte_count)),
        _ => None,
    }
}

fn dispatch_key_for_index(
    input: &EmissionPlanningInput<'_>,
    dispatch_index: u32,
) -> Option<StateKey> {
    input
        .runtime_dispatch_loop
        .cases
        .iter()
        .find_map(|(_, case)| (case.dispatch_index == dispatch_index).then_some(case.key))
}
