use crate::InstructionSelectionInput;
use omega_control_flow::StateKey;
use omega_runtime_bodies::RuntimeDispatchBodyOperation;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use omega_runtime_branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchExpansion, RuntimeLeafBranchOperationKind,
};
use omega_state_calls::StateCallRole;
use psi_arena::Arena;
use psi_checked_trees::expression::{
    Expression, ExpressionHandle, ExpressionNode, ExpressionTable, TableBorrowExpression,
};
use psi_checked_trees::name::Identifier;
use psi_checked_trees::statement::StatementNode;

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
    select_runtime_frame_slot_value_write_in_table_with_call_ordinal,
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
    if operation_has_straight_line_expansions(input, dispatch_index, operation, false) {
        return;
    }
    let mut matching_expansions = input
        .runtime_branching_calls
        .leaf_expansions
        .storage_slice()
        .iter()
        .filter(|expansion| {
            leaf_expansion_matches_operation(expansion, dispatch_index, operation, false)
        })
        .collect::<Vec<_>>();

    order_return_value_fallbacks_last(&mut matching_expansions);

    let multi_arm = matching_expansions.len() > 1;
    let scope_id = matching_expansions
        .first()
        .map_or(0, |expansion| expansion.scope_id);
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
            scope_id,
            operation.source_key,
            operation.statement_index,
            selected_instructions,
        );
    }
}

fn operation_has_straight_line_expansions(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    operation: &RuntimeDispatchBodyOperation,
    allow_synthetic_nested_operation: bool,
) -> bool {
    input
        .runtime_branching_calls
        .straight_line_expansions
        .storage_slice()
        .iter()
        .any(|expansion| {
            expansion.dispatch_index == dispatch_index
                && super::super::state_key_matches_statement_source(
                    expansion.source_key,
                    operation.source_key,
                )
                && expansion.statement_index == operation.statement_index
                && match operation.kind {
                    RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                        role,
                        call_ordinal,
                        ..
                    }
                    | RuntimeDispatchBodyOperationKind::InlineStateCall {
                        role,
                        call_ordinal,
                        ..
                    }
                    | RuntimeDispatchBodyOperationKind::StateCall {
                        role, call_ordinal, ..
                    } => expansion.role == role && expansion.call_ordinal == call_ordinal,
                    RuntimeDispatchBodyOperationKind::Other => allow_synthetic_nested_operation,
                    _ => false,
                }
        })
}

pub(in crate::selection::runtime_dispatch) fn select_runtime_leaf_branch_expansion_for_tree(
    input: &InstructionSelectionInput<'_>,
    expansion: &RuntimeLeafBranchExpansion,
    use_local_guard: bool,
    has_following_arm: bool,
    scratch: &mut LeafBranchSelectionScratch,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if use_local_guard {
        let mut local = expansion.clone();
        local.resolved_guard = local.guard;
        local.guard_kind = local.local_guard_kind;
        select_runtime_leaf_branch_expansion(
            input,
            &local,
            has_following_arm,
            scratch,
            runtime_value_operands,
            selected_instructions,
        );
    } else {
        select_runtime_leaf_branch_expansion(
            input,
            expansion,
            has_following_arm,
            scratch,
            runtime_value_operands,
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
        // TransitionArgument leaves defer like AssignmentValue ones: the
        // capture (callee result -> the call's arg slot) must run AFTER the
        // callee's spliced body ops, or it copies the internal `let` slot's
        // pre-body ZII (`done(self.dbl(5), 7)` delivered a=0).
        if !matches!(
            expansion.role,
            StateCallRole::AssignmentValue | StateCallRole::TransitionArgument
        ) {
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
        && match operation.kind {
            // Pair the expansion with ITS OWN call op: a transition passing
            // two value calls (`done(self.dbl(5), self.dbl(6))`) has two
            // same-statement StateCall ops, and a statement-keyed match
            // emitted EVERY expansion at EVERY call op -- both captures fired
            // at call 1 (before call 2's body ran) and each fired twice.
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

/// Emit guarded terminal-value arms in source order and the transition's
/// default arm last. A matched guarded arm emits `ForwardBranchSkip`, so a
/// default emitted first would skip its guarded siblings before they are
/// tested.
fn order_return_value_fallbacks_last(expansions: &mut [&RuntimeLeafBranchExpansion]) {
    expansions.sort_by_key(|expansion| {
        (
            expansion.target_value.is_valid() && expansion.is_default_target,
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
    selected_instructions.begin_permission_site(
        expansion.branch_key,
        expansion.target_statement_index,
        None,
        None,
    );
    // Inline-branching calls defer concrete selection from the caller's
    // StateCall operation to this expansion. Join the emitted span back to
    // the exact caller call ordinal and to the called state's entry, while
    // retaining the callee terminal site's events installed above.
    selected_instructions.include_permission_events_for_site(
        expansion.source_key,
        expansion.statement_index,
        Some(Some(expansion.call_ordinal)),
        Some(expansion.branch_key.state),
    );
    selected_instructions.include_state_entry_permission_events(expansion.branch_key);
    let guards = select_runtime_leaf_branch_guards(input, expansion, runtime_value_operands);
    // The static summary runs on the BINDING-RESOLVED guard: an inline arm
    // guard over a substituted literal (`"a/b/c".len > 0`) is statically
    // decidable only after the caller's argument replaces the callee param.
    let summary_bindings = input
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);
    let mut summary_expressions =
        psi_checked_trees::expression::ExpressionTable::with_expression_capacity(8);
    let summary_guard = if expansion.resolved_guard.is_valid() {
        let copied = summary_expressions.copy_from(
            &input.runtime_branching_calls.expressions,
            expansion.resolved_guard,
        );
        crate::selection::bindings::resolve_leaf_binding_expression_handle(
            &input.runtime_branching_calls.expressions,
            &mut summary_expressions,
            copied,
            summary_bindings,
        )
    } else {
        expansion.resolved_guard
    };
    let static_summary =
        static_guard_conjunct_summary_in_table(input, &summary_expressions, summary_guard);
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
        //
        // STRING/TEXT-comparison guards are NO LONGER exempted: a text guard
        // through a place, ref param, or slice-element pointee now lowers (the
        // value-position text descriptor + carrier pointee compares), so a
        // residual unresolved text guard is a real silent-drop bug and must
        // poison loudly like any other -- not slip through as a stale 0 (the
        // value-call slice-element carrier arm-drop, byval_elem).
        if expansion.target_value.is_valid()
            && expansion.role == StateCallRole::AssignmentValue
            && !statement_dispatches_call_result(
                input,
                expansion.dispatch_index,
                expansion.source_key,
                expansion.statement_index,
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
    if !guards_were_empty {
        push_branch_scope_marker(
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
    // TERMINAL-VALUE expansions emit ARM-LOCAL INITIALIZERS FIRST -- source
    // order: the arm's `let b = self.name == ".."` must write its slot
    // BEFORE the terminal value write copies that slot out (emitting them
    // after left the copy reading the unwritten slot: the multiarm texteq
    // divergence's final layer). InlineLeaf expansions keep the historical
    // late position below.
    if !expansion.leaf_key.is_valid() {
        select_runtime_leaf_local_initializer_writes(
            input,
            expansion,
            scratch,
            runtime_value_operands,
            selected_instructions,
        );
    }
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
    if expansion.leaf_key.is_valid() {
        select_runtime_leaf_local_initializer_writes(
            input,
            expansion,
            scratch,
            runtime_value_operands,
            selected_instructions,
        );
    }
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
                    byte_offset: expansion.scope_id as usize,
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
        order_return_value_fallbacks_last(group);

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
                group.first().map_or(0, |expansion| expansion.scope_id),
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
pub(in crate::selection::runtime_dispatch) fn push_branch_arms_end_marker(
    scope_id: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: omega_abstract_operations::StateGuardLowering::BranchArmsEnd,
            operator: omega_abstract_operations::StateGuardOperator::Equal,
            storage_region: RuntimeStorageRegion::Machine,
            byte_offset: scope_id as usize,
            byte_size: 0,
            expected_value: 0,
            has_storage: false,
            is_float: false,
        },
        source_key,
        source_statement: statement_index,
    });
}

pub(in crate::selection::runtime_dispatch) fn push_branch_scope_marker(
    scope_id: u32,
    source_key: omega_control_flow::StateKey,
    statement_index: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    // Zero-width marker consumed by branch-distance planning. i64::MIN keeps
    // it distinguishable from ordinary guard-closing NoOps.
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: omega_abstract_operations::StateGuardLowering::NoOp,
            operator: omega_abstract_operations::StateGuardOperator::Equal,
            storage_region: RuntimeStorageRegion::Machine,
            byte_offset: scope_id as usize,
            byte_size: 0,
            expected_value: i64::MIN,
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

/// Emit write instructions that deliver the leaf branch's terminal value into
/// the call-result slot for the expansion's statement.
///
/// # Four-layer substitution pipeline
///
/// Terminal values frequently reference names and locals from the CALLER's
/// context, not the leaf's own frame. Each fix below addressed a class of
/// silent write drops where a name could not be resolved and the entire
/// result-slot write vanished (call returned a stale zero):
///
/// 1. **Binding application** (`resolve_leaf_binding_expression_handle`): the
///    leaf expansion carries `bindings` that map parameter names to their
///    call-site expressions. Applied first so the terminal value references
///    caller-side names after this step.
///
/// 2. **Caller-local initializer substitution**
///    (`resolve_leaf_caller_local_initializer_names`): names that refer to
///    fold-only locals in the CALLER state (locals without a frame slot
///    because the storage planner expected them to be folded away) cannot be
///    resolved as runtime places. This pass substitutes them with their
///    initializer expressions, keeping the substitution strictly
///    decreasing (local_index bound) so it terminates. Also handles
///    `StructLiteral` receivers (projects the named field out) and guards
///    against substituting call-result-backed locals with their raw call
///    expression (which no write strategy can lower).
///
/// 3. **Triple-context resolution loop** (`resolution_keys`): after
///    substitution the expression may reference names from the CALL-TARGET
///    context (the arm-owning callee state that spelled a chained call's
///    args -- tried FIRST, because the branch-key attempt's case-wide name
///    fallback can match a same-named slot of another scope; the
///    nested-inline result scramble, 2026-07-11e), the CALLEE context
///    (`branch_key`), or the CALLER context (`source_key`). All are tried
///    in order so that, e.g., an attached callee's `self.field` resolves in
///    the callee context while a free-machine result that folds to a
///    caller-side expression resolves in the caller context.
///
/// 4. **Write-strategy cascade**: for each resolution key, three strategies
///    are attempted in order:
///    - `emit_runtime_frame_slot_slice_descriptor_write_in_table` — slice/
///      text-window fat descriptors.
///    - `select_runtime_frame_slot_value_write_in_table` — scalar integers and
///      pointer-sized values.
///    - `select_runtime_resolved_mutation_write_in_table_with_scratch` — struct
///      decomposition, storage copies, and other compound forms.
///
/// # Known-safe fallthrough
///
/// When all strategies fail for all resolution keys, **nothing is emitted**.
/// This is intentional: unimplemented text-guard lowering through refs/params
/// reaches this point, and emitting nothing is equivalent to the historical
/// behaviour before the strategies were added. The `UnresolvedInlineArmGuard`
/// poison (emitted higher up, before this function) converts the assignment-
/// value inline-guard case to a compile error; this fallthrough covers
/// remaining gaps that are not yet poisoned.
///
/// The slot's `byte_size` is always `> 0` when this function is called (the
/// storage planner in `omega-runtime-storage` skips slot creation for zero-
/// size return types at `body.rs:851`).
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
    // The CALL-TARGET scope joins the loop for BARE-NAME terminals only: a
    // chained inline value call's arm args are spelled in the CALLEE's
    // arm-owning state (`Main -> holder.run()`: run.run's `done(total)`
    // names RUN's local), which is neither branch_key (the arm's TARGET
    // state, `done`) nor source_key (the top case). A statement with
    // anything but exactly one target contributes no key.
    let call_target_key = {
        let mut targets = input.state_calls.calls.iter().filter_map(|(_, call)| {
            (call.source_key.machine == expansion.source_key.machine
                && call.source_key.state == expansion.source_key.state
                && call.statement_index == expansion.statement_index)
                .then_some(call.target_key)
        });
        let first = targets.next();
        match (first, targets.next()) {
            (Some(target), None) => Some(target),
            _ => None,
        }
    };
    // BARE-NAME terminals resolve ONLY through frame slots, so key order is
    // decided by SCOPE-STRICT resolvability (runtime_frame_slot_for_
    // expression_scoped): a key that owns no slot for the name is skipped
    // outright -- attempting it would reach the case-wide NAME fallback and
    // match a same-named slot of another scope. Two pinned failure modes of
    // naive orderings: branch-first let `done` (slotless) steal Main's
    // `total` via the fallback (the nested-inline result scramble); a
    // target-first WITHOUT the gate stole each idx-arm's own `b` for the
    // callee entry's `b` (account_ledger, three same-named arm locals --
    // regressed 465b82bbf, caught by samples_with_documented_exit).
    // Non-name terminals keep the pre-existing [branch, source] order and
    // lenient behavior; when NO key strictly resolves a bare name, the
    // ungated pre-existing order applies too (old fallback semantics).
    let value_is_bare_name = matches!(
        expressions.expression(resolved_value),
        ExpressionNode::Name(_)
    );
    // The target scope's key comes from the SLOT side (unique-per-machine
    // by name; state symbols differ between the state-call and control-flow
    // layers, so target_key.state cannot be compared with slot keys).
    let target_resolution_key = call_target_key
        .filter(|_| value_is_bare_name)
        .and_then(|target| {
            crate::selection::storage_places::unique_machine_frame_slot_key_for_expression(
                input,
                expansion.dispatch_index,
                target.machine,
                &expressions,
                resolved_value,
            )
        });
    let mut resolution_keys = [
        Some(expansion.branch_key),
        target_resolution_key,
        Some(expansion.source_key),
    ];
    if resolution_keys[1] == resolution_keys[0] {
        resolution_keys[1] = None;
    }
    if expansion.branch_key == expansion.source_key {
        resolution_keys[2] = None;
    }
    if value_is_bare_name {
        let any_strict = resolution_keys.iter().flatten().any(|key| {
            crate::selection::storage_places::runtime_frame_slot_for_expression_scoped(
                input,
                expansion.dispatch_index,
                *key,
                &expressions,
                resolved_value,
            )
            .is_some()
        });
        if any_strict {
            for slot_key in resolution_keys.iter_mut() {
                let strict = slot_key.is_some_and(|key| {
                    crate::selection::storage_places::runtime_frame_slot_for_expression_scoped(
                        input,
                        expansion.dispatch_index,
                        key,
                        &expressions,
                        resolved_value,
                    )
                    .is_some()
                });
                if !strict {
                    *slot_key = None;
                }
            }
        }
    }
    if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() {
        eprintln!(
            "LEAFWRITE: dispatch {} source m{} s{} branch m{} s{} target {:?} stmt {} slot@{} bare {} keys {:?}",
            expansion.dispatch_index,
            expansion.source_key.machine.arena_index(),
            expansion.source_key.state.arena_index(),
            expansion.branch_key.machine.arena_index(),
            expansion.branch_key.state.arena_index(),
            call_target_key.map(|key| (key.machine.arena_index(), key.state.arena_index())),
            expansion.statement_index,
            slot.byte_offset,
            value_is_bare_name,
            resolution_keys
                .iter()
                .map(|key| key.map(|key| {
                    (
                        key.machine.arena_index(),
                        key.state.arena_index(),
                        key.segment_index,
                    )
                }))
                .collect::<Vec<_>>(),
        );
    }
    for resolution_key in resolution_keys.into_iter().flatten() {
        // Expressions substituted from the caller (including a nested value
        // call used as this call's argument) keep the CALLER statement's
        // result-slot identity. Callee-owned terminal expressions still use
        // the terminal statement index for locals and fields.
        let resolution_is_caller =
            super::super::state_key_matches_statement_source(resolution_key, expansion.source_key);
        let resolution_statement_index = if resolution_is_caller {
            expansion.statement_index
        } else {
            expansion.target_statement_index
        };
        if emit_runtime_frame_slot_slice_descriptor_write_in_table(
            input,
            expansion.dispatch_index,
            resolution_key,
            resolution_statement_index,
            &expressions,
            slot,
            resolved_value,
            runtime_value_operands,
            selected_instructions,
        ) {
            return;
        }
        let kind = if resolution_is_caller {
            select_runtime_frame_slot_value_write_in_table_with_call_ordinal(
                input,
                expansion.dispatch_index,
                resolution_key,
                resolution_statement_index,
                &expressions,
                slot,
                resolved_value,
                expansion.call_ordinal.saturating_add(1),
                &static_values,
                runtime_value_operands,
            )
        } else {
            select_runtime_frame_slot_value_write_in_table(
                input,
                expansion.dispatch_index,
                resolution_key,
                resolution_statement_index,
                &expressions,
                slot,
                resolved_value,
                &static_values,
                runtime_value_operands,
            )
        };
        if let Some(kind) = kind {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key: resolution_key,
                source_statement: resolution_statement_index,
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
            resolution_statement_index,
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
    // as before -- EXCEPT a bare CALL terminal, poisoned below.
    //
    // A TERMINAL VALUE that is a bare CALL no strategy lowered is a
    // host-boundary call in value-return position (`machine close(..) -> i32
    // { self.host.close(fd) }` -- any UNRESOLVED machine/state value call was
    // already a frontend error). Silently emitting nothing made the call
    // vanish and its result slot read ZII 0 (Filesystem::close reported
    // "success" while the fd stayed open). Poison instead; emission planning
    // rejects it with the bind-to-a-`let` diagnostic.
    if matches!(
        scratch.expressions.expression(resolved_value),
        ExpressionNode::Call(_)
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
            source_key: expansion.branch_key,
            source_statement: expansion.statement_index,
        });
    }
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
///
/// This is layer 2 of the four-layer substitution pipeline described on
/// `select_runtime_leaf_branch_terminal_value_write`. Call sites apply bindings
/// (layer 1) before calling this function, and the dual-context resolution loop
/// (layer 3) retries write strategies after it returns.
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
                psi_checked_trees::expression::TableBinaryExpression {
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
                psi_checked_trees::expression::TableCallExpression {
                    receiver,
                    target_symbol: call.target_symbol,
                    target: call.target.clone(),
                    machine_arguments: call.machine_arguments.clone(),
                    quotient_operation: call.quotient_operation.clone(),
                    private_layout_operation: call.private_layout_operation.clone(),
                    arguments: copied_arguments,
                    evidence_arguments: call.evidence_arguments.clone(),
                    operational_acknowledgement: call.operational_acknowledgement,
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
            // Project a STRUCT-LITERAL receiver onto the named field's value:
            // `job.id` with slot-less `job` substituted by its initializer
            // `Job { id: self.v }` folds to `self.v`. A literal has no storage
            // place, so an unprojected `<literal>.id` member can never resolve
            // and the containing write would silently drop (the by-value
            // struct-arg-to-free-machine miscompile).
            let literal = match expressions.expression(receiver).clone() {
                ExpressionNode::StructLiteral(literal) => Some(literal),
                // By-value parameter bindings retain a `Mutable` wrapper while
                // being substituted into a leaf. A field read is still a value
                // projection from that literal, not an addressable write.
                ExpressionNode::Borrow(inner) => {
                    match expressions.expression(inner.target).clone() {
                        ExpressionNode::StructLiteral(literal) => Some(literal),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some(struct_literal) = literal {
                for offset in 0..struct_literal.fields.count() {
                    let field = expressions
                        .struct_field_at_offset(struct_literal.fields, offset)
                        .clone();
                    if field.name == member.member {
                        return field.value;
                    }
                }
            }
            if receiver == member.receiver {
                return expression;
            }
            expressions.insert(ExpressionNode::Member(
                psi_checked_trees::expression::TableMemberExpression {
                    receiver,
                    member_symbol: member.member_symbol,
                    member: member.member.clone(),
                    case_variant: member.case_variant.clone(),
                },
            ))
        }
        ExpressionNode::Borrow(inner) => {
            let resolved = resolve_leaf_caller_local_initializer_names(
                input,
                expansion,
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
                    psi_checked_trees::expression::TableStructLiteralField {
                        name: field.name,
                        field_symbol: field.field_symbol,
                        value: resolved,
                    },
                );
            }
            if !changed {
                return expression;
            }
            expressions.insert(ExpressionNode::StructLiteral(
                psi_checked_trees::expression::TableStructLiteral {
                    type_name: struct_literal.type_name,
                    type_symbol: struct_literal.type_symbol,
                    case_name: struct_literal.case_name,
                    case_symbol: struct_literal.case_symbol,
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
        if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() {
            eprintln!(
                "LEAFINIT: op m{} s{} stmt {} kind Other? {}",
                operation.source_key.machine.arena_index(),
                operation.source_key.state.arena_index(),
                operation.statement_index,
                matches!(operation.kind, RuntimeLeafBranchOperationKind::Other),
            );
        }
        if !matches!(operation.kind, RuntimeLeafBranchOperationKind::Other) {
            continue;
        }
        let slot_lookup = input
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
            });
        if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() {
            eprintln!(
                "LEAFINIT:   slot? {}",
                slot_lookup
                    .map(|slot| slot.byte_offset as i64)
                    .unwrap_or(-1),
            );
        }
        let Some(slot) = slot_lookup else {
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
            continue;
        }
        // The TEXT-EQUALITY initializer flavor (`let b: bool = self.name ==
        // "omega"` per arm): the scalar value write above cannot serve it,
        // and the fall-through silently left the arm local ZII (the multiarm
        // texteq divergence, parked 2026-07-13, served 2026-07-11aa). The
        // DISPATCH route's local-initializer path serves the same shape via
        // the frame-slot text-comparison writer; give the LEAF route the
        // identical strategy, resolved under the OPERATION's key (the
        // initializer's names live in the ARM's machine).
        crate::selection::runtime_dispatch::writes::emit_runtime_frame_slot_text_comparison_write_in_table(
            input,
            expansion.dispatch_index,
            operation.source_key,
            operation.statement_index,
            expressions,
            slot,
            resolved_initializer,
            runtime_value_operands,
            selected_instructions,
        );
    }
}

fn local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> Option<psi_checked_trees::expression::ExpressionHandle> {
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
                kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                    omega_target_operations::RuntimeStorageRegion::Machine,
                    byte_offset,
                    value,
                    byte_size,
                ),
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
                kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                    omega_target_operations::RuntimeStorageRegion::Machine,
                    byte_offset,
                    value,
                    byte_size,
                ),
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
        input,
        expansion.dispatch_index,
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
    target: psi_checked_trees::expression::ExpressionHandle,
    value_expression: psi_checked_trees::expression::ExpressionHandle,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place_in_table(
        &input.layouts,
        input,
        expansion.dispatch_index,
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
