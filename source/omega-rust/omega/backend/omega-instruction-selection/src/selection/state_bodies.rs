use crate::InstructionSelectionInput;
use omega_control_flow::{OperationKind, PlannedTransitionTarget, StateKey};
use omega_state_schedule::{ScheduledState, ScheduledStateCollector};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::ExpressionTable;
use psi_checked_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

use super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, RuntimeAliasResolutionContext,
    resolve_runtime_alias_binding_handle, strip_mutable_expression_handle,
};
use super::host_operations::select_host_call;
use super::instruction_sink::SelectedInstructionSink;
use super::lookups::{
    host_call_for_statement, state_call_for_statement, state_mutation_for_statement,
};
use super::runtime_dispatch::{
    BranchPreludeSelectionScratch, RuntimeStorageWriteScratch, StraightLineBranchSelectionScratch,
    emit_runtime_frame_slot_slice_descriptor_write_in_table, runtime_frame_slot_target_expression,
    select_assignment_value_call_result_local_copy, select_runtime_branch_preludes_for_operation,
    select_runtime_frame_slot_value_write_in_table,
    select_runtime_storage_resolved_mutation_write_in_table_with_scratch,
    select_runtime_straight_line_nested_branch_expansions_for_operation,
    select_runtime_unaliased_storage_mutation_write_with_scratch,
};
use omega_abstract_operations::{InstructionOperand, RuntimeValueOperand, SelectedInstruction};
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_runtime_branching::{
    RuntimeStraightLineBranchOperation, RuntimeStraightLineBranchOperationKind,
};
use omega_state_calls::{StateCall, StateCallLowering, StateCallRole};

const INLINE_STATE_BODY_VISIT_COUNT: usize = 16;

pub(super) struct StateBodyVisitStack {
    inline: [Option<StateKey>; INLINE_STATE_BODY_VISIT_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl StateBodyVisitStack {
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self {
            inline: [None; INLINE_STATE_BODY_VISIT_COUNT],
            len: 0,
            overflow: Vec::with_capacity(capacity.saturating_sub(INLINE_STATE_BODY_VISIT_COUNT)),
        }
    }

    fn contains(&self, key: StateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_STATE_BODY_VISIT_COUNT))
            .flatten()
            .any(|candidate| *candidate == key)
            || self.overflow.contains(&key)
    }

    fn push(&mut self, key: StateKey) {
        if self.len < INLINE_STATE_BODY_VISIT_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_STATE_BODY_VISIT_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

pub(super) fn select_state_body_instructions(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
    dispatch_index: Option<u32>,
    aliases: &RuntimeAliasBuffer,
    alias_expressions: &ExpressionTable,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut StateBodyVisitStack,
) {
    if visiting.contains(state_key) {
        return;
    }

    visiting.push(state_key);

    let Some(state) = input.control_flow.state_by_key(state_key) else {
        visiting.pop();
        return;
    };
    let Some(operations) = input.control_flow.operations.span(state.operations) else {
        visiting.pop();
        return;
    };
    let transitions = input
        .control_flow
        .transitions
        .span_or_empty(state.transitions);
    let alias_capacity = aliases.bindings().len();
    let operation_capacity = operations.len();
    let mut expressions = ExpressionTable::with_expression_capacity(
        alias_capacity
            .saturating_add(operation_capacity)
            .saturating_add(2),
    );
    let mut child_alias_expressions = ExpressionTable::with_expression_capacity(
        alias_capacity.saturating_add(operation_capacity),
    );
    let mut mutation_mutable_expressions = ExpressionTable::with_expression_capacity(2);
    let mut mutation_segment_expressions = ExpressionTable::with_expression_capacity(2);
    let mut storage_write_scratch = RuntimeStorageWriteScratch::default();
    let mut static_values = super::runtime_dispatch::RuntimeStaticValues::with_capacity(
        input.runtime_storage.frame_slots.len(),
    );

    for operation in operations {
        let (call_ordinal, call_target, target_entry) =
            if let OperationKind::Call { target_symbol, .. } = operation.kind {
                if let Some(state_call) =
                    state_call_for_statement(input, state.key, operation.statement_index)
                {
                    (
                        Some(Some(state_call.call_ordinal)),
                        Some(target_symbol),
                        Some(state_call.target_key),
                    )
                } else if let Some(host_call) =
                    host_call_for_statement(input, state.key, operation.statement_index)
                {
                    (
                        Some(Some(host_call.call_ordinal)),
                        Some(target_symbol),
                        None,
                    )
                } else {
                    (Some(None), Some(target_symbol), None)
                }
            } else {
                (None, None, None)
            };
        selected_instructions.begin_permission_site(
            state.key,
            operation.statement_index,
            call_ordinal,
            call_target,
        );
        if let Some(target_key) = target_entry {
            selected_instructions.include_state_entry_permission_events(target_key);
        }
        if matches!(operation.kind, OperationKind::LocalData)
            && let Some(dispatch_index) =
                dispatch_index_for_state(input, state.key).or(dispatch_index)
        {
            select_runtime_state_body_local_initializer_write(
                input,
                dispatch_index,
                state.key,
                operation.statement_index,
                aliases,
                alias_expressions,
                &mut expressions,
                &mut mutation_mutable_expressions,
                &mut mutation_segment_expressions,
                &mut static_values,
                operands,
                runtime_value_operands,
                selected_instructions,
            );
            continue;
        }

        // Asm intrinsic statement (`asm { hlt }`): a receiverless call on the
        // unnameable `asm#hlt` target lowers to the raw MachineHalt
        // instruction, not a state transition. (`asm { in/out }` port ops are
        // the next rung.)
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#hlt"
        {
            selected_instructions.push(SelectedInstruction {
                kind: omega_abstract_operations::SelectedInstructionKind::MachineHalt,
                source_key: state.key,
                source_statement: operation.statement_index,
            });
            continue;
        }

        if let Some((register, source_expression)) =
            super::lookups::asm_control_register_write_source(
                input,
                state.key,
                operation.statement_index,
            )
        {
            let storage_dispatch_index =
                storage_dispatch_index_for_state(input, state.key, dispatch_index);
            if let Some(source) = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                input, storage_dispatch_index, state.key, operation.statement_index,
                &input.state_calls.expressions, source_expression, &static_values, runtime_value_operands,
            ) {
                selected_instructions.push(SelectedInstruction {
                    kind: omega_abstract_operations::SelectedInstructionKind::ControlRegisterWrite {
                        register,
                        source,
                    },
                    source_key: state.key,
                    source_statement: operation.statement_index,
                });
            }
            continue;
        }

        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#wrmsr"
        {
            if let Some((index_expr, value_expr)) =
                super::lookups::asm_msr_write_operands(input, state.key, operation.statement_index)
            {
                let storage_dispatch_index =
                    storage_dispatch_index_for_state(input, state.key, dispatch_index);
                let index = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                    input, storage_dispatch_index, state.key, operation.statement_index,
                    &input.state_calls.expressions, index_expr, &static_values, runtime_value_operands,
                );
                let value = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                    input, storage_dispatch_index, state.key, operation.statement_index,
                    &input.state_calls.expressions, value_expr, &static_values, runtime_value_operands,
                );
                if let (Some(index), Some(value)) = (index, value) {
                    selected_instructions.push(SelectedInstruction {
                        kind: omega_abstract_operations::SelectedInstructionKind::MsrWrite {
                            index,
                            value,
                        },
                        source_key: state.key,
                        source_statement: operation.statement_index,
                    });
                }
            }
            continue;
        }

        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && let Some(kind) =
                psi_language_core::inline_assembly::AsmFenceKind::from_intrinsic_name(
                    target.as_str(),
                )
        {
            selected_instructions.push(SelectedInstruction {
                kind: omega_abstract_operations::SelectedInstructionKind::MemoryFence(kind),
                source_key: state.key,
                source_statement: operation.statement_index,
            });
            continue;
        }

        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && let Some(kind) =
                psi_language_core::inline_assembly::AsmInterruptControlKind::from_intrinsic_name(
                    target.as_str(),
                )
        {
            selected_instructions.push(SelectedInstruction {
                kind: omega_abstract_operations::SelectedInstructionKind::InterruptControl(kind),
                source_key: state.key,
                source_statement: operation.statement_index,
            });
            continue;
        }

        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#popfq"
        {
            if let Some(source_expression) = super::lookups::asm_flags_restore_source(
                input,
                state.key,
                operation.statement_index,
            ) {
                let storage_dispatch_index =
                    storage_dispatch_index_for_state(input, state.key, dispatch_index);
                if let Some(source) = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                    input,
                    storage_dispatch_index,
                    state.key,
                    operation.statement_index,
                    &input.state_calls.expressions,
                    source_expression,
                    &static_values,
                    runtime_value_operands,
                ) {
                    selected_instructions.push(SelectedInstruction {
                        kind: omega_abstract_operations::SelectedInstructionKind::FlagsRestore {
                            source,
                        },
                        source_key: state.key,
                        source_statement: operation.statement_index,
                    });
                }
            }
            continue;
        }

        // `asm { out <port>, <value> }` -- a Call to `asm#port_out`: resolve the
        // two operands and emit a raw port write.
        if let OperationKind::Call {
            target,
            has_receiver: false,
            ..
        } = &operation.kind
            && target.as_str() == "asm#port_out"
        {
            if let Some((port_expr, value_expr)) =
                super::lookups::asm_port_write_operands(input, state.key, operation.statement_index)
            {
                let storage_dispatch_index =
                    storage_dispatch_index_for_state(input, state.key, dispatch_index);
                let port = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                    input, storage_dispatch_index, state.key, operation.statement_index,
                    &input.state_calls.expressions, port_expr, &static_values, runtime_value_operands,
                );
                let value = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                    input, storage_dispatch_index, state.key, operation.statement_index,
                    &input.state_calls.expressions, value_expr, &static_values, runtime_value_operands,
                );
                if let (Some(port), Some(value)) = (port, value) {
                    selected_instructions.push(SelectedInstruction {
                        kind: omega_abstract_operations::SelectedInstructionKind::PortWrite {
                            port,
                            value,
                        },
                        source_key: state.key,
                        source_statement: operation.statement_index,
                    });
                }
            }
            continue;
        }

        if matches!(operation.kind, OperationKind::Call { .. })
            && super::wire_encode::select_wire_encode_call(
                input,
                storage_dispatch_index_for_state(input, state.key, dispatch_index),
                state.key,
                operation.statement_index,
                selected_instructions,
            )
        {
            // The synthesized wire call mutates its &mut arguments (written,
            // and the buffer bytes) OUTSIDE the assignment machinery that
            // feeds static_values: drop every recorded constant so later
            // reads come from live storage. Skipping this folded
            // `let m = d.x == 3` after a DECODE to the pre-decode constant
            // (the wire-wide decode-then-let-compare divergence) -- plain
            // machine calls invalidate through their own selection, and
            // these two branches were the only call shapes that did not.
            static_values.clear();
            continue;
        }

        if matches!(operation.kind, OperationKind::Call { .. })
            && super::wire_decode::select_wire_decode_call(
                input,
                storage_dispatch_index_for_state(input, state.key, dispatch_index),
                state.key,
                operation.statement_index,
                selected_instructions,
            )
        {
            // See the wire-encode arm above: decode writes the value's
            // fields, the read cursor, and the verdict through &mut
            // arguments the static-value walk cannot see.
            static_values.clear();
            continue;
        }

        if matches!(operation.kind, OperationKind::Call { .. })
            && let Some(host_call) =
                host_call_for_statement(input, state.key, operation.statement_index)
        {
            select_host_call(
                input,
                host_call,
                dispatch_index,
                Some(RuntimeAliasResolutionContext {
                    aliases: aliases.bindings(),
                    alias_expressions,
                }),
                operands,
                runtime_value_operands,
                selected_instructions,
            );
            continue;
        }

        if matches!(
            operation.kind,
            OperationKind::Assignment
                | OperationKind::ConstantIntegerAssignment
                | OperationKind::StaticAssignment
        ) && let Some(mutation) =
            state_mutation_for_statement(input, state.key, operation.statement_index)
        {
            let storage_dispatch_index =
                storage_dispatch_index_for_state(input, state.key, dispatch_index);

            if let Some((register, dest_expr)) =
                super::lookups::asm_control_register_read_destination(
                    input,
                    state.key,
                    operation.statement_index,
                )
            {
                let dest = crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                    input,
                    storage_dispatch_index,
                    state.key,
                    &input.state_storage.expressions,
                    dest_expr,
                );
                if let Some(dest) = dest {
                    selected_instructions.push(SelectedInstruction {
                        kind: omega_abstract_operations::SelectedInstructionKind::ControlRegisterRead {
                            register,
                            dest_region: dest.region,
                            dest_byte_offset: dest.byte_offset,
                        },
                        source_key: state.key,
                        source_statement: operation.statement_index,
                    });
                    static_values.clear();
                }
                continue;
            }

            if let Some((index_expr, dest_expr)) =
                super::lookups::asm_msr_read_operands(input, state.key, operation.statement_index)
            {
                let index = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                    input, storage_dispatch_index, state.key, operation.statement_index,
                    &input.state_storage.expressions, index_expr, &static_values, runtime_value_operands,
                );
                let dest = crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                    input,
                    storage_dispatch_index,
                    state.key,
                    &input.state_storage.expressions,
                    dest_expr,
                );
                if let (Some(index), Some(dest)) = (index, dest) {
                    selected_instructions.push(SelectedInstruction {
                        kind: omega_abstract_operations::SelectedInstructionKind::MsrRead {
                            index,
                            dest_region: dest.region,
                            dest_byte_offset: dest.byte_offset,
                        },
                        source_key: state.key,
                        source_statement: operation.statement_index,
                    });
                    static_values.clear();
                }
                continue;
            }

            if let Some(dest_expr) = super::lookups::asm_flags_snapshot_destination(
                input,
                state.key,
                operation.statement_index,
            ) {
                let dest = crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                    input,
                    storage_dispatch_index,
                    state.key,
                    &input.state_storage.expressions,
                    dest_expr,
                );
                if let Some(dest) = dest {
                    selected_instructions.push(SelectedInstruction {
                        kind: omega_abstract_operations::SelectedInstructionKind::FlagsSnapshot {
                            dest_region: dest.region,
                            dest_byte_offset: dest.byte_offset,
                        },
                        source_key: state.key,
                        source_statement: operation.statement_index,
                    });
                    // The hardware value is not a compile-time constant. Keep
                    // later reads from reusing any pre-snapshot fold.
                    static_values.clear();
                }
                continue;
            }

            // `asm { in <dest>, <port> }` -- an assignment whose value is the
            // `asm#port_in` call: emit a raw port read into the destination
            // place instead of a mutation write.
            if let Some((port_expr, dest_expr)) =
                super::lookups::asm_port_read_operands(input, state.key, operation.statement_index)
            {
                let port = crate::selection::runtime_dispatch::writes::mutation::resolve_runtime_value_operand_in_table(
                    input, storage_dispatch_index, state.key, operation.statement_index,
                    &input.state_storage.expressions, port_expr, &static_values, runtime_value_operands,
                );
                let dest = crate::selection::storage_places::resolve_runtime_storage_place_in_table(
                    input,
                    storage_dispatch_index,
                    state.key,
                    &input.state_storage.expressions,
                    dest_expr,
                );
                if let (Some(port), Some(dest)) = (port, dest) {
                    selected_instructions.push(SelectedInstruction {
                        kind: omega_abstract_operations::SelectedInstructionKind::PortRead {
                            port,
                            dest_region: dest.region,
                            dest_byte_offset: dest.byte_offset,
                        },
                        source_key: state.key,
                        source_statement: operation.statement_index,
                    });
                }
                continue;
            }

            if aliases.bindings().is_empty()
                && select_runtime_unaliased_storage_mutation_write_with_scratch(
                    input,
                    storage_dispatch_index,
                    state.key,
                    operation.statement_index,
                    mutation.target,
                    mutation.value,
                    &mut static_values,
                    &mut storage_write_scratch,
                    runtime_value_operands,
                    selected_instructions,
                )
            {
                continue;
            }

            expressions.clear();
            let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
                alias_expressions,
                aliases.bindings(),
                &mut expressions,
            );
            let target = expressions.copy_from(&input.state_storage.expressions, mutation.target);
            let value = expressions.copy_from(&input.state_storage.expressions, mutation.value);
            let resolved_target = resolve_runtime_alias_binding_handle(
                target,
                state.key,
                copied_aliases.bindings(),
                &mut expressions,
            );
            let resolved_value = resolve_runtime_alias_binding_handle(
                value,
                state.key,
                copied_aliases.bindings(),
                &mut expressions,
            );
            mutation_segment_expressions.clear();
            if select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
                input,
                storage_dispatch_index,
                state.key,
                resolved_target.source_key,
                resolved_value.source_key,
                operation.statement_index,
                &expressions,
                resolved_target.expression,
                resolved_value.expression,
                copied_aliases.bindings(),
                &mut static_values,
                &mut mutation_mutable_expressions,
                &mut mutation_segment_expressions,
                runtime_value_operands,
                selected_instructions,
            ) {
                continue;
            }
            if select_assignment_value_call_terminal_fallback_write(
                input,
                storage_dispatch_index,
                state.key,
                operation.statement_index,
                resolved_target.source_key,
                resolved_target.expression,
                &mut expressions,
                &mut static_values,
                &mut mutation_mutable_expressions,
                &mut mutation_segment_expressions,
                runtime_value_operands,
                selected_instructions,
            ) {
                continue;
            }
            // Non-table mutation-write fallback removed (Phase 4): proven dead emitter
            // — the storage `_in_table` write + terminal-fallback paths above cover
            // every case that lowers. An unhandled assignment-value-call here emits
            // nothing, exactly as before.
            continue;
        }

        let OperationKind::Call { .. } = &operation.kind else {
            continue;
        };
        let Some(state_call) =
            state_call_for_statement(input, state.key, operation.statement_index)
        else {
            continue;
        };

        if !state_call.target_key.is_valid() {
            continue;
        }

        child_alias_expressions.clear();
        let mut child_aliases = RuntimeAliasBuffer::copy_from_bindings(
            alias_expressions,
            aliases.bindings(),
            &mut child_alias_expressions,
        );
        bind_state_call_aliases(
            input,
            state_call,
            &mut child_aliases,
            &mut child_alias_expressions,
        );
        selected_instructions.end_permission_site();
        select_state_body_instructions(
            input,
            state_call.target_key,
            dispatch_index,
            &child_aliases,
            &child_alias_expressions,
            operands,
            runtime_value_operands,
            selected_instructions,
            visiting,
        );
    }
    selected_instructions.end_permission_site();

    for transition in transitions {
        follow_transition_target(
            input,
            dispatch_index,
            state.key,
            transition.statement_index,
            aliases,
            alias_expressions,
            &transition.target,
            transition.expressions.target_arguments,
            operands,
            runtime_value_operands,
            selected_instructions,
            visiting,
        );
        if transition.continuation != PlannedTransitionTarget::None {
            follow_transition_target(
                input,
                dispatch_index,
                state.key,
                transition.statement_index,
                aliases,
                alias_expressions,
                &transition.continuation,
                transition.expressions.continuation_arguments,
                operands,
                runtime_value_operands,
                selected_instructions,
                visiting,
            );
        }
    }

    visiting.pop();
}

#[allow(clippy::too_many_arguments)]
fn select_assignment_value_call_terminal_fallback_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
    target_source_key: StateKey,
    resolved_target: psi_checked_trees::expression::ExpressionHandle,
    expressions: &mut ExpressionTable,
    static_values: &mut super::runtime_dispatch::RuntimeStaticValues,
    mutable_expressions: &mut ExpressionTable,
    resolved_segment_expressions: &mut ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(state_call) =
        super::lookups::state_assignment_value_call(input, source_key, statement_index)
    else {
        return false;
    };
    let Some(terminal_value) = terminal_state_value_expression(input, state_call.target_key) else {
        return false;
    };
    let terminal_value = expressions.copy_from(&input.program.expression_table, terminal_value);
    select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        source_key,
        target_source_key,
        state_call.target_key,
        statement_index,
        expressions,
        resolved_target,
        terminal_value,
        &[],
        static_values,
        mutable_expressions,
        resolved_segment_expressions,
        runtime_value_operands,
        selected_instructions,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_state_body_local_initializer_write(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    state_key: StateKey,
    statement_index: usize,
    aliases: &RuntimeAliasBuffer,
    alias_expressions: &ExpressionTable,
    expressions: &mut ExpressionTable,
    mutable_expressions: &mut ExpressionTable,
    segment_expressions: &mut ExpressionTable,
    static_values: &mut super::runtime_dispatch::RuntimeStaticValues,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(slot) =
        initializer_result_or_local_slot(input, dispatch_index, state_key, statement_index)
    else {
        return;
    };
    let wrote_assignment_value_call = select_assignment_value_call_for_local_initializer(
        input,
        dispatch_index,
        state_key,
        statement_index,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    if wrote_assignment_value_call
        && matches!(
            slot.kind,
            omega_runtime_storage::RuntimeFrameSlotKind::StateCallResult { .. }
        )
    {
        return;
    }
    expressions.clear();
    let Some(initializer) =
        local_initializer_handle(input, expressions, state_key, statement_index)
    else {
        return;
    };
    let copied_aliases =
        RuntimeAliasBuffer::copy_from_bindings(alias_expressions, aliases.bindings(), expressions);
    let resolved_initializer = resolve_runtime_alias_binding_handle(
        initializer,
        state_key,
        copied_aliases.bindings(),
        expressions,
    );
    if emit_runtime_frame_slot_slice_descriptor_write_in_table(
        input,
        dispatch_index,
        resolved_initializer.source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer.expression,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
    if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
        input,
        dispatch_index,
        resolved_initializer.source_key,
        statement_index,
        expressions,
        slot,
        resolved_initializer.expression,
        static_values,
        runtime_value_operands,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind,
            source_key: state_key,
            source_statement: statement_index,
        });
        return;
    }

    let target = runtime_frame_slot_target_expression(expressions, slot);
    if select_runtime_storage_resolved_mutation_write_in_table_with_scratch(
        input,
        dispatch_index,
        state_key,
        state_key,
        resolved_initializer.source_key,
        statement_index,
        expressions,
        target,
        resolved_initializer.expression,
        &[],
        static_values,
        mutable_expressions,
        segment_expressions,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }
}

fn select_assignment_value_call_for_local_initializer(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    state_key: StateKey,
    statement_index: usize,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let Some(state_call) =
        super::lookups::state_assignment_value_call(input, state_key, statement_index)
    else {
        return false;
    };
    if state_call.lowering != StateCallLowering::InlineBranching {
        return false;
    }

    let operation = RuntimeStraightLineBranchOperation {
        source_key: state_key,
        statement_index,
        kind: RuntimeStraightLineBranchOperationKind::StateCall {
            role: StateCallRole::AssignmentValue,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    };
    let before = selected_instructions.len();
    let body_operation = RuntimeDispatchBodyOperation {
        source_key: state_key,
        statement_index,
        kind: RuntimeDispatchBodyOperationKind::StateCall {
            role: StateCallRole::AssignmentValue,
            call_ordinal: state_call.call_ordinal,
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    };
    let mut prelude_cursor = 0usize;
    let mut prelude_scratch = BranchPreludeSelectionScratch::default();
    select_runtime_branch_preludes_for_operation(
        input,
        dispatch_index,
        &body_operation,
        &mut prelude_cursor,
        &mut prelude_scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );

    let mut scratch = StraightLineBranchSelectionScratch::default();
    select_runtime_straight_line_nested_branch_expansions_for_operation(
        input,
        dispatch_index,
        &operation,
        &mut scratch,
        operands,
        runtime_value_operands,
        selected_instructions,
    );
    if selected_instructions.len() == before {
        return false;
    }

    select_assignment_value_call_result_local_copy(
        input,
        dispatch_index,
        state_key,
        statement_index,
        StateCallRole::AssignmentValue,
        state_call.call_ordinal,
        selected_instructions,
    );
    true
}

fn initializer_result_or_local_slot<'input>(
    input: &'input InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'input omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .assignment_value_result_slot(dispatch_index, source_key, statement_index)
        .or_else(|| local_storage_slot(input, dispatch_index, source_key, statement_index))
}

fn local_storage_slot<'input>(
    input: &'input InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'input omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == dispatch_index
                && state_key_matches_source(slot.source_key, source_key)
                && slot.statement_index == statement_index
                && matches!(
                    slot.kind,
                    omega_runtime_storage::RuntimeFrameSlotKind::LocalStorage
                ))
            .then_some(slot)
        })
}

fn state_key_matches_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

fn local_initializer_handle(
    input: &InstructionSelectionInput<'_>,
    table: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> Option<psi_checked_trees::expression::ExpressionHandle> {
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

fn terminal_state_value_expression(
    input: &InstructionSelectionInput<'_>,
    target_key: StateKey,
) -> Option<psi_checked_trees::expression::ExpressionHandle> {
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
    let statement = input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .last()?;

    match statement {
        StatementNode::Expression(expression) => expression.is_valid().then_some(*expression),
        StatementNode::Transition(transition)
            if !transition.continuation.is_valid()
                && matches!(transition.guard, TransitionGuardNode::Always) =>
        {
            match input
                .program
                .statement_table
                .transition_target(transition.target)
            {
                TransitionTargetNode::Value(expression) => {
                    expression.is_valid().then_some(*expression)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn follow_transition_target(
    input: &InstructionSelectionInput<'_>,
    current_dispatch_index: Option<u32>,
    current_key: StateKey,
    statement_index: usize,
    aliases: &RuntimeAliasBuffer,
    alias_expressions: &ExpressionTable,
    target: &PlannedTransitionTarget,
    target_arguments: HandleSpan<psi_checked_trees::expression::ExpressionHandle>,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut StateBodyVisitStack,
) {
    let PlannedTransitionTarget::State { key, .. } = target else {
        return;
    };

    if !key.is_valid() {
        return;
    }

    selected_instructions.begin_permission_site(
        current_key,
        statement_index,
        Some(None),
        Some(key.state),
    );
    selected_instructions.include_state_entry_permission_events(*key);
    selected_instructions.end_permission_site();

    let (child_aliases, child_alias_expressions) = bind_transition_target_aliases(
        input,
        current_key,
        *key,
        target_arguments,
        aliases,
        alias_expressions,
    );

    select_state_body_instructions(
        input,
        *key,
        dispatch_index_for_state(input, *key).or(current_dispatch_index),
        &child_aliases,
        &child_alias_expressions,
        operands,
        runtime_value_operands,
        selected_instructions,
        visiting,
    );
}

fn bind_transition_target_aliases(
    input: &InstructionSelectionInput<'_>,
    current_key: StateKey,
    target_key: StateKey,
    target_arguments: HandleSpan<psi_checked_trees::expression::ExpressionHandle>,
    aliases: &RuntimeAliasBuffer,
    alias_expressions: &ExpressionTable,
) -> (RuntimeAliasBuffer, ExpressionTable) {
    let target_argument_count = target_arguments.len();
    let mut child_alias_expressions = ExpressionTable::with_expression_capacity(
        aliases
            .bindings()
            .len()
            .saturating_add(target_argument_count)
            .saturating_mul(2),
    );
    let mut child_aliases = RuntimeAliasBuffer::copy_from_bindings(
        alias_expressions,
        aliases.bindings(),
        &mut child_alias_expressions,
    );

    let Some(target_state) = input.control_flow.state_by_key(target_key) else {
        return (child_aliases, child_alias_expressions);
    };
    let arguments = input
        .control_flow
        .expressions
        .expression_handles(target_arguments);

    for (parameter, argument) in input
        .control_flow
        .state_parameters(target_state)
        .iter()
        .zip(arguments.iter().copied())
    {
        let argument = child_alias_expressions.copy_from(&input.control_flow.expressions, argument);
        let resolved_argument = resolve_runtime_alias_binding_handle(
            argument,
            current_key,
            child_aliases.bindings(),
            &mut child_alias_expressions,
        );
        let expression =
            strip_mutable_expression_handle(&child_alias_expressions, resolved_argument.expression);
        child_aliases.set_alias(RuntimeAliasBinding {
            source_key: target_key,
            parameter_symbol: parameter.symbol,
            parameter_name: parameter.name.clone(),
            expression_source_key: resolved_argument.source_key,
            expression,
        });
    }

    (child_aliases, child_alias_expressions)
}

pub(super) fn runtime_reachable_states(
    input: &InstructionSelectionInput<'_>,
) -> Vec<ScheduledState> {
    let mut states = ScheduledStateCollector::new();

    for (_, state) in input.runtime_flow.states.iter() {
        states.push_key(state.key);
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        states.push_key(state_call.source_key);

        if state_call.target_key.is_valid() {
            states.push_key(state_call.target_key);
        }
    }

    states.finish()
}

fn bind_state_call_aliases(
    input: &InstructionSelectionInput<'_>,
    state_call: &StateCall,
    aliases: &mut RuntimeAliasBuffer,
    alias_expressions: &mut ExpressionTable,
) {
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let argument_expression =
            alias_expressions.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_expression = resolve_runtime_alias_binding_handle(
            argument_expression,
            state_call.source_key,
            aliases.bindings(),
            alias_expressions,
        );
        let expression =
            strip_mutable_expression_handle(alias_expressions, resolved_expression.expression);
        aliases.set_alias(RuntimeAliasBinding {
            source_key: state_call.target_key,
            parameter_symbol: argument.parameter_symbol,
            parameter_name: argument.parameter_name.clone(),
            expression_source_key: resolved_expression.source_key,
            expression,
        });
    }
}

fn dispatch_index_for_state(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
) -> Option<u32> {
    input
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| body.key == state_key)
        .map(|(_, body)| body.dispatch_index)
}

fn storage_dispatch_index_for_state(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
    current_dispatch_index: Option<u32>,
) -> u32 {
    dispatch_index_for_state(input, state_key)
        .or(current_dispatch_index)
        .unwrap_or(0)
}
