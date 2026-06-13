use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use omega_checked_trees::name::Identifier;
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_runtime_branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchExpansion, RuntimeLeafBranchOperationKind,
};
use omega_state_calls::StateCallRole;

use super::super::super::bindings::resolve_leaf_binding_expression_handle;
use super::super::super::lookups::state_mutation_for_statement;
use super::super::super::storage_places::resolve_runtime_storage_place_in_table;
use super::super::super::storage_places::{
    resolve_machine_owned_place, resolve_machine_owned_place_in_table,
    resolve_runtime_pointee_slot_offset_in_table, static_integer_value,
    static_integer_value_in_table,
};
use super::super::guards::{
    select_runtime_leaf_branch_guards, static_guard_conjunct_summary_in_table,
};
use super::super::text_writes::{
    runtime_text_builder_write_in_table_emit, runtime_text_builder_write_with_handle_resolver_emit,
};
use super::super::writes::{
    RuntimeStaticValues, emit_runtime_frame_slot_slice_descriptor_write_in_table,
    runtime_frame_slot_target_expression, runtime_storage_copy,
    runtime_storage_fixed_indexed_source_copy, select_runtime_frame_slot_value_write_in_table,
};
use super::mutation::select_runtime_resolved_mutation_write_in_table_with_scratch;
use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
};
use omega_runtime_dispatch_loop::RuntimeDispatchLoopAction;

fn supports_scalar_integer_write(byte_size: usize) -> bool {
    matches!(byte_size, 1 | 2 | 4 | 8)
}

#[derive(Default)]
pub(in crate::selection::runtime_dispatch) struct LeafBranchSelectionScratch {
    expressions: ExpressionTable,
    mutable_expressions: ExpressionTable,
    resolved_segment_expressions: ExpressionTable,
    fallback_segment_expressions: ExpressionTable,
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_leaf_branch_expansions_for_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    _expansion_cursor: &mut usize,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut matching_expansions = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            leaf_expansion_matches_operation(expansion, dispatch_index, operation, false)
        })
        .collect::<Vec<_>>();

    order_return_value_fallbacks_first(&mut matching_expansions);

    let multi_arm = matching_expansions.len() > 1;
    for expansion in matching_expansions {
        select_runtime_leaf_branch_expansion(
            input,
            expansion,
            multi_arm,
            scratch,
            runtime_value_operands,
            selected_instructions,
        );
    }
    if multi_arm {
        push_branch_arms_end_marker(
            operation.source_key,
            operation.statement_index,
            selected_instructions,
        );
    }
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_leaf_branch_expansions_matching_operation(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut matching_expansions = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            leaf_expansion_matches_operation(expansion, dispatch_index, operation, true)
        })
        .collect::<Vec<_>>();

    order_return_value_fallbacks_first(&mut matching_expansions);

    let multi_arm = matching_expansions.len() > 1;
    for expansion in matching_expansions {
        select_runtime_leaf_branch_expansion(
            input,
            expansion,
            multi_arm,
            scratch,
            runtime_value_operands,
            selected_instructions,
        );
    }
    if multi_arm {
        push_branch_arms_end_marker(
            operation.source_key,
            operation.statement_index,
            selected_instructions,
        );
    }
}

/// True when every leaf expansion matching this operation is an
/// ASSIGNMENT-VALUE selection (`let v = self.f(...)`) -- the deferral-safe
/// shape. The runtime-bodies splice lays the callee's effect operations out
/// BETWEEN the StateCall operation and the statement's LocalStorage operation,
/// so a value selection emitted at the StateCall reads the callee's
/// PRE-mutation state (the interpreter delivers the post-mutation value); the
/// dispatch loop defers it to the LocalStorage operation instead. Statements
/// that also carry call-argument or statement-role leafs keep the immediate
/// emission: their values feed sibling machinery emitted before the splice
/// completes.
pub(in crate::selection) fn leaf_expansions_defer_to_local_initializer(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
) -> bool {
    let mut any_matched = false;
    for (_, expansion) in input.runtime_branching_calls.leaf_expansions.iter() {
        if !leaf_expansion_matches_operation(expansion, dispatch_index, operation, false) {
            continue;
        }
        if expansion.role != StateCallRole::AssignmentValue {
            return false;
        }
        any_matched = true;
    }
    any_matched
}

fn leaf_expansion_matches_operation(
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
        && (matches!(
            operation.kind,
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
                | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
                | RuntimeDispatchBodyOperationKind::StateCall { .. }
        ) || (allow_synthetic_nested_operation
            && matches!(operation.kind, RuntimeDispatchBodyOperationKind::Other)))
}

fn order_return_value_fallbacks_first(expansions: &mut [&RuntimeLeafBranchExpansion]) {
    expansions.sort_by_key(|expansion| {
        (
            !(expansion.target_value.is_valid() && expansion.is_default_target),
            expansion.edge_order,
        )
    });
}

fn select_runtime_leaf_branch_expansion(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    multi_arm: bool,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let guards = select_runtime_leaf_branch_guards(input, expansion, runtime_value_operands);
    let static_summary = static_guard_conjunct_summary_in_table(
        input,
        &input.runtime_branching_calls.expressions,
        expansion.resolved_guard,
    );
    if static_summary.has_false {
        return;
    }
    if guards.is_empty()
        && expansion.guard_kind != omega_state_guards::StateGuardKind::Always
        && !static_summary.has_true
    {
        // An ASSIGNMENT-VALUE arm (`let n = self.m.f(..)`: the arm writes the
        // call's result slot) whose guard could not be RESOLVED (not
        // statically decided -- that returns above) must never be silently
        // dropped when the call stays INLINE: its compare AND its result
        // write both vanish, so the call returns a stale 0 (the slice-len
        // value-call guard bug). Emit a loud zero-width poison marker
        // instead; emission planning rejects any `UnresolvedInlineArmGuard`
        // left in the plan, failing the compile with a "needs lowering"
        // diagnostic.
        //
        // NOT poisoned (the skip is benign there -- another path emits the
        // arm's effect, or the gap is a separately-tracked lowering family):
        // - STATEMENT arms (guarded transitions to leaf states): arm
        //   selection has a dispatch-edge fallback (string-equality false
        //   arms, the dungeon).
        // - CALL-ARGUMENT / other roles: nested value-call arguments are
        //   covered by their own call expansions (MazeBuilder::connect).
        // - DISPATCHED value calls (a dispatch edge with a call_result at
        //   this statement): the dispatch return-write delivers the value
        //   (the recursive `weaken` termination canary).
        // - STRING-comparison guards (the guard compares against a string
        //   literal): text-guard lowering through refs/params is a known
        //   separate gap, and green compile canaries rely on the historical
        //   skip (calls/runtime_transition_argument_call_value).
        if expansion.target_value.is_valid()
            && expansion.role == StateCallRole::AssignmentValue
            && !statement_dispatches_call_result(
                input,
                expansion.dispatch_index,
                expansion.source_key,
                expansion.statement_index,
            )
            && !guard_contains_string_literal(
                &input.runtime_branching_calls.expressions,
                expansion.resolved_guard,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::EvaluateDispatchGuard {
                    guard_lowering:
                        omega_abstract_operations::StateGuardLowering::UnresolvedInlineArmGuard,
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
        return;
    }
    let guards_were_empty = guards.is_empty();
    let guard_start = selected_instructions.len();
    for guard in guards {
        selected_instructions.push(SelectedInstruction {
            kind: guard,
            source_key: expansion.source_key,
            source_statement: expansion.statement_index,
        });
    }

    let write_start = selected_instructions.len();
    select_runtime_leaf_nested_call_argument_writes(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_leaf_branch_terminal_value_write(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_leaf_assignment_value_target_copy(input, expansion, selected_instructions);
    // No call-result -> LOCAL-slot copy here: when the statement's local keeps
    // its LocalStorage slot, the dispatch loop's initializer write
    // (copy_assignment_value_call_result_into_local) emits that copy at the
    // LocalStorage operation -- always AFTER the callee's spliced mutations.
    // A leaf-side copy would either duplicate it (deferred emission lands at
    // the same operation) or sit at the StateCall, BEFORE the splice (the
    // stale-read position the deferral exists to avoid).
    select_runtime_leaf_local_initializer_writes(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_leaf_branch_mutation_writes(
        input,
        expansion,
        scratch,
        runtime_value_operands,
        selected_instructions,
    );
    select_runtime_leaf_branch_completion_dispatch(input, expansion, selected_instructions);
    if selected_instructions.len() == write_start {
        while selected_instructions.len() > guard_start {
            selected_instructions.pop();
        }
    } else if !guards_were_empty {
        // In a MULTI-arm transition, a matched (guarded) arm must skip the remaining
        // sibling arms, whose bodies would otherwise run and clobber this arm's
        // effect. Emit a forward jump to the transition's `BranchArmsEnd` marker.
        // The jump carries the leaf state's key (NOT expansion.source_key) so it is
        // not mistaken for the guard's failure-branch site boundary; the trailing
        // NoOp keeps expansion.source_key and stays the jne's landing target -- now
        // sitting AFTER the jump, so the jne auto-skips past it.
        if multi_arm {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::EvaluateDispatchGuard {
                    guard_lowering:
                        omega_abstract_operations::StateGuardLowering::ForwardBranchSkip,
                    operator: omega_abstract_operations::StateGuardOperator::Equal,
                    storage_region: RuntimeStorageRegion::Machine,
                    byte_offset: 0,
                    byte_size: 0,
                    expected_value: 0,
                    has_storage: false,
                    is_float: false,
                },
                source_key: expansion.leaf_key,
                source_statement: expansion.target_statement_index,
            });
        }
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EvaluateDispatchGuard {
                guard_lowering: omega_abstract_operations::StateGuardLowering::NoOp,
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

/// True when the guard expression compares against a STRING literal anywhere
/// in its conjunction -- a text guard. Text-guard lowering through refs and
/// params is a separately-tracked gap; such arms keep the historical silent
/// skip instead of the unresolved-guard poison.
fn guard_contains_string_literal(
    expressions: &ExpressionTable,
    guard: omega_checked_trees::expression::ExpressionHandle,
) -> bool {
    if !guard.is_valid() {
        return false;
    }
    match expressions.expression(guard) {
        omega_checked_trees::expression::ExpressionNode::String(_) => true,
        omega_checked_trees::expression::ExpressionNode::Binary(binary) => {
            guard_contains_string_literal(expressions, binary.left)
                || guard_contains_string_literal(expressions, binary.right)
        }
        omega_checked_trees::expression::ExpressionNode::Mutable(inner) => {
            guard_contains_string_literal(expressions, *inner)
        }
        _ => false,
    }
}

/// True when the dispatch loop carries a CALL-RESULT edge for this statement
/// in this dispatch context -- i.e. the statement's value call is DISPATCHED
/// and its result is delivered by the dispatch return-write, so the inline
/// leaf arms here are redundant and skipping them is benign.
fn statement_dispatches_call_result(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input.runtime_dispatch_loop.cases.iter().any(|(_, case)| {
        case.dispatch_index == dispatch_index
            && super::super::state_key_matches_statement_source(case.key, source_key)
            && input
                .runtime_dispatch_loop
                .edges
                .span(case.edges)
                .unwrap_or(&[])
                .iter()
                .any(|edge| edge.statement_index == statement_index && edge.call_result.is_some())
    })
}

fn select_runtime_leaf_nested_call_argument_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.role != StateCallRole::Statement {
        return;
    }

    let mut call_arguments = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|call_argument| {
            call_argument.dispatch_index == expansion.dispatch_index
                && super::super::state_key_matches_statement_source(
                    call_argument.source_key,
                    expansion.source_key,
                )
                && call_argument.statement_index == expansion.statement_index
                && call_argument.role == StateCallRole::CallArgument
        })
        .collect::<Vec<_>>();
    call_arguments
        .sort_by_key(|call_argument| (call_argument.call_ordinal, call_argument.edge_order));

    let mut group_start = 0;
    while group_start < call_arguments.len() {
        let call_ordinal = call_arguments[group_start].call_ordinal;
        let group_end = call_arguments[group_start..]
            .iter()
            .position(|call_argument| call_argument.call_ordinal != call_ordinal)
            .map_or(call_arguments.len(), |offset| group_start + offset);
        let group = &mut call_arguments[group_start..group_end];
        order_return_value_fallbacks_first(group);

        let multi_arm = group.len() > 1;
        for call_argument in group.iter().copied() {
            select_runtime_leaf_branch_expansion(
                input,
                call_argument,
                multi_arm,
                scratch,
                runtime_value_operands,
                selected_instructions,
            );
        }
        if multi_arm {
            push_branch_arms_end_marker(
                expansion.source_key,
                expansion.statement_index,
                selected_instructions,
            );
        }
        group_start = group_end;
    }
}

/// Emit the `BranchArmsEnd` marker that terminates a multi-arm guarded transition's
/// arms; it is the target of every `ForwardBranchSkip` emitted for that transition.
fn push_branch_arms_end_marker(
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: omega_abstract_operations::StateGuardLowering::BranchArmsEnd,
            operator: omega_abstract_operations::StateGuardOperator::Equal,
            storage_region: RuntimeStorageRegion::Machine,
            byte_offset: 0,
            byte_size: 0,
            expected_value: 0,
            has_storage: false,
            is_float: false,
        },
        source_key,
        source_statement: statement_index,
    });
}

fn select_runtime_leaf_branch_completion_dispatch(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if expansion.role != StateCallRole::Statement || expansion.target_value.is_valid() {
        return;
    }
    if !leaf_state_is_empty(input, expansion.leaf_key) {
        return;
    }
    let Some(dispatch_index) = source_completion_dispatch_index(input, expansion) else {
        return;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::SetDispatchState { dispatch_index },
        source_key: expansion.source_key,
        source_statement: expansion.statement_index,
    });
}

fn leaf_state_is_empty(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> bool {
    let Some(state) = input.control_flow.state_by_key(key) else {
        return false;
    };
    input
        .control_flow
        .operations
        .span(state.operations)
        .map_or(true, <[_]>::is_empty)
        && input
            .control_flow
            .transitions
            .span(state.transitions)
            .map_or(true, <[_]>::is_empty)
}

fn source_completion_dispatch_index(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<u32> {
    let case = input
        .runtime_dispatch_loop
        .cases
        .iter()
        .find_map(|(_, case)| (case.key == expansion.source_key).then_some(case))?;
    let edges = input.runtime_dispatch_loop.edges.span(case.edges)?;
    let mut targets = edges.iter().filter_map(|edge| match edge.action {
        RuntimeDispatchLoopAction::EnterState | RuntimeDispatchLoopAction::Terminate => {
            Some(edge.target_dispatch_index)
        }
        RuntimeDispatchLoopAction::Unknown => None,
    });
    let first = targets.next()?;
    targets.next().is_none().then_some(first)
}

fn select_runtime_leaf_branch_terminal_value_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if !expansion.target_value.is_valid() {
        return;
    }
    let value = expansion.target_value;
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
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    scratch.expressions.clear();
    let expressions = &mut scratch.expressions;
    let value = expressions.copy_from(&input.runtime_branching_calls.expressions, value);
    let resolved_value = resolve_leaf_binding_expression_handle(
        &input.runtime_branching_calls.expressions,
        expressions,
        value,
        bindings,
    );
    // A nested inline value call's terminal value can reference its CALLER's
    // fold-only locals: `chance`'s `roll < numerator` binds `numerator` to
    // should_carve's local `chance`, which has no frame slot (the planner
    // expects it to fold), so the leaf context cannot resolve the name as a
    // place and the whole result-slot write silently dropped (the dungeon's
    // side-room transitions never fired). Substitute such names with the
    // local's initializer, bindings re-applied.
    let resolved_value = resolve_leaf_caller_local_initializer_names(
        input,
        expansion,
        expressions,
        resolved_value,
        bindings,
        expansion.statement_index,
    );
    let static_values = RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    // Resolve in the CALLEE's context first (`branch_key`: the leaf value's own
    // names -- an attached callee's `self.field` -- live there), then retry in
    // the CALLER's context (`source_key`): after binding + caller-local
    // initializer substitution the value references CALLER places (`work(y)`
    // with `let y = self.v` resolves the terminal `y + 1` to the caller's
    // `self.v + 1`), which the callee context cannot resolve -- the result-slot
    // write silently dropped and the call returned a stale 0 (the by-value
    // arg-to-free-machine miscompile).
    let mut resolution_keys = [Some(expansion.branch_key), Some(expansion.source_key)];
    if expansion.branch_key == expansion.source_key {
        resolution_keys[1] = None;
    }
    for resolution_key in resolution_keys.into_iter().flatten() {
        if emit_runtime_frame_slot_slice_descriptor_write_in_table(
            input,
            expansion.dispatch_index,
            resolution_key,
            expansion.target_statement_index,
            &expressions,
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
            resolution_key,
            expansion.target_statement_index,
            &expressions,
            slot,
            resolved_value,
            &static_values,
            runtime_value_operands,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: resolution_key,
                source_statement: expansion.target_statement_index,
            });
            return;
        }

        let target = runtime_frame_slot_target_expression(expressions, slot);
        if select_runtime_resolved_mutation_write_in_table_with_scratch(
            input,
            expansion.dispatch_index,
            resolution_key,
            expansion.source_key,
            resolution_key,
            expansion.target_statement_index,
            &expressions,
            target,
            resolved_value,
            &mut scratch.mutable_expressions,
            &mut scratch.resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
    }

    // The non-table mutation-write fallback was a proven dead emitter (0 emissions
    // across the full canary suite and the dungeon stress sample's 470 reaches): the
    // `_in_table` path above handles every case that actually lowers. Removed in the
    // Phase 4 selection cleanup; an unhandled case here simply emits nothing, exactly
    // as before.
}

/// Substitute names that refer to the expansion SOURCE state's earlier `let`
/// locals with their initializer expressions, bindings re-applied. Only locals
/// WITHOUT a frame slot in this dispatch are substituted: a slot-less local is
/// one the storage planner expected to fold away, so a name reaching selection
/// can never resolve as a place -- without substitution the containing write is
/// silently dropped. Locals WITH a slot keep their name and resolve against
/// live storage. `statement_bound` restricts matching to locals declared
/// before the referencing statement; recursing with the matched local's own
/// index keeps initializer chains strictly decreasing, so this terminates.
fn resolve_leaf_caller_local_initializer_names(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    expressions: &mut ExpressionTable,
    expression: ExpressionHandle,
    bindings: &[RuntimeLeafBranchBinding],
    statement_bound: usize,
) -> ExpressionHandle {
    match expressions.expression(expression).clone() {
        ExpressionNode::Binary(binary) => {
            let left = resolve_leaf_caller_local_initializer_names(
                input,
                expansion,
                expressions,
                binary.left,
                bindings,
                statement_bound,
            );
            let right = resolve_leaf_caller_local_initializer_names(
                input,
                expansion,
                expressions,
                binary.right,
                bindings,
                statement_bound,
            );
            if left == binary.left && right == binary.right {
                return expression;
            }
            expressions.insert(ExpressionNode::Binary(
                omega_checked_trees::expression::TableBinaryExpression {
                    left,
                    operator: binary.operator,
                    right,
                },
            ))
        }
        ExpressionNode::Cast(cast) => {
            let value = resolve_leaf_caller_local_initializer_names(
                input,
                expansion,
                expressions,
                cast.value,
                bindings,
                statement_bound,
            );
            if value == cast.value {
                return expression;
            }
            expressions.insert(ExpressionNode::Cast(
                omega_checked_trees::expression::TableCastExpression {
                    value,
                    target_type: cast.target_type,
                },
            ))
        }
        ExpressionNode::Call(call) => {
            let mut changed = false;
            let receiver = if call.receiver.is_valid() {
                let resolved = resolve_leaf_caller_local_initializer_names(
                    input,
                    expansion,
                    expressions,
                    call.receiver,
                    bindings,
                    statement_bound,
                );
                changed |= resolved != call.receiver;
                resolved
            } else {
                call.receiver
            };
            let copied_arguments = expressions.reserve_expression_handles(call.arguments.count());
            for offset in 0..call.arguments.count() {
                let argument = expressions.expression_handle_at_offset(call.arguments, offset);
                let resolved = resolve_leaf_caller_local_initializer_names(
                    input,
                    expansion,
                    expressions,
                    argument,
                    bindings,
                    statement_bound,
                );
                changed |= resolved != argument;
                expressions.set_expression_handle_at_offset(copied_arguments, offset, resolved);
            }
            if !changed {
                return expression;
            }
            expressions.insert(ExpressionNode::Call(
                omega_checked_trees::expression::TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    arguments: copied_arguments,
                },
            ))
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_leaf_caller_local_initializer_names(
                input,
                expansion,
                expressions,
                member.receiver,
                bindings,
                statement_bound,
            );
            if receiver == member.receiver {
                return expression;
            }
            // Project a STRUCT-LITERAL receiver onto the named field's value:
            // `job.id` with slot-less `job` substituted by its initializer
            // `Job { id: self.v }` folds to `self.v`. A literal has no storage
            // place, so an unprojected `<literal>.id` member can never resolve
            // and the containing write would silently drop (the by-value
            // struct-arg-to-free-machine miscompile).
            if let ExpressionNode::StructLiteral(struct_literal) =
                expressions.expression(receiver).clone()
            {
                for offset in 0..struct_literal.fields.count() {
                    let field = expressions
                        .struct_field_at_offset(struct_literal.fields, offset)
                        .clone();
                    if field.name == member.member {
                        return field.value;
                    }
                }
            }
            expressions.insert(ExpressionNode::Member(
                omega_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                },
            ))
        }
        ExpressionNode::Mutable(inner) => {
            let resolved = resolve_leaf_caller_local_initializer_names(
                input,
                expansion,
                expressions,
                inner,
                bindings,
                statement_bound,
            );
            if resolved == inner {
                return expression;
            }
            expressions.insert(ExpressionNode::Mutable(resolved))
        }
        // A STRUCT-LITERAL terminal value (`Pair { a: x, b: x + 1 }` returned
        // by value) carries the substitution into each FIELD value: after
        // binding, fields reference CALLER locals (`seed`) that may have no
        // frame slot (the planner expects them to fold), so without the
        // recursion every per-field result-slot write silently dropped (the
        // by-value struct-RETURN miscompile).
        ExpressionNode::StructLiteral(struct_literal) => {
            let mut changed = false;
            let copied_fields = expressions.reserve_struct_fields(struct_literal.fields.count());
            for offset in 0..struct_literal.fields.count() {
                let field = expressions
                    .struct_field_at_offset(struct_literal.fields, offset)
                    .clone();
                let resolved = resolve_leaf_caller_local_initializer_names(
                    input,
                    expansion,
                    expressions,
                    field.value,
                    bindings,
                    statement_bound,
                );
                changed |= resolved != field.value;
                expressions.set_struct_field_at_offset(
                    copied_fields,
                    offset,
                    omega_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        value: resolved,
                    },
                );
            }
            if !changed {
                return expression;
            }
            expressions.insert(ExpressionNode::StructLiteral(
                omega_checked_trees::expression::TableStructLiteral {
                    type_name: struct_literal.type_name,
                    case_name: struct_literal.case_name,
                    fields: copied_fields,
                },
            ))
        }
        ExpressionNode::Name(path) => {
            if path.members.count() != 1 {
                return expression;
            }
            let Some(machine) = input
                .program
                .machines()
                .iter()
                .find(|machine| machine.symbol == expansion.source_key.machine)
            else {
                return expression;
            };
            let Some(state) = input
                .program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == expansion.source_key.state)
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
                // Match by SYMBOL when the path carries one (distinct same-named
                // locals stay distinct); fall back to the name for symbol-less
                // paths (the shape leaf binding expressions actually carry).
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
            // A local backed by LIVE storage keeps its name: either its own
            // LocalStorage slot, or -- when its initializer is a state CALL --
            // the call's RESULT slot (it carries the local's name and holds the
            // runtime value). Substituting a call-result-backed local with its
            // initializer plants a Call expression no write strategy can lower
            // at runtime (the chained-call struct-RETURN miscompile: a field
            // value `seed` with `let seed = Worker::bump(30)` substituted to
            // the call and the whole field write silently dropped).
            let has_slot = input.runtime_storage.frame_slots.iter().any(|(_, slot)| {
                slot.dispatch_index == expansion.dispatch_index
                    && slot.source_key == expansion.source_key
                    && slot.statement_index == local_index
                    && matches!(
                        slot.kind,
                        omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                            | omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult {
                                role: StateCallRole::AssignmentValue,
                                ..
                            }
                    )
            });
            if has_slot {
                return expression;
            }
            let initializer = expressions.copy_from(&input.program.expression_table, initial_value);
            let bound = resolve_leaf_binding_expression_handle(
                &input.runtime_branching_calls.expressions,
                expressions,
                initializer,
                bindings,
            );
            resolve_leaf_caller_local_initializer_names(
                input,
                expansion,
                expressions,
                bound,
                bindings,
                local_index,
            )
        }
        _ => expression,
    }
}

fn select_runtime_leaf_assignment_value_target_copy(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
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
            kind: SelectedInstructionKind::CopyRuntimeStorageToRuntimePointee {
                source_region: RuntimeStorageRegion::RuntimeFrame,
                source_offset: source_slot.byte_offset,
                pointer_byte_offset: pointer_target.pointer_byte_offset,
                field_byte_offset: pointer_target.field_byte_offset,
                byte_count: source_slot.byte_size,
            },
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
        kind: SelectedInstructionKind::CopyRuntimeStorage {
            source_region: RuntimeStorageRegion::RuntimeFrame,
            source_offset: source_slot.byte_offset,
            target_region: target_place.region,
            target_offset: target_place.byte_offset,
            byte_count: source_slot.byte_size,
        },
        source_key: expansion.source_key,
        source_statement: expansion.statement_index,
    });
}

fn select_runtime_leaf_local_initializer_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(operations) = input
        .runtime_branching_calls
        .leaf_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        if !matches!(operation.kind, RuntimeLeafBranchOperationKind::Other) {
            continue;
        }
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
            continue;
        };

        scratch.expressions.clear();
        let Some(initializer) = local_initializer_handle(
            input,
            &mut scratch.expressions,
            operation.source_key,
            operation.statement_index,
        ) else {
            continue;
        };
        let expressions = &mut scratch.expressions;
        let resolved_initializer = resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            expressions,
            initializer,
            bindings,
        );
        let static_values =
            RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
        if emit_runtime_frame_slot_slice_descriptor_write_in_table(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            operation.statement_index,
            expressions,
            slot,
            resolved_initializer,
            runtime_value_operands,
            selected_instructions,
        ) {
            continue;
        }
        if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
            input,
            expansion.dispatch_index,
            expansion.source_key,
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
        }
    }
}

fn local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> Option<omega_checked_trees::expression::ExpressionHandle> {
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

fn select_runtime_leaf_branch_mutation_writes(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(operations) = input
        .runtime_branching_calls
        .leaf_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = input
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    scratch.expressions.clear();
    scratch.resolved_segment_expressions.clear();
    scratch.fallback_segment_expressions.clear();

    for operation in operations {
        let RuntimeLeafBranchOperationKind::Mutation {
            lowering,
            target,
            value,
            ..
        } = &operation.kind
        else {
            continue;
        };
        let target_source_key =
            if *lowering == omega_state_storage::StateMutationLowering::AlreadyLowered {
                operation.source_key
            } else {
                expansion.source_key
            };
        scratch.expressions.clear();
        let expressions = &mut scratch.expressions;
        let target = expressions.copy_from(&input.runtime_branching_calls.expressions, *target);
        let value = expressions.copy_from(&input.runtime_branching_calls.expressions, *value);
        let resolved_target = resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            expressions,
            target,
            bindings,
        );
        let resolved_value = resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            expressions,
            value,
            bindings,
        );
        if let Some((byte_offset, byte_size, value)) = runtime_leaf_machine_integer_write_in_table(
            input,
            expansion,
            &expressions,
            resolved_target,
            resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                    byte_offset,
                    byte_size,
                    value,
                },
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
            continue;
        }

        let mutation_emitted = select_runtime_resolved_mutation_write_in_table_with_scratch(
            input,
            expansion.dispatch_index,
            operation.source_key,
            target_source_key,
            expansion.source_key,
            operation.statement_index,
            &expressions,
            resolved_target,
            resolved_value,
            &mut scratch.mutable_expressions,
            &mut scratch.resolved_segment_expressions,
            runtime_value_operands,
            selected_instructions,
        );
        if mutation_emitted {
            continue;
        }

        scratch.resolved_segment_expressions.clear();
        if runtime_text_builder_write_in_table_emit(
            input,
            expansion.dispatch_index,
            operation.source_key,
            target_source_key,
            operation.statement_index,
            &expressions,
            resolved_target,
            &mut scratch.resolved_segment_expressions,
            &|expressions, expression| {
                resolve_leaf_binding_expression_handle(
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
        ) {
            continue;
        }

        let resolved_target = expressions.to_tree(resolved_target);
        let resolved_value = expressions.to_tree(resolved_value);
        if let Some((byte_offset, byte_size, value)) =
            runtime_leaf_machine_integer_write(input, expansion, &resolved_target, &resolved_value)
        {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                    byte_offset,
                    byte_size,
                    value,
                },
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
            continue;
        }

        let (operation_machine, operation_state) = state_names(input, operation.source_key);
        scratch.fallback_segment_expressions.clear();
        if runtime_text_builder_write_with_handle_resolver_emit(
            input,
            expansion.dispatch_index,
            operation.source_key,
            &operation_machine,
            &operation_state,
            operation.statement_index,
            &resolved_target,
            &mut scratch.fallback_segment_expressions,
            &|expressions, expression| {
                resolve_leaf_binding_expression_handle(
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
        ) {
            continue;
        }

        if let Some(copy) = runtime_leaf_storage_copy(
            input,
            expansion,
            &operation_machine,
            &operation_state,
            &resolved_target,
            &resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: copy,
                source_key: operation.source_key,
                source_statement: operation.statement_index,
            });
        }
    }
}

fn state_names(
    input: &InstructionSelectionInput<'_>,
    key: omega_control_flow::StateKey,
) -> (Identifier, Identifier) {
    input.control_flow.state_names_by_key_cloned(key)
}

fn runtime_leaf_machine_integer_write(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    target: &Expression,
    value_expression: &Expression,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &input.layouts,
        input.entry_key.machine,
        expansion.source_key.machine,
        target,
    )?;
    if !supports_scalar_integer_write(byte_size) {
        return None;
    }
    let value = static_integer_value(&input.layouts, value_expression)?;

    Some((byte_offset, byte_size, value))
}

fn runtime_leaf_machine_integer_write_in_table(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    expressions: &ExpressionTable,
    target: omega_checked_trees::expression::ExpressionHandle,
    value_expression: omega_checked_trees::expression::ExpressionHandle,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place_in_table(
        &input.layouts,
        input.entry_key.machine,
        expansion.source_key.machine,
        expressions,
        target,
    )?;
    if !supports_scalar_integer_write(byte_size) {
        return None;
    }
    let value = static_integer_value_in_table(&input.layouts, expressions, value_expression)?;
    Some((byte_offset, byte_size, value))
}

fn runtime_leaf_storage_copy(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    operation_machine: &str,
    operation_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    runtime_storage_fixed_indexed_source_copy(
        input,
        expansion.dispatch_index,
        expansion.source_key,
        expansion.source_key,
        operation_machine,
        operation_state,
        target,
        value,
    )
    .or_else(|| {
        runtime_storage_copy(
            input,
            expansion.dispatch_index,
            expansion.source_key,
            expansion.source_key,
            operation_machine,
            operation_state,
            target,
            value,
        )
    })
}
