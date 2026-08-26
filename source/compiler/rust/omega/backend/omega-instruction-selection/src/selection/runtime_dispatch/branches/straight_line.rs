use crate::InstructionSelectionInput;
use omega_control_flow::{OperationKind, StateKey, StateParameterFlow};
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_runtime_branching::{
    RuntimeLeafBranchExpansion, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchBindingKind, RuntimeStraightLineBranchExpansion,
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};
use psi_arena::Arena;
use psi_checked_trees::expression::{
    ExpressionHandle, ExpressionNode, ExpressionTable, TableBorrowExpression,
};
use psi_checked_trees::statement::StatementNode;

use super::super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, RuntimeAliasResolutionContext,
    resolve_straight_line_binding_expression_handle, strip_mutable_expression_handle,
};
use super::super::super::lookups::{
    host_call_for_statement, state_assignment_value_call, state_assignment_value_call_by_ordinal,
    state_call_for_statement, state_mutation_for_statement, state_operations, state_parameters,
    state_transition_argument_call, state_transition_argument_call_by_ordinal,
};
use super::super::super::storage_places::{
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_place_in_table,
};
use super::super::guards::select_runtime_straight_line_branch_guards;
use super::super::text_writes::runtime_text_builder_write_in_table_emit;
use super::super::writes::{
    RuntimeStaticValues, emit_runtime_frame_slot_slice_descriptor_write_in_table,
    runtime_frame_slot_target_expression, select_runtime_frame_slot_value_write_in_table,
};
use super::mutation::select_runtime_resolved_mutation_write_in_table_with_scratch;
use super::prelude::{BranchPreludeSelectionScratch, select_runtime_branch_preludes_for_operation};
use crate::selection::host_operations::select_host_call;
use crate::selection::instruction_sink::SelectedInstructionSink;
use crate::selection::state_bodies::{StateBodyVisitStack, select_state_body_instructions};
use omega_abstract_operations::{
    InstructionOperand, RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction,
    SelectedInstructionKind, StateGuardOperator,
};
use omega_state_calls::{StateCallArgument, StateCallLowering, StateCallRole};
use omega_state_guards::{StateGuardKind, StateGuardLowering};

#[derive(Default)]
pub(in crate::selection) struct StraightLineBranchSelectionScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
}

pub(in crate::selection) fn select_runtime_straight_line_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    _expansion_cursor: &mut usize,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(root_scope_id) = root_scope_id_for_operation(input, dispatch_index, operation, false)
    else {
        return;
    };
    select_runtime_branch_scope(
        input,
        dispatch_index,
        operation,
        root_scope_id,
        false,
        scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
}

#[derive(Clone, Copy)]
enum RuntimeBranchTreeArm<'a> {
    Straight(&'a RuntimeStraightLineBranchExpansion),
    Leaf(&'a RuntimeLeafBranchExpansion),
}

impl RuntimeBranchTreeArm<'_> {
    fn edge_order(self) -> usize {
        match self {
            Self::Straight(expansion) => expansion.edge_order,
            Self::Leaf(expansion) => expansion.edge_order,
        }
    }

    fn is_default(self, use_local_guard: bool) -> bool {
        match self {
            Self::Straight(expansion) => {
                if use_local_guard {
                    expansion.local_guard_kind == StateGuardKind::Always
                } else {
                    expansion.guard_kind == StateGuardKind::Always
                }
            }
            Self::Leaf(expansion) => {
                if use_local_guard {
                    expansion.local_guard_kind == StateGuardKind::Always
                } else {
                    expansion.guard_kind == StateGuardKind::Always
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_branch_scope(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    scope_id: u32,
    use_local_guard: bool,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let straight = input
        .runtime_branching_calls
        .straight_line_expansions
        .storage_slice();
    let leaves = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice();

    // A synthetic self-target expansion is the nested call's entry prelude,
    // not a transition arm. It runs once before the root scope's edges.
    for prelude in straight.iter().filter(|expansion| {
        expansion.scope_id == scope_id
            && expansion.child_scope_id == scope_id
            && expansion.branch_key == expansion.target_key
            && straight_line_expansion_matches_tree_operation(
                expansion,
                dispatch_index,
                operation,
                true,
            )
    }) {
        select_runtime_straight_line_tree_arm_body(
            input,
            prelude,
            true,
            false,
            operation,
            scratch,
            operands,
            runtime_value_operands,
            selected_instructions,
        );
    }

    let mut arms = Vec::new();
    arms.extend(straight.iter().filter_map(|expansion| {
        (expansion.scope_id == scope_id
            && expansion.child_scope_id != scope_id
            && straight_line_expansion_matches_tree_operation(
                expansion,
                dispatch_index,
                operation,
                true,
            ))
        .then_some(RuntimeBranchTreeArm::Straight(expansion))
    }));
    arms.extend(leaves.iter().filter_map(|expansion| {
        (expansion.scope_id == scope_id
            && leaf_expansion_matches_tree_operation(expansion, dispatch_index, operation, true))
        .then_some(RuntimeBranchTreeArm::Leaf(expansion))
    }));
    arms.sort_by_key(|arm| (arm.is_default(use_local_guard), arm.edge_order()));

    let arm_count = arms.len();
    let mut leaf_scratch = super::LeafBranchSelectionScratch::default();
    for (index, arm) in arms.into_iter().enumerate() {
        let has_following_arm = index + 1 < arm_count;
        match arm {
            RuntimeBranchTreeArm::Straight(expansion) => {
                select_runtime_straight_line_tree_arm_body(
                    input,
                    expansion,
                    use_local_guard,
                    has_following_arm,
                    operation,
                    scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            RuntimeBranchTreeArm::Leaf(expansion) => {
                super::select_runtime_leaf_branch_expansion_for_tree(
                    input,
                    expansion,
                    use_local_guard,
                    has_following_arm,
                    &mut leaf_scratch,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
        }
    }
    if arm_count > 1 {
        super::push_branch_arms_end_marker(
            scope_id,
            operation.source_key,
            operation.statement_index,
            selected_instructions,
        );
    }
}

fn root_scope_id_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    allow_synthetic_nested_operation: bool,
) -> Option<u32> {
    // A nested value machine may terminate directly from its entry state. Such
    // a call has only a leaf expansion, so restricting root discovery to
    // straight-line expansions silently skips the call's result write.
    let straight_scopes = input
        .runtime_branching_calls
        .straight_line_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            straight_line_expansion_matches_tree_operation(
                expansion,
                dispatch_index,
                operation,
                allow_synthetic_nested_operation,
            )
        })
        .map(|expansion| expansion.scope_id);
    let leaf_scopes = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            leaf_expansion_matches_tree_operation(
                expansion,
                dispatch_index,
                operation,
                allow_synthetic_nested_operation,
            )
        })
        .map(|expansion| expansion.scope_id);
    straight_scopes.chain(leaf_scopes).min()
}

fn straight_line_expansion_matches_tree_operation(
    expansion: &RuntimeStraightLineBranchExpansion,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    allow_synthetic_nested_operation: bool,
) -> bool {
    expansion.dispatch_index == dispatch_index
        && super::super::state_key_matches_statement_source(
            expansion.source_key,
            operation.source_key,
        )
        && expansion.statement_index == operation.statement_index
        && match operation.kind {
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                role, call_ordinal, ..
            }
            | RuntimeDispatchBodyOperationKind::InlineStateCall {
                role, call_ordinal, ..
            }
            | RuntimeDispatchBodyOperationKind::StateCall {
                role, call_ordinal, ..
            } => expansion.role == role && expansion.call_ordinal == call_ordinal,
            RuntimeDispatchBodyOperationKind::Other => allow_synthetic_nested_operation,
            _ => false,
        }
}

fn leaf_expansion_matches_tree_operation(
    expansion: &RuntimeLeafBranchExpansion,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    allow_synthetic_nested_operation: bool,
) -> bool {
    expansion.dispatch_index == dispatch_index
        && super::super::state_key_matches_statement_source(
            expansion.source_key,
            operation.source_key,
        )
        && expansion.statement_index == operation.statement_index
        && match operation.kind {
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                role, call_ordinal, ..
            }
            | RuntimeDispatchBodyOperationKind::InlineStateCall {
                role, call_ordinal, ..
            }
            | RuntimeDispatchBodyOperationKind::StateCall {
                role, call_ordinal, ..
            } => expansion.role == role && expansion.call_ordinal == call_ordinal,
            RuntimeDispatchBodyOperationKind::Other => allow_synthetic_nested_operation,
            _ => false,
        }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_tree_arm_body(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    use_local_guard: bool,
    has_following_arm: bool,
    operation: &RuntimeDispatchBodyOperation,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut local;
    let expansion = if use_local_guard {
        local = expansion.clone();
        local.resolved_guard = local.guard;
        local.guard_kind = local.local_guard_kind;
        &local
    } else {
        expansion
    };
    let guards =
        select_runtime_straight_line_branch_guards(input, expansion, runtime_value_operands);
    let emitted_guard = !guards.is_empty();
    let guard_start = selected_instructions.len();
    if emitted_guard {
        super::push_branch_scope_marker(
            expansion.scope_id,
            expansion.source_key,
            expansion.statement_index,
            selected_instructions,
        );
    }
    for guard in guards {
        selected_instructions.push(SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    }
    let write_start = selected_instructions.len();
    select_runtime_straight_line_branch_writes(
        input,
        expansion,
        scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_straight_line_branch_terminal_value_write(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_straight_line_assignment_value_target_copy(
        input,
        expansion,
        selected_instructions,
    );
    if expansion.child_scope_id != expansion.scope_id {
        select_runtime_branch_scope(
            input,
            expansion.dispatch_index,
            operation,
            expansion.child_scope_id,
            true,
            scratch,
            operands,
            runtime_value_operands,
            selected_instructions,
        );
    }
    if selected_instructions.len() == write_start {
        while selected_instructions.len() > guard_start {
            selected_instructions.pop();
        }
        return;
    }
    if has_following_arm {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::ForwardBranchSkip,
                operator: StateGuardOperator::Equal,
                storage_region: RuntimeStorageRegion::Machine,
                byte_offset: expansion.scope_id as usize,
                byte_size: 0,
                expected_value: 0,
                has_storage: false,
                is_float: false,
            },
            source_key: expansion.target_key,
            source_statement: expansion.target_statement_index,
        });
    }
    if emitted_guard {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: StateGuardLowering::NoOp,
                operator: StateGuardOperator::Equal,
                storage_region: RuntimeStorageRegion::Machine,
                byte_offset: 0,
                byte_size: 0,
                expected_value: 0,
                has_storage: false,
                is_float: false,
            },
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    }
}

fn select_runtime_straight_line_branch_terminal_value_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    scratch: &mut StraightLineBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if !expansion.target_value.is_valid() {
        return;
    }
    if !matches!(
        expansion.role,
        StateCallRole::AssignmentValue
            | StateCallRole::CallArgument
            | StateCallRole::TransitionArgument
            | StateCallRole::TransitionGuard
    ) {
        return;
    }
    let Some(slot) = input.runtime_storage.call_result_slot_by_ordinal(
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expansion.role,
        expansion.call_ordinal,
    ) else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    scratch.expressions.clear();
    let expressions = &mut scratch.expressions;
    let value = expressions.copy_from(
        &input.runtime_branching_calls.expressions,
        expansion.target_value,
    );
    let resolved_value = resolve_straight_line_binding_expression_handle(
        &input.runtime_branching_calls.expressions,
        expressions,
        value,
        bindings,
    );
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expressions,
        slot,
        resolved_value,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expressions,
        slot,
        resolved_value,
        &static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
        return;
    }

    let target = runtime_frame_slot_target_expression(expressions, slot);
    if select_runtime_resolved_mutation_write_in_table_with_scratch(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.source_key,
        expansion.source_key,
        expansion.statement_index,
        expressions,
        target,
        resolved_value,
        &mut scratch.mutable_expressions,
        &mut scratch.resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    // Non-table mutation-write fallback removed (Phase 4): proven dead emitter — the
    // `_in_table` path above handles every case that lowers — EXCEPT a bare CALL
    // terminal, poisoned below (the leaf writer's twin: a host-boundary call in
    // value-return position would silently never run and its result slot would
    // read ZII 0; any unresolved machine/state value call was already a
    // frontend error).
    if matches!(
        scratch.expressions.expression(resolved_value),
        psi_checked_trees::expression::ExpressionNode::Call(_)
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering:
                    omega_abstract_operations::StateGuardLowering::UnloweredTerminalHostCall,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
                storage_region: RuntimeStorageRegion::Machine,
                byte_offset: 0,
                byte_size: 0,
                expected_value: 0,
                has_storage: false,
                is_float: false,
            },
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    }
}

fn select_runtime_straight_line_assignment_value_target_copy(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.role != StateCallRole::AssignmentValue || !expansion.target_value.is_valid() {
        return;
    }
    let Some(source_slot) = input.runtime_storage.call_result_slot_by_ordinal(
        expansion.dispatch_index,
        expansion.source_key,
        expansion.statement_index,
        expansion.role,
        expansion.call_ordinal,
    ) else {
        return;
    };
    let Some(mutation) =
        state_mutation_for_statement(input, expansion.source_key, expansion.statement_index)
    else {
        return;
    };
    if let Some(pointer_target) = resolve_runtime_pointee_slot_offset_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.state_storage.expressions,
        mutation.target,
    ) && source_slot.byte_size == pointer_target.pointee_byte_size
        && source_slot.byte_size > 0
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::copy_places_to_pointee(
                RuntimeStorageRegion::RuntimeFrame,
                source_slot.byte_offset,
                pointer_target.pointer_byte_offset,
                pointer_target.field_byte_offset,
                source_slot.byte_size,
            ),
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
        return;
    }
    let Some(target_place) = resolve_runtime_storage_place_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        &input.state_storage.expressions,
        mutation.target,
    ) else {
        return;
    };
    if source_slot.byte_size != target_place.byte_count || source_slot.byte_size == 0 {
        return;
    }

    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::copy_places_direct(
            RuntimeStorageRegion::RuntimeFrame,
            source_slot.byte_offset,
            target_place.region,
            target_place.byte_offset,
            source_slot.byte_size,
        ),
        source_key: expansion.source_key,
        source_statement: expansion.statement_index,
    });
}

fn select_runtime_straight_line_branch_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(operations) = input
        .runtime_branching_calls
        .straight_line_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();

    // Statements whose assignment-value InlineBranching call this loop already
    // emitted. `straight_line_operations` lists a StateCall operation AND a
    // LocalData operation for `let x = self.f(...)`; the StateCall arm runs the
    // callee and copies its result into the local, so the LocalData arm must not
    // emit the call AGAIN (the callee's side effects ran twice per evaluation --
    // one doubling per nesting level of the dungeon's RNG chain).
    let mut emitted_assignment_calls: Vec<(StateKey, usize)> = Vec::new();

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::LocalData => {
                if emitted_assignment_calls
                    .contains(&(operation.source_key, operation.statement_index))
                {
                    continue;
                }
                select_runtime_straight_line_local_initializer_write(
                    input,
                    expansion,
                    operation,
                    bindings,
                    scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::HostCall => {
                let Some(host_call) =
                    host_call_for_statement(input, operation.source_key, operation.statement_index)
                else {
                    continue;
                };
                let alias_bindings = straight_line_alias_bindings(expansion, bindings);
                let alias_context = (!alias_bindings.bindings().is_empty()).then_some(
                    RuntimeAliasResolutionContext {
                        aliases: alias_bindings.bindings(),
                        alias_expressions: &input.runtime_branching_calls.expressions,
                    },
                );
                select_host_call(
                    input,
                    host_call,
                    Some(expansion.dispatch_index),
                    alias_context,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                scratch.expressions.clear();
                let expressions = &mut scratch.expressions;
                let target =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *target);
                let value =
                    expressions.copy_from(&input.runtime_branching_calls.expressions, *value);
                let resolved_target = resolve_straight_line_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    target,
                    bindings,
                );
                let resolved_value = resolve_straight_line_binding_expression_handle(
                    &input.runtime_branching_calls.expressions,
                    expressions,
                    value,
                    bindings,
                );
                if select_runtime_resolved_mutation_write_in_table_with_scratch(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.source_key,
                    operation.source_key,
                    operation.statement_index,
                    &expressions,
                    resolved_target,
                    resolved_value,
                    &mut scratch.mutable_expressions,
                    &mut scratch.resolved_segment_expressions,
                    runtime_value_operands,
                    selected_instructions,
                ) {
                    continue;
                }
                scratch.resolved_segment_expressions.clear();
                if runtime_text_builder_write_in_table_emit(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.source_key,
                    operation.statement_index,
                    &expressions,
                    resolved_target,
                    &mut scratch.resolved_segment_expressions,
                    &|expressions, expression| {
                        resolve_straight_line_binding_expression_handle(
                            &input.runtime_branching_calls.expressions,
                            expressions,
                            expression,
                            bindings,
                        )
                    },
                    &mut |kind| {
                        selected_instructions.push(
                            omega_abstract_operations::SelectedInstruction {
                                kind,
                                source_key: operation.source_key,
                                source_statement: operation.statement_index,
                            },
                        );
                    },
                ) {
                    continue;
                }
                // Non-table mutation-write fallback removed (Phase 4): proven dead
                // emitter — the `_in_table` write + text-builder paths above cover
                // every case that lowers.
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                role,
                call_ordinal,
                target_key,
                lowering: StateCallLowering::InlineLeaf,
                ..
            } => {
                select_runtime_straight_line_leaf_state_call_writes(
                    input,
                    expansion,
                    operation,
                    *role,
                    *call_ordinal,
                    bindings,
                    *target_key,
                    scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_assignment_value_call_result_local_copy(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                role,
                call_ordinal,
                target_key,
                lowering: StateCallLowering::InlineExpansion,
                ..
            } => {
                select_runtime_straight_line_inline_state_call(
                    input,
                    expansion,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    *target_key,
                    bindings,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_assignment_value_call_result_local_copy(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                role,
                call_ordinal,
                lowering: StateCallLowering::InlineBranching,
                ..
            } => {
                select_runtime_straight_line_nested_branch_expansions_for_operation(
                    input,
                    expansion.dispatch_index,
                    operation,
                    scratch,
                    operands,
                    runtime_value_operands,
                    selected_instructions,
                );
                select_assignment_value_call_result_local_copy(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    operation.statement_index,
                    *role,
                    *call_ordinal,
                    selected_instructions,
                );
                if *role == StateCallRole::AssignmentValue
                    && local_initializer_is_direct_call(
                        input,
                        operation.source_key,
                        operation.statement_index,
                    )
                {
                    emitted_assignment_calls
                        .push((operation.source_key, operation.statement_index));
                }
            }
            _ => {}
        }
    }
}

/// Substitute Names that refer to PRIOR slot-less locals of `expansion`'s state
/// (statements before `statement_bound`) with their initializers -- bindings
/// re-applied, recursively folded. Mirrors the leaf path's
/// `resolve_leaf_caller_local_initializer_names` for the straight-line branch.
/// A local that HAS a slot keeps its Name (its slot holds the value); only the
/// arithmetic subset is folded (Binary/Unary/Cast/Mutable/Name), so a Member /
/// Call / aggregate initializer is left untouched (no stale field read, no
/// un-lowerable call planted -- the same guards the leaf resolver relies on).
fn fold_straight_line_prior_local_names(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
    dispatch_index: u32,
    expressions: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeStraightLineBranchBinding],
    statement_bound: usize,
) -> ExpressionHandle {
    match expressions.expression(expression).clone() {
        ExpressionNode::Binary(binary) => {
            let left = fold_straight_line_prior_local_names(
                input,
                state_key,
                dispatch_index,
                expressions,
                binary.left,
                bindings,
                statement_bound,
            );
            let right = fold_straight_line_prior_local_names(
                input,
                state_key,
                dispatch_index,
                expressions,
                binary.right,
                bindings,
                statement_bound,
            );
            if left == binary.left && right == binary.right {
                return expression;
            }
            expressions.insert(ExpressionNode::Binary(
                psi_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                },
            ))
        }
        ExpressionNode::Unary(unary) => {
            let operand = fold_straight_line_prior_local_names(
                input,
                state_key,
                dispatch_index,
                expressions,
                unary.operand,
                bindings,
                statement_bound,
            );
            if operand == unary.operand {
                return expression;
            }
            expressions.insert(ExpressionNode::Unary(
                psi_checked_trees::expression::TableUnaryExpression {
                    operator: unary.operator,
                    operand,
                },
            ))
        }
        ExpressionNode::Cast(cast) => {
            let value = fold_straight_line_prior_local_names(
                input,
                state_key,
                dispatch_index,
                expressions,
                cast.value,
                bindings,
                statement_bound,
            );
            if value == cast.value {
                return expression;
            }
            expressions.insert(ExpressionNode::Cast(
                psi_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                },
            ))
        }
        ExpressionNode::Borrow(inner) => {
            let resolved = fold_straight_line_prior_local_names(
                input,
                state_key,
                dispatch_index,
                expressions,
                inner.target,
                bindings,
                statement_bound,
            );
            if resolved == inner.target {
                return expression;
            }
            expressions.insert(ExpressionNode::Borrow(TableBorrowExpression {
                target: resolved,
                access: inner.access,
            }))
        }
        ExpressionNode::Name(path) => {
            if path.members.count() != 1 {
                return expression;
            }
            let Some(machine) = input
                .program
                .machines()
                .iter()
                .find(|machine| machine.symbol == state_key.machine)
            else {
                return expression;
            };
            let Some(state) = input
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == state_key.state)
            else {
                return expression;
            };
            let statements = input
                .program
                .statement_table
                .statements(state.statement_nodes);
            let mut matched: Option<(usize, ExpressionHandle)> = None;
            for (index, statement) in statements.iter().enumerate().take(statement_bound) {
                let StatementNode::LocalData(local) = statement else {
                    continue;
                };
                let matches = if path.head_symbol.is_valid() || path.symbol.is_valid() {
                    local.symbol == path.symbol || local.symbol == path.head_symbol
                } else {
                    expressions
                        .name_path_members(path.members)
                        .first()
                        .is_some_and(|name| *name == local.name)
                };
                if matches && local.initial_value.is_valid() {
                    matched = Some((index, local.initial_value));
                }
            }
            let Some((local_index, initial_value)) = matched else {
                return expression;
            };
            // A slotted local keeps its Name -- its own initializer write (emitted
            // earlier in the straight-line sequence) holds the value.
            let has_slot = input.runtime_storage.frame_slots.iter().any(|(_, slot)| {
                slot.dispatch_index == dispatch_index
                    && slot.source_key == state_key
                    && slot.statement_index == local_index
                    && matches!(
                        slot.kind,
                        omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                    )
            });
            if has_slot {
                return expression;
            }
            let initializer = expressions.copy_from(&input.program.expression_table, initial_value);
            let bound = resolve_straight_line_binding_expression_handle(
                &input.runtime_branching_calls.expressions,
                expressions,
                initializer,
                bindings,
            );
            fold_straight_line_prior_local_names(
                input,
                state_key,
                dispatch_index,
                expressions,
                bound,
                bindings,
                local_index,
            )
        }
        _ => expression,
    }
}

fn select_runtime_straight_line_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    bindings: &[RuntimeStraightLineBranchBinding],
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == expansion.dispatch_index
                && slot.source_key == operation.source_key
                && slot.statement_index == operation.statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
    else {
        return;
    };
    if select_runtime_straight_line_assignment_value_local_initializer_call(
        input,
        expansion,
        operation,
        scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    let Some(initializer) = local_initializer_handle(
        input,
        &mut scratch.expressions,
        operation.source_key,
        operation.statement_index,
    ) else {
        return;
    };
    let expressions = &mut scratch.expressions;
    let resolved_initializer = resolve_straight_line_binding_expression_handle(
        &input.runtime_branching_calls.expressions,
        expressions,
        initializer,
        bindings,
    );
    // Fold PRIOR slot-less locals of the same state into this initializer. A
    // value callee's spliced POST-ENTRY state elides an intermediate local read
    // only by a sibling `let` (`let scaled = q*freq; let rem = total - scaled;`)
    // -- it has no slot and its write is skipped above -- so `rem`'s
    // initializer keeps `scaled` as a Name resolving to a missing slot -> ZII
    // (the post-entry chained-let miscompile). The LEAF path already does this
    // via resolve_leaf_caller_local_initializer_names; the straight-line path
    // only applied bindings. Mirror it here.
    let resolved_initializer = fold_straight_line_prior_local_names(
        input,
        operation.source_key,
        expansion.dispatch_index,
        expressions,
        resolved_initializer,
        bindings,
        operation.statement_index,
    );
    if crate::selection::runtime_dispatch::emit_local_dynamic_conformance_descriptor(
        input,
        expansion.dispatch_index,
        operation.source_key,
        operation.statement_index,
        slot,
        expressions,
        resolved_initializer,
        selected_instructions,
    ) {
        return;
    }
    // A judged recast local is an address-bearing runtime view. The ordinary
    // dispatch path materializes that address before projected uses; a value
    // callee spliced into a straight-line branch must do the same. Otherwise
    // its later `view.field` local initializers dereference the ZII frame slot
    // (the programmable-layout `struct stat` decode either read zero or
    // faulted, depending on whether storage planning retained the view).
    let recast_initializer = match expressions.expression(resolved_initializer) {
        ExpressionNode::Borrow(inner)
            if matches!(
                expressions.expression(inner.target),
                ExpressionNode::Cast(cast) if cast.form.is_recast()
            ) =>
        {
            inner.target
        }
        _ => resolved_initializer,
    };
    if let ExpressionNode::Cast(cast) = expressions.expression(recast_initializer)
        && cast.form.is_recast()
    {
        if emit_runtime_frame_slot_slice_descriptor_write_in_table(
            input,
            expansion.dispatch_index,
            operation.source_key,
            operation.statement_index,
            expressions,
            slot,
            recast_initializer,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
        // Mutable recasts always carry the backing ADDRESS. Unlike a shared
        // scalar view, a writable view may not content-spill into its slot:
        // later mutation resolves the slot as a pointer and writes through it.
        if cast.form == psi_language_core::CastForm::RecastMutable {
            if let Some(place) = resolve_runtime_storage_place_in_table(
                input,
                expansion.dispatch_index,
                operation.source_key,
                expressions,
                cast.value,
            ) {
                selected_instructions.push(SelectedInstruction {
                    kind: crate::selection::runtime_dispatch::write_place_address_direct(
                        place.region,
                        place.byte_offset,
                        slot.byte_offset,
                    ),
                    source_key: operation.source_key,
                    source_statement: operation.statement_index,
                });
                return;
            }
            if let Some(indexed) =
                crate::selection::storage_places::resolve_runtime_machine_indexed_target_in_table(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    expressions,
                    cast.value,
                )
            {
                selected_instructions.push(SelectedInstruction {
                    kind: crate::selection::runtime_dispatch::write_place_address_machine_indexed(
                        indexed.base_byte_offset,
                        indexed.index_region,
                        indexed.index_offset,
                        indexed.index_byte_size,
                        indexed.element_byte_size,
                        indexed.field_byte_offset,
                        slot.byte_offset,
                    ),
                    source_key: operation.source_key,
                    source_statement: operation.statement_index,
                });
                return;
            }
        }
        let target_size =
            crate::selection::runtime_dispatch::recast_target_byte_size(input, cast.target_type);
        if let Some(size) = target_size
            && let Some(place) = resolve_runtime_storage_place_in_table(
                input,
                expansion.dispatch_index,
                operation.source_key,
                expressions,
                cast.value,
            )
            && size != place.byte_count
        {
            let kind = if size > input.runtime_abi.pointer_size {
                crate::selection::runtime_dispatch::write_place_address_direct(
                    place.region,
                    place.byte_offset,
                    slot.byte_offset,
                )
            } else {
                crate::selection::runtime_dispatch::copy_places_direct(
                    place.region,
                    place.byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    size,
                )
            };
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
            return;
        }
        if let Some(size) = target_size
            && let Some(indexed) =
                crate::selection::storage_places::resolve_runtime_machine_indexed_target_in_table(
                    input,
                    expansion.dispatch_index,
                    operation.source_key,
                    expressions,
                    cast.value,
                )
        {
            let kind = if size > input.runtime_abi.pointer_size {
                crate::selection::runtime_dispatch::write_place_address_machine_indexed(
                    indexed.base_byte_offset,
                    indexed.index_region,
                    indexed.index_offset,
                    indexed.index_byte_size,
                    indexed.element_byte_size,
                    indexed.field_byte_offset,
                    slot.byte_offset,
                )
            } else {
                crate::selection::runtime_dispatch::copy_places_from_machine_indexed(
                    indexed.base_byte_offset,
                    indexed.index_region,
                    indexed.index_offset,
                    indexed.index_byte_size,
                    indexed.element_byte_size,
                    indexed.field_byte_offset,
                    RuntimeStorageRegion::RuntimeFrame,
                    slot.byte_offset,
                    size,
                )
            };
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
            return;
        }
    }
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    // This expression belongs to the branch operation's state. The outer
    // expansion source identifies the caller and cannot resolve result slots
    // for calls embedded in this local initializer.
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        expansion.dispatch_index,
        operation.source_key,
        operation.statement_index,
        expressions,
        slot,
        resolved_initializer,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        operation.source_key,
        operation.statement_index,
        expressions,
        slot,
        resolved_initializer,
        &static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: operation.source_key,
            source_statement: operation.statement_index,
        });
        return;
    }
    // A branch-local text equality (`let matches = self.name == "omega"`)
    // cannot use the scalar value writer above. It must still be materialized
    // here, in the straight-line branch prelude, because the nested
    // transition's guard or forwarded argument can read the local before any
    // terminal leaf expansion runs.
    if crate::selection::runtime_dispatch::writes::emit_runtime_frame_slot_text_comparison_write_in_table(
        input,
        expansion.dispatch_index,
        operation.source_key,
        operation.statement_index,
        expressions,
        slot,
        resolved_initializer,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    let target = runtime_frame_slot_target_expression(expressions, slot);
    scratch.resolved_segment_expressions.clear();
    if runtime_text_builder_write_in_table_emit(
        input,
        expansion.dispatch_index,
        operation.source_key,
        expansion.source_key,
        operation.statement_index,
        expressions,
        target,
        &mut scratch.resolved_segment_expressions,
        &|expressions, expression| {
            resolve_straight_line_binding_expression_handle(
                &input.runtime_branching_calls.expressions,
                expressions,
                expression,
                bindings,
            )
        },
        &mut |kind| {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
        },
    ) {}
}

fn select_runtime_straight_line_assignment_value_local_initializer_call(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if !local_initializer_is_direct_call(input, operation.source_key, operation.statement_index) {
        return false;
    }
    let Some(state_call) =
        state_assignment_value_call(input, operation.source_key, operation.statement_index)
    else {
        return false;
    };
    if state_call.lowering != StateCallLowering::InlineBranching {
        return false;
    }

    let branch_operation = RuntimeStraightLineBranchOperation {
        source_key: operation.source_key,
        statement_index: operation.statement_index,
        kind: RuntimeStraightLineBranchOperationKind::StateCall {
            role: StateCallRole::AssignmentValue,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    };
    let body_operation = RuntimeDispatchBodyOperation {
        source_key: operation.source_key,
        statement_index: operation.statement_index,
        kind: RuntimeDispatchBodyOperationKind::StateCall {
            role: StateCallRole::AssignmentValue,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    };

    let before = selected_instructions.len();
    let mut prelude_cursor = 0usize;
    let mut prelude_scratch = BranchPreludeSelectionScratch::default();
    select_runtime_branch_preludes_for_operation(
        input,
        expansion.dispatch_index,
        &body_operation,
        &mut prelude_cursor,
        &mut prelude_scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_straight_line_nested_branch_expansions_for_operation(
        input,
        expansion.dispatch_index,
        &branch_operation,
        scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    if selected_instructions.len() == before {
        return false;
    }

    select_assignment_value_call_result_local_copy(
        input,
        expansion.dispatch_index,
        operation.source_key,
        operation.statement_index,
        StateCallRole::AssignmentValue,
        state_call.call_ordinal,
        selected_instructions,
    );
    true
}

fn local_initializer_is_direct_call(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    let Some(machine) = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)
    else {
        return false;
    };
    let Some(state) = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)
    else {
        return false;
    };
    let Some(StatementNode::LocalData(local)) = input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };
    if !local.initial_value.is_valid() {
        return false;
    }
    let mut initializer = local.initial_value;
    while let ExpressionNode::Borrow(inner) = input.program.expression_table.expression(initializer)
    {
        initializer = inner.target;
    }
    matches!(
        input.program.expression_table.expression(initializer),
        ExpressionNode::Call(_)
    )
}

fn local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> Option<ExpressionHandle> {
    table.clear();
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == source_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == source_key.state)?;
    let statement = input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let StatementNode::LocalData(local_data) = statement else {
        return None;
    };
    local_data
        .initial_value
        .is_valid()
        .then(|| table.copy_from(&input.program.expression_table, local_data.initial_value))
}

pub(in crate::selection) fn select_runtime_straight_line_nested_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeStraightLineBranchOperation,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let RuntimeStraightLineBranchOperationKind::StateCall {
        role,
        call_ordinal,
        target_key,
        argument_count,
        lowering,
    } = operation.kind
    else {
        return;
    };
    let body_operation = RuntimeDispatchBodyOperation {
        source_key: operation.source_key,
        statement_index: operation.statement_index,
        kind: RuntimeDispatchBodyOperationKind::StateCall {
            role,
            call_ordinal,
            target_key,
            argument_count,
            lowering,
        },
    };
    if let Some(root_scope_id) =
        root_scope_id_for_operation(input, dispatch_index, &body_operation, false)
    {
        select_runtime_branch_scope(
            input,
            dispatch_index,
            &body_operation,
            root_scope_id,
            true,
            scratch,
            operands,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

pub(in crate::selection) fn select_assignment_value_call_result_local_copy(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    // An embedded call's result slot is an operand of the enclosing
    // initializer, not the initializer's final value. Copy directly into the
    // local only for `let x = call()`; the full writer handles every larger
    // expression after all of its call operands materialize.
    if role != StateCallRole::AssignmentValue
        || !local_initializer_is_direct_call(input, source_key, statement_index)
    {
        return;
    }
    let Some(source_slot) = input.runtime_storage.call_result_slot_by_ordinal(
        dispatch_index,
        source_key,
        statement_index,
        role,
        call_ordinal,
    ) else {
        return;
    };
    let Some(target_slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && slot.source_key == source_key
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
    else {
        return;
    };
    if source_slot.byte_size != target_slot.byte_size || source_slot.byte_size == 0 {
        return;
    }
    selected_instructions.push(SelectedInstruction {
        kind: crate::selection::runtime_dispatch::copy_places_direct(
            RuntimeStorageRegion::RuntimeFrame,
            source_slot.byte_offset,
            RuntimeStorageRegion::RuntimeFrame,
            target_slot.byte_offset,
            source_slot.byte_size,
        ),
        source_key,
        source_statement: statement_index,
    });
}

fn straight_line_alias_bindings(
    expansion: &RuntimeStraightLineBranchExpansion,
    bindings: &[RuntimeStraightLineBranchBinding],
) -> RuntimeAliasBuffer {
    RuntimeAliasBuffer::from_iter(bindings.iter().map(|binding| {
        let source_key = match binding.kind {
            RuntimeStraightLineBranchBindingKind::BranchParameter => expansion.branch_key,
            RuntimeStraightLineBranchBindingKind::TargetParameter => expansion.target_key,
        };
        RuntimeAliasBinding {
            source_key,
            parameter_symbol: binding.parameter_symbol,
            parameter_name: binding.parameter_name.clone(),
            expression_source_key: expansion.source_key,
            expression: binding.expression,
        }
    }))
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_inline_state_call(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    call_ordinal: usize,
    target_key: StateKey,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let state_call = match role {
        StateCallRole::Statement => state_call_for_statement(input, source_key, statement_index),
        StateCallRole::AssignmentValue => {
            state_assignment_value_call_by_ordinal(input, source_key, statement_index, call_ordinal)
                .or_else(|| state_assignment_value_call(input, source_key, statement_index))
        }
        StateCallRole::CallArgument => {
            super::super::super::lookups::state_call_argument_call_by_ordinal(
                input,
                source_key,
                statement_index,
                call_ordinal,
            )
        }
        StateCallRole::TransitionArgument => state_transition_argument_call_by_ordinal(
            input,
            source_key,
            statement_index,
            call_ordinal,
        )
        .or_else(|| state_transition_argument_call(input, source_key, statement_index)),
        _ => None,
    };
    let Some(state_call) = state_call else { return };
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    let mut child_alias_expressions =
        ExpressionTable::with_expression_capacity(arguments.len().saturating_mul(2));
    let mut child_aliases = RuntimeAliasBuffer::with_capacity(arguments.len());

    for argument in arguments {
        let argument_expression =
            child_alias_expressions.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_expression = resolve_straight_line_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            &mut child_alias_expressions,
            argument_expression,
            straight_line_bindings,
        );
        let expression =
            strip_mutable_expression_handle(&child_alias_expressions, resolved_expression);
        child_aliases.set_alias(RuntimeAliasBinding {
            source_key: target_key,
            parameter_symbol: argument.parameter_symbol,
            parameter_name: argument.parameter_name.clone(),
            expression_source_key: source_key,
            expression,
        });
    }

    select_state_body_instructions(
        input,
        target_key,
        Some(expansion.dispatch_index),
        &child_aliases,
        &child_alias_expressions,
        operands,
        runtime_value_operands,
        selected_instructions,
        &mut StateBodyVisitStack::with_capacity(input.control_flow.states.len()),
    );
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_leaf_state_call_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &RuntimeStraightLineBranchOperation,
    role: StateCallRole,
    call_ordinal: usize,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    target_key: StateKey,
    scratch: &mut StraightLineBranchSelectionScratch,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let state_call = match role {
        StateCallRole::Statement => {
            state_call_for_statement(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::AssignmentValue => state_assignment_value_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_assignment_value_call(input, operation.source_key, operation.statement_index)
        }),
        StateCallRole::CallArgument => {
            super::super::super::lookups::state_call_argument_call_by_ordinal(
                input,
                operation.source_key,
                operation.statement_index,
                call_ordinal,
            )
        }
        StateCallRole::TransitionArgument => state_transition_argument_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_transition_argument_call(input, operation.source_key, operation.statement_index)
        }),
        _ => None,
    };
    let Some(state_call) = state_call else {
        return;
    };
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };
    let leaf_parameters = state_parameters(input, target_key);

    let Some(operations) = state_operations(input, target_key) else {
        return;
    };
    let (child_aliases, child_alias_expressions) = leaf_call_alias_bindings(
        input,
        operation.source_key,
        target_key,
        arguments,
        straight_line_bindings,
    );
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();
    for leaf_operation in operations {
        if matches!(leaf_operation.kind, OperationKind::Call { .. }) {
            let Some(host_call) =
                host_call_for_statement(input, target_key, leaf_operation.statement_index)
            else {
                continue;
            };
            let alias_context =
                (!child_aliases.bindings().is_empty()).then_some(RuntimeAliasResolutionContext {
                    aliases: child_aliases.bindings(),
                    alias_expressions: &child_alias_expressions,
                });
            select_host_call(
                input,
                host_call,
                Some(expansion.dispatch_index),
                alias_context,
                operands,
                runtime_value_operands,
                selected_instructions,
            );
            continue;
        }

        if matches!(leaf_operation.kind, OperationKind::LocalData) {
            select_runtime_leaf_state_call_local_initializer_write(
                input,
                expansion,
                target_key,
                leaf_operation.statement_index,
                leaf_parameters,
                arguments,
                straight_line_bindings,
                scratch,
                runtime_value_operands,
                selected_instructions,
            );
            continue;
        }

        let Some(mutation) =
            state_mutation_for_statement(input, target_key, leaf_operation.statement_index)
        else {
            continue;
        };

        scratch.expressions.clear();
        let expressions = &mut scratch.expressions;
        let mutation_target =
            expressions.copy_from(&input.state_storage.expressions, mutation.target);
        let mutation_value =
            expressions.copy_from(&input.state_storage.expressions, mutation.value);
        let resolved_target = resolve_leaf_call_expression_handle(
            input,
            expressions,
            target_key,
            mutation_target,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        );
        let resolved_value = resolve_leaf_call_expression_handle(
            input,
            expressions,
            target_key,
            mutation_value,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        );
        if select_runtime_resolved_mutation_write_in_table_with_scratch(
            input,
            expansion.dispatch_index,
            target_key,
            target_key,
            target_key,
            leaf_operation.statement_index,
            &expressions,
            resolved_target,
            resolved_value,
            &mut scratch.mutable_expressions,
            &mut scratch.resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        ) {
            continue;
        }
        scratch.resolved_segment_expressions.clear();
        if runtime_text_builder_write_in_table_emit(
            input,
            expansion.dispatch_index,
            target_key,
            target_key,
            leaf_operation.statement_index,
            &expressions,
            resolved_target,
            &mut scratch.resolved_segment_expressions,
            &|expressions, expression| {
                resolve_leaf_call_expression_handle(
                    input,
                    expressions,
                    target_key,
                    expression,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                )
            },
            &mut |kind| {
                selected_instructions.push(omega_abstract_operations::SelectedInstruction {
                    kind,
                    source_key: target_key,
                    source_statement: leaf_operation.statement_index,
                });
            },
        ) {
            continue;
        }
        // Non-table mutation-write fallback removed (Phase 4): proven dead emitter —
        // the `_in_table` write + text-builder paths above cover every lowering case.
    }
}

fn leaf_call_alias_bindings(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    target_key: StateKey,
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> (RuntimeAliasBuffer, ExpressionTable) {
    let mut child_alias_expressions =
        ExpressionTable::with_expression_capacity(arguments.len().saturating_mul(2));
    let mut child_aliases = RuntimeAliasBuffer::with_capacity(arguments.len());

    for argument in arguments {
        let argument_expression =
            child_alias_expressions.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_expression = resolve_straight_line_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            &mut child_alias_expressions,
            argument_expression,
            straight_line_bindings,
        );
        let expression =
            strip_mutable_expression_handle(&child_alias_expressions, resolved_expression);
        child_aliases.set_alias(RuntimeAliasBinding {
            source_key: target_key,
            parameter_symbol: argument.parameter_symbol,
            parameter_name: argument.parameter_name.clone(),
            expression_source_key: source_key,
            expression,
        });
    }

    (child_aliases, child_alias_expressions)
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_leaf_state_call_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeStraightLineBranchExpansion,
    target_key: StateKey,
    statement_index: usize,
    leaf_parameters: &[StateParameterFlow],
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    scratch: &mut StraightLineBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) = input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == expansion.dispatch_index
                && slot.source_key == target_key
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
    else {
        return;
    };

    let Some(initializer) =
        local_initializer_handle(input, &mut scratch.expressions, target_key, statement_index)
    else {
        return;
    };
    let resolved_initializer = resolve_leaf_call_expression_handle(
        input,
        &mut scratch.expressions,
        target_key,
        initializer,
        leaf_parameters,
        arguments,
        straight_line_bindings,
    );
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        statement_index,
        &scratch.expressions,
        slot,
        resolved_initializer,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        statement_index,
        &scratch.expressions,
        slot,
        resolved_initializer,
        &static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: target_key,
            source_statement: statement_index,
        });
    }
}

fn resolve_leaf_call_expression_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    target_key: StateKey,
    expression: ExpressionHandle,
    leaf_parameters: &[StateParameterFlow],
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> ExpressionHandle {
    match table.expression(expression).clone() {
        ExpressionNode::ArrayLiteral(values) => {
            let copied_values = table.reserve_expression_handles(values.count());
            for offset in 0..values.count() {
                let value = table.expression_handle_at_offset(values, offset);
                let resolved = resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    value,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                );
                table.set_expression_handle_at_offset(copied_values, offset, resolved);
            }
            table.insert(ExpressionNode::ArrayLiteral(copied_values))
        }
        ExpressionNode::Binary(binary) => {
            let left = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                binary.left,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            let right = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                binary.right,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Binary(
                psi_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                },
            ))
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                cast.value,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Cast(
                psi_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                    target_label: cast.target_label,
                    domain: cast.domain,
                    semantic_domain: cast.semantic_domain,
                    semantic_domain_arguments: cast.semantic_domain_arguments,
                    semantic_domain_symbol: cast.semantic_domain_symbol,
                    semantic_domain_id: cast.semantic_domain_id,
                    form: cast.form,
                },
            ))
        }
        ExpressionNode::Call(call) => {
            let receiver = call.receiver.is_valid().then(|| {
                resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    call.receiver,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                )
            });
            let copied_arguments = table.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument = table.expression_handle_at_offset(call.arguments, offset);
                let resolved = resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    argument,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                );
                table.set_expression_handle_at_offset(copied_arguments, offset, resolved);
            }
            table.insert(ExpressionNode::Call(
                psi_checked_trees::expression::TableCallExpression {
                    receiver: receiver.unwrap_or_else(ExpressionHandle::invalid),
                    target_symbol: call.target_symbol,
                    target: call.target,
                    machine_arguments: call.machine_arguments,
                    quotient_operation: call.quotient_operation,
                    private_layout_operation: call.private_layout_operation,
                    arguments: copied_arguments,
                    evidence_arguments: call.evidence_arguments,
                    operational_acknowledgement: call.operational_acknowledgement,
                },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                indexed.collection,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            let index = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                indexed.index,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Indexed(
                psi_checked_trees::expression::TableIndexedExpression { collection, index },
            ))
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                member.receiver,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            table.insert(ExpressionNode::Member(
                psi_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member,
                    case_variant: member.case_variant,
                },
            ))
        }
        ExpressionNode::Borrow(target) => {
            let resolved_target = resolve_leaf_call_expression_handle(
                input,
                table,
                target_key,
                target.target,
                leaf_parameters,
                arguments,
                straight_line_bindings,
            );
            if matches!(table.expression(resolved_target), ExpressionNode::Borrow(_)) {
                resolved_target
            } else {
                table.insert(ExpressionNode::Borrow(TableBorrowExpression {
                    target: resolved_target,
                    access: target.access,
                }))
            }
        }
        ExpressionNode::Name(path) if path.members.count() > 0 => resolve_leaf_call_name_handle(
            input,
            table,
            target_key,
            &path,
            leaf_parameters,
            arguments,
            straight_line_bindings,
        )
        .unwrap_or(expression),
        ExpressionNode::StructLiteral(struct_literal) => {
            let copied_fields = table.reserve_struct_fields(struct_literal.fields.count());
            for offset in 0..struct_literal.fields.count() {
                let field = table
                    .struct_field_at_offset(struct_literal.fields, offset)
                    .clone();
                let value = resolve_leaf_call_expression_handle(
                    input,
                    table,
                    target_key,
                    field.value,
                    leaf_parameters,
                    arguments,
                    straight_line_bindings,
                );
                table.set_struct_field_at_offset(
                    copied_fields,
                    offset,
                    psi_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        field_symbol: field.field_symbol,
                        value,
                    },
                );
            }
            table.insert(ExpressionNode::StructLiteral(
                psi_checked_trees::expression::TableStructLiteral {
                    type_name: struct_literal.type_name,
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name,
                    case_symbol: struct_literal.case_symbol,
                    fields: copied_fields,
                },
            ))
        }
        _ => expression,
    }
}

fn resolve_leaf_call_name_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    target_key: StateKey,
    path: &psi_checked_trees::expression::TableNamePath,
    leaf_parameters: &[StateParameterFlow],
    arguments: &[StateCallArgument],
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
) -> Option<ExpressionHandle> {
    if let Some(parameter_index) = leaf_parameters.iter().position(|parameter| {
        parameter.symbol.is_valid()
            && path.head_symbol.is_valid()
            && parameter.symbol == path.head_symbol
    }) {
        let argument = arguments.get(parameter_index)?;
        let argument_expression =
            table.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_argument = resolve_straight_line_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            table,
            argument_expression,
            straight_line_bindings,
        );

        return Some(table.insert_copy_with_member_suffix(
            resolved_argument,
            path.members,
            path.member_symbols,
            1,
        ));
    }

    let initializer = leaf_local_initializer_handle(input, table, target_key, path)?;
    let resolved_initializer = resolve_leaf_call_expression_handle(
        input,
        table,
        target_key,
        initializer,
        leaf_parameters,
        arguments,
        straight_line_bindings,
    );
    Some(table.insert_copy_with_member_suffix(
        resolved_initializer,
        path.members,
        path.member_symbols,
        1,
    ))
}

fn leaf_local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    target_key: StateKey,
    path: &psi_checked_trees::expression::TableNamePath,
) -> Option<ExpressionHandle> {
    let machine = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_key.machine)?;
    let state = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == target_key.state)?;
    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    statements.iter().find_map(|statement| {
        let psi_checked_trees::statement::StatementNode::LocalData(local_data) = statement else {
            return None;
        };
        let matches_symbol = path.head_symbol.is_valid() && local_data.symbol == path.head_symbol;
        (local_data.initial_value.is_valid() && matches_symbol)
            .then(|| table.copy_from(&input.program.expression_table, local_data.initial_value))
    })
}
