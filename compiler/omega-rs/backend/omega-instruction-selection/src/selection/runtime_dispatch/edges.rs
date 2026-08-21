use super::argument_materialization::{
    select_runtime_dispatch_argument_materialization, static_runtime_argument_value,
};
use super::guards::{
    select_runtime_dispatch_expression_guard,
    select_runtime_dispatch_expression_guard_conjuncts_in_table,
    select_runtime_dispatch_expression_guard_in_table,
};
use crate::InstructionSelectionInput;
use crate::selection::bindings::RuntimeAliasBinding;
use crate::selection::storage_places::{
    resolve_runtime_bit_field_place_in_table, resolve_runtime_frame_indexed_target_in_table,
    resolve_runtime_pointee_slot_offset_in_table, resolve_runtime_storage_is_signed_in_table,
    resolve_runtime_storage_place_in_table, resolve_runtime_storage_primitive_type_in_table,
    resolve_runtime_stored_integer_projection_in_table, static_integer_value,
};
use omega_control_flow::StateKey;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_state_graph::CallResultReturn;
use omega_state_guards::{StateGuardOperandStorage, lower_guard_conjunction};
use omega_state_values::simplify_state_expression;
use psi_arena::Arena;
use psi_checked_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, ExpressionTable,
};
use psi_checked_trees::statement::TransitionGuard;
use psi_checked_trees::types::PrimitiveType;

use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
    StateGuardLowering, StateGuardOperator,
};

/// Whether `edge`'s guard is a single ordered comparison (`<`, `<=`, `>`, `>=`)
/// whose operands are an unsigned integer type. Such a guard must branch with
/// unsigned conditions; the clause operator is swapped accordingly. And-
/// conjunctions and signed/undeterminable operands keep the signed form.
fn guard_comparison_operands_unsigned(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    edge: &RuntimeDispatchLoopEdge,
) -> bool {
    if !edge.guard_has_expression {
        return false;
    }
    let expressions = &input.state_guards.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(edge.guard_expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
    ) {
        return false;
    }
    let signed = resolve_runtime_storage_is_signed_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    )
    .or_else(|| {
        resolve_runtime_storage_is_signed_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        )
    });
    signed == Some(false)
}

/// Whether a conjunction contains a comparison operand reached through a
/// pointer-bearing reference slot. The flat state-guard clause model records
/// such a bare local as RuntimeFrame storage (the pointer bytes); expression
/// selection can preserve the required Pointee dereference.
fn guard_contains_pointee_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return guard_contains_pointee_operand(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.left,
        ) || guard_contains_pointee_operand(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        );
    }
    resolve_runtime_pointee_slot_offset_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    )
    .is_some()
        || resolve_runtime_pointee_slot_offset_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        )
        .is_some()
}

/// Whether a conjunction contains a plan-laid compact bit-field operand. The
/// flat state-guard clause model records the containing storage byte, not the
/// logical field value; expression selection preserves the fragment geometry
/// and assembles that value before comparing it.
fn guard_contains_bit_field_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return guard_contains_bit_field_operand(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.left,
        ) || guard_contains_bit_field_operand(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        );
    }
    resolve_runtime_bit_field_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    )
    .is_some()
        || resolve_runtime_bit_field_place_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        )
        .is_some()
}

/// Whether a conjunction contains a plan-laid `IntegerAt` operand. The flat
/// guard model sees the semantic carrier width, while the physical field may
/// be narrower and require sign/zero extension before comparison.
fn guard_contains_stored_integer_operand(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    guard: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = expressions.expression(guard) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return guard_contains_stored_integer_operand(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.left,
        ) || guard_contains_stored_integer_operand(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        );
    }
    resolve_runtime_stored_integer_projection_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    )
    .is_some()
        || resolve_runtime_stored_integer_projection_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        )
        .is_some()
}

/// True when a guard comparison's operands are f64, so the static/runtime
/// compare must use `comisd` rather than an integer `cmp`. First cut: f64 only
/// (matches the arithmetic path). The operand type is read from whichever side
/// resolves to a storage place (the literal side does not).
fn guard_comparison_operands_float(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
    source_key: StateKey,
    edge: &RuntimeDispatchLoopEdge,
) -> bool {
    if !edge.guard_has_expression {
        return false;
    }
    let expressions = &input.state_guards.expressions;
    let ExpressionNode::Binary(binary) = expressions.expression(edge.guard_expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal
            | BinaryOperator::NotEqual
            | BinaryOperator::Greater
            | BinaryOperator::GreaterOrEqual
            | BinaryOperator::Less
            | BinaryOperator::LessOrEqual
    ) {
        return false;
    }
    let primitive = resolve_runtime_storage_primitive_type_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        binary.left,
    )
    .or_else(|| {
        resolve_runtime_storage_primitive_type_in_table(
            input,
            dispatch_index,
            source_key,
            expressions,
            binary.right,
        )
    });
    matches!(primitive, Some(PrimitiveType::F64 | PrimitiveType::F32))
}

fn unsigned_comparison_operator(operator: StateGuardOperator) -> StateGuardOperator {
    match operator {
        StateGuardOperator::Greater => StateGuardOperator::GreaterUnsigned,
        StateGuardOperator::GreaterOrEqual => StateGuardOperator::GreaterOrEqualUnsigned,
        StateGuardOperator::Less => StateGuardOperator::LessUnsigned,
        StateGuardOperator::LessOrEqual => StateGuardOperator::LessOrEqualUnsigned,
        other => other,
    }
}

pub(super) fn select_runtime_dispatch_edge(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    if matches!(edge.action, RuntimeDispatchLoopAction::Unknown) {
        return;
    }

    select_dispatch_guard_instructions(
        input,
        edge,
        source_key,
        source_dispatch_index,
        runtime_value_operands,
        selected_instructions,
    );

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            select_runtime_dispatch_argument_materialization(
                input,
                source_key,
                source_dispatch_index,
                edge.statement_index,
                edge.target_dispatch_index,
                edge.target_arguments,
                aliases,
                alias_expressions,
                runtime_value_operands,
                selected_instructions,
            );

            select_runtime_dispatch_call_result_return(
                input,
                edge,
                source_key,
                source_dispatch_index,
                runtime_value_operands,
                selected_instructions,
            );

            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::SetDispatchState {
                    dispatch_index: edge.target_dispatch_index,
                },
                source_key,
                source_statement: edge.statement_index,
            });
        }
        RuntimeDispatchLoopAction::Terminate => {
            let wrote_return_value = select_runtime_dispatch_return_value(
                input,
                edge,
                source_key,
                source_dispatch_index,
                runtime_value_operands,
                selected_instructions,
            );
            // NATURAL TERMINATION exits 0 (the interpreter -- the oracle --
            // already does; native returned whatever the last computation
            // left in the return register, probed 1-vs-0 2026-07-11y). A
            // terminate edge with no terminal value zeroes it; value
            // terminals and exit_process paths are untouched.
            if !wrote_return_value {
                selected_instructions.push(SelectedInstruction {
                    kind: SelectedInstructionKind::WriteReturnRegisterInteger {
                        register: super::normalized_entry_integer_result_register(input),
                        byte_size: 4,
                        value: 0,
                    },
                    source_key,
                    source_statement: edge.statement_index,
                });
            }
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_key,
                source_statement: edge.statement_index,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
}

/// Deliver a terminating state's terminal value through the entry ABI.
///
/// Ten shapes lower, in order:
/// 1. an INTEGER CONSTANT terminal (`70`, `state shutdown { 0 }`) writes the immediate
///    into the return register (the original literal-only path);
/// 2. a FLOAT CONSTANT terminal stages its raw bits through exact-width result
///    scratch and loads the plan-selected vector result register;
/// 3. a RUNTIME PLACE terminal (`self.count` read-back, a local with a frame
///    slot) loads the place into the return register at runtime — locals that
///    are reassigned always have storage, so the stale-initializer fold below
///    can never apply to them. Native records follow all normalized result
///    fragments, including hidden destinations;
/// 4. a RECORD/CASE LITERAL materializes into layout-sized result scratch,
///    then follows the same direct fragments or hidden destination;
/// 5. a runtime INDEXED scalar copies into result scratch before its native
///    result-register load;
/// 6. a STORAGE-LESS local / constant arithmetic terminal (`let x = 1 + 69; x`)
///    substitutes simple local initializers and constant-folds; such locals
///    were culled from storage precisely because nothing mutates them, so the
///    initializer IS the terminal value;
/// 7. a runtime numeric CAST terminal converts into result scratch using the
///    declared entry result type before the normalized scalar-register load;
/// 8. a runtime logical-NOT terminal compares its byte-sized operand with zero
///    into result scratch, then loads the normalized integer result register;
/// 9. a runtime scalar BINARY terminal recursively builds nested value operands
///    and computes through the ordinary domain-aware writer into result scratch,
///    then loads the normalized integer or vector result register;
/// 10. scalar `min`, `max`, and `sqrt` BUILTIN terminals follow that same
///     pre-resolved-place writer and normalized register load.
fn select_runtime_dispatch_return_value(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if let Some(value) = static_terminal_target_value(input, source_key, edge.order) {
        let (register, byte_size) = super::normalized_entry_integer_result_placement(input)
            .unwrap_or_else(|| (super::normalized_entry_integer_result_register(input), 4));
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteReturnRegisterInteger {
                register,
                byte_size,
                value,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }

    let Some(value_expr) = terminal_target_value_expression(input, source_key, edge.order) else {
        return false;
    };

    if let ExpressionNode::Float(literal) = input.control_flow.expressions.expression(value_expr)
        && let Some((scratch_offset, scratch_size, register)) =
            normalized_entry_scalar_result_scratch(input)
    {
        let value = if scratch_size == 4 {
            i64::from(literal.f32_bits())
        } else {
            literal.value().to_bits() as i64
        };
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                RuntimeStorageRegion::RuntimeFrame,
                scratch_offset,
                value,
                scratch_size,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register,
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: scratch_offset,
                byte_size: scratch_size,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }

    if let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        source_dispatch_index,
        source_key,
        &input.control_flow.expressions,
        value_expr,
    ) && select_entry_runtime_place_result(
        input,
        edge,
        source_key,
        &place,
        selected_instructions,
    ) {
        return true;
    }

    let result_scratch = crate::selection::storage_places::RuntimeStoragePlace {
        region: RuntimeStorageRegion::RuntimeFrame,
        byte_offset: input.runtime_storage.entry_result_scratch_base,
        byte_count: input.runtime_storage.entry_result_scratch_size,
    };
    if result_scratch.byte_count > 0
        && super::normalized_entry_record_result_placement(input).is_some()
        && select_dispatch_case_literal_terminal_return(
            input,
            edge,
            source_key,
            source_dispatch_index,
            result_scratch.region,
            result_scratch.byte_offset,
            result_scratch.byte_count,
            value_expr,
            selected_instructions,
        )
        && select_entry_runtime_place_result(
            input,
            edge,
            source_key,
            &result_scratch,
            selected_instructions,
        )
    {
        return true;
    }

    if let Some((scratch_offset, scratch_size, register)) =
        normalized_entry_scalar_result_scratch(input)
        && let Some(indexed) = resolve_runtime_frame_indexed_target_in_table(
            input,
            source_dispatch_index,
            source_key,
            &input.control_flow.expressions,
            value_expr,
        )
        && indexed.byte_count == scratch_size
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::copy_places_from_indexed(
                indexed.descriptor_offset,
                indexed.index_region,
                indexed.index_offset,
                indexed.index_byte_size,
                indexed.element_byte_size,
                indexed.field_byte_offset,
                RuntimeStorageRegion::RuntimeFrame,
                scratch_offset,
                scratch_size,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register,
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: scratch_offset,
                byte_size: scratch_size,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }

    if let Some(value) =
        simplified_static_terminal_value(input, source_key, edge.statement_index, value_expr)
    {
        let (register, byte_size) = super::normalized_entry_integer_result_placement(input)
            .unwrap_or_else(|| (super::normalized_entry_integer_result_register(input), 4));
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteReturnRegisterInteger {
                register,
                byte_size,
                value,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }

    if let Some((scratch_offset, scratch_size, register)) =
        normalized_entry_scalar_result_scratch(input)
        && select_dispatch_cast_terminal_return(
            input,
            edge,
            source_key,
            source_dispatch_index,
            RuntimeStorageRegion::RuntimeFrame,
            scratch_offset,
            scratch_size,
            value_expr,
            runtime_value_operands,
            selected_instructions,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register,
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: scratch_offset,
                byte_size: scratch_size,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }

    if let Some((scratch_offset, scratch_size, register)) =
        normalized_entry_scalar_result_scratch(input)
        && select_dispatch_unary_terminal_return(
            input,
            edge,
            source_key,
            source_dispatch_index,
            RuntimeStorageRegion::RuntimeFrame,
            scratch_offset,
            scratch_size,
            value_expr,
            runtime_value_operands,
            selected_instructions,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register,
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: scratch_offset,
                byte_size: scratch_size,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }

    if let Some((scratch_offset, scratch_size, register)) =
        normalized_entry_scalar_result_scratch(input)
        && select_dispatch_scalar_operation_terminal_return(
            input,
            edge,
            source_key,
            source_dispatch_index,
            RuntimeStorageRegion::RuntimeFrame,
            scratch_offset,
            scratch_size,
            value_expr,
            runtime_value_operands,
            selected_instructions,
        )
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register,
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: scratch_offset,
                byte_size: scratch_size,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }
    false
}

fn normalized_entry_scalar_result_scratch(
    input: &InstructionSelectionInput<'_>,
) -> Option<(usize, usize, omega_calling_conventions::MachineRegister)> {
    let byte_size = input.runtime_storage.entry_result_scratch_size;
    if !matches!(byte_size, 1 | 2 | 4 | 8)
        || super::normalized_entry_record_result_placement(input).is_some()
    {
        return None;
    }
    let register = super::normalized_entry_scalar_result_register(input, byte_size)?;
    Some((
        input.runtime_storage.entry_result_scratch_base,
        byte_size,
        register,
    ))
}

fn select_entry_runtime_place_result(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    place: &crate::selection::storage_places::RuntimeStoragePlace,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if matches!(place.byte_count, 1 | 2 | 4 | 8)
        && super::normalized_entry_record_result_placement(input).is_none()
    {
        let register = super::normalized_entry_scalar_result_register(input, place.byte_count)
            .unwrap_or_else(|| super::normalized_entry_integer_result_register(input));
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register,
                region: place.region,
                byte_offset: place.byte_offset,
                byte_size: place.byte_count,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }
    let Some((result_shape, locations)) = super::normalized_entry_record_result_placement(input)
    else {
        return false;
    };
    if place.byte_count != usize::from(result_shape.byte_size) {
        return false;
    }
    if locations.iter().all(|location| {
        matches!(
            location,
            omega_calling_conventions::ValueLocation::Register { .. }
        )
    }) {
        for location in locations {
            let omega_calling_conventions::ValueLocation::Register {
                register,
                value_byte_offset,
                byte_size,
            } = location
            else {
                unreachable!("result locations were checked as registers")
            };
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                    register,
                    region: place.region,
                    byte_offset: place.byte_offset + usize::from(value_byte_offset),
                    byte_size: usize::from(byte_size),
                },
                source_key,
                source_statement: edge.statement_index,
            });
        }
        return true;
    }
    if matches!(
        locations.as_slice(),
        [omega_calling_conventions::ValueLocation::Indirect { .. }]
    ) && input.runtime_storage.entry_indirect_result_pointer_size == 8
    {
        let pointer_offset = input.runtime_storage.entry_indirect_result_pointer_base;
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CopyPlaces {
                source: omega_abstract_operations::Place::at(place.region, place.byte_offset),
                target: super::pointee_place(pointer_offset, 0),
                byte_count: place.byte_count,
                role: omega_abstract_operations::CopyPlacesRole::ExitIndirectResult,
            },
            source_key,
            source_statement: edge.statement_index,
        });
        if matches!(
            omega_calling_conventions::CallingPolicy::native_for_target(input.target),
            omega_calling_conventions::CallingPolicy::MicrosoftX64
                | omega_calling_conventions::CallingPolicy::SystemVAMD64
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                    register: omega_calling_conventions::MachineRegister::X86Rax,
                    region: RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: pointer_offset,
                    byte_size: 8,
                },
                source_key,
                source_statement: edge.statement_index,
            });
        }
        return true;
    }
    false
}

/// Constant-fold a terminal value through SIMPLE local initializers
/// (`let exit_code: i32 = 70; exit_code`, `let x: i32 = 1 + 69; x`). Locals
/// that anything reassigns always receive a frame slot (see
/// `local_data_requires_storage`), so they resolve as runtime places before
/// this fold is consulted and the initializer substitution stays sound.
fn simplified_static_terminal_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    value_expr: ExpressionHandle,
) -> Option<i64> {
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
    // Substitution and folding are separate passes inside the simplifier (a
    // Name's binding value comes back UNFOLDED), so iterate to a small
    // fixpoint: pass 1 turns `x` into `1 + 69`, pass 2 folds it to `70`.
    // The bound covers chains of locals (`let a = ...; let b = a + 1; b`).
    let mut expression = input.control_flow.expressions.to_tree(value_expr);
    for _ in 0..4 {
        let simplified =
            simplify_state_expression(input.program, machine, state, statement_index, &expression);
        if let Some(value) = static_integer_value(input.layouts, &simplified) {
            return Some(value);
        }
        if simplified == expression {
            return None;
        }
        expression = simplified;
    }
    None
}

/// When this EnterState edge is a value-returning callee clone's TERMINAL (it
/// carries a `call_result`), write the terminal's value back to the caller's
/// call-result slot before leaving the callee. The callee runs in its own context
/// (`source_dispatch_index`) but shares the frame, so the slot offset resolved
/// across dispatch indices is valid here. Handles a static literal terminal
/// (`-> 99`) and a runtime place terminal (`-> acc`, resolved in the callee).
fn runtime_dispatch_call_result_slot<'a>(
    input: &'a InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    call_result: CallResultReturn,
) -> Option<&'a omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .state_call_result_slot_for_dispatch_by_ordinal(
            edge.target_dispatch_index,
            call_result.call_source_key,
            call_result.statement_index,
            call_result.call_ordinal,
        )
        .or_else(|| {
            input
                .runtime_storage
                .state_call_result_slot_any_role_by_ordinal(
                    call_result.call_source_key,
                    call_result.statement_index,
                    call_result.call_ordinal,
                )
        })
        .or_else(|| {
            // A bare `let value = self.call()` may use the local's ordinary
            // LocalStorage slot directly instead of allocating a tagged
            // StateCallResult slot. That legacy fallback is safe only when
            // this statement contains one value call: with siblings, the
            // ordinal-specific scratch slots above must remain distinct.
            let mut value_calls = input
                .state_calls
                .calls_for_statement(call_result.call_source_key, call_result.statement_index)
                .filter(|call| call.role != omega_state_calls::StateCallRole::Statement);
            let only_call = value_calls.next()?;
            (only_call.call_ordinal == call_result.call_ordinal && value_calls.next().is_none())
                .then(|| {
                    input.runtime_storage.state_call_result_slot_any_role(
                        call_result.call_source_key,
                        call_result.statement_index,
                    )
                })
                .flatten()
        })
}

fn select_runtime_dispatch_call_result_return(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(call_result) = edge.call_result else {
        return;
    };
    // The caller's result place: a frame RESULT SLOT for a `let`-bound call, or
    // -- when no slot exists because the caller assigned the call to a FIELD
    // (`self.total = self.count(..)`; fields live in the machine region, so no
    // frame slot is ever allocated) -- the assignment TARGET's machine-region
    // place, read from the caller statement's Assignment refs. Without the
    // fallback the write was silently SKIPPED and the field stayed ZII
    // (probed native 71 vs interp 70). Restricted to Machine-region targets:
    // a frame-place target under the callee's dispatch context would resolve
    // against the wrong frame, and the frame case is the slot path's job.
    if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() {
        let found = runtime_dispatch_call_result_slot(input, edge, call_result);
        eprintln!(
            "call-result WRITE (return-target dispatch {}): caller m{} s{} stmt {} -> slot {:?}",
            edge.target_dispatch_index,
            call_result.call_source_key.machine.arena_index(),
            call_result.call_source_key.state.arena_index(),
            call_result.statement_index,
            found.map(|slot| (
                slot.dispatch_index,
                slot.byte_offset,
                slot.byte_size,
                format!("{:?}", slot.kind)
            )),
        );
    }
    let (target_region, target_offset, byte_size) =
        if let Some(slot) = runtime_dispatch_call_result_slot(input, edge, call_result) {
            (
                RuntimeStorageRegion::RuntimeFrame,
                slot.byte_offset,
                slot.byte_size,
            )
        } else if let Some(place) = assignment_target_machine_place(
            input,
            edge.target_dispatch_index,
            call_result.call_source_key,
            call_result.statement_index,
        ) {
            (place.region, place.byte_offset, place.byte_count)
        } else {
            return;
        };
    if byte_size == 0 {
        return;
    }

    let value_expr_probe = terminal_target_value_expression(input, source_key, edge.order);
    if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() {
        eprintln!(
            "call-result VALUE: src m{} s{} order {} -> expr {:?}",
            source_key.machine.arena_index(),
            source_key.state.arena_index(),
            edge.order,
            value_expr_probe.is_some(),
        );
    }
    let Some(value_expr) = terminal_target_value_expression(input, source_key, edge.order) else {
        return;
    };

    // Static literal terminal (`-> 99`): write the constant directly.
    if let Some(value) =
        static_runtime_argument_value(input.control_flow.expressions.expression(value_expr))
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                target_region,
                target_offset,
                value,
                byte_size,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
        return;
    }

    // FLOAT-literal terminal (`-> 1.5`): the constant is its IEEE bit pattern
    // (the callee's return type fixed the slot as a float), narrowed to f32
    // bits for a 4-byte result slot -- the integer write stores raw bytes, so
    // the pattern lands verbatim.
    if let ExpressionNode::Float(literal) = input.control_flow.expressions.expression(value_expr) {
        let value = if byte_size == 4 {
            i64::from(literal.f32_bits())
        } else {
            literal.value().to_bits() as i64
        };
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                target_region,
                target_offset,
                value,
                byte_size,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
        return;
    }

    // Runtime place terminal (`-> acc`, `-> self.base`): resolve the value's
    // place in the CALLEE context and copy it into the caller's call-result
    // slot. The source REGION comes from the resolved place -- a param/local
    // terminal lives in the frame, but a FIELD-read terminal (`-> self.base`)
    // lives in the MACHINE region; the old hardcoded RuntimeFrame read the
    // frame at a machine offset and returned garbage (silent-wrong, probed
    // native 71 vs interp 70).
    if let Some(place) = resolve_runtime_storage_place_in_table(
        input,
        source_dispatch_index,
        source_key,
        &input.control_flow.expressions,
        value_expr,
    ) {
        if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() {
            eprintln!(
                "call-result COPY: src {:?}+{} -> {:?}+{} ({} bytes) dispatch {}",
                place.region,
                place.byte_offset,
                target_region,
                target_offset,
                byte_size,
                source_dispatch_index,
            );
        }
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::copy_places_direct(
                place.region,
                place.byte_offset,
                target_region,
                target_offset,
                byte_size,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
        return;
    }

    // Scalar operation terminal (`-> acc + 100`, `-> min(a, b)`): compute into
    // the caller's result place through the same pre-resolved-place writer used
    // by frame-slot values. This preserves recursive operands, float class,
    // signedness, domains, comparisons, and supported scalar builtins. Before
    // this path a binary terminal fell through SILENTLY and the caller's slot
    // kept its prior/ZII value (probed native 71 vs interp 70).
    if select_dispatch_scalar_operation_terminal_return(
        input,
        edge,
        source_key,
        source_dispatch_index,
        target_region,
        target_offset,
        byte_size,
        value_expr,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    // Scalar cast terminal (`-> value as i32`): reuse the checked conversion
    // writer used by entry returns, but target this call site's exact result
    // place. Recursive slice folds commonly return a domain-removing cast of
    // their accumulator; without this path the caller slot stayed ZII.
    if select_dispatch_cast_terminal_return(
        input,
        edge,
        source_key,
        source_dispatch_index,
        target_region,
        target_offset,
        byte_size,
        value_expr,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    // Logical-NOT terminal (`-> !self.flag`): ordinary value calls need the
    // same runtime unary materialization as process-entry returns. Before this
    // arm the callee's result slot retained ZII and the caller observed false.
    if select_dispatch_unary_terminal_return(
        input,
        edge,
        source_key,
        source_dispatch_index,
        target_region,
        target_offset,
        byte_size,
        value_expr,
        runtime_value_operands,
        selected_instructions,
    ) {
        return;
    }

    // SLICE-ELEMENT terminal (`-> s[j]`): the callee's frame holds the slice
    // DESCRIPTOR and the index; copy the indexed element into the caller's
    // result place. The kind is picked BY TARGET REGION, exactly as the
    // mutation path pairs them (writes/storage_copy.rs): a frame result slot
    // rides CopyRuntimeFrameIndexedToRuntimeFrame -- the ToRuntimeStorage
    // kind's encoder is machine-region only, and emitting IT against a frame
    // slot is what CRASHED the first probe (2026-07-09l, reverted; served
    // via this region split 2026-07-09k2).
    if let Some(indexed) = resolve_runtime_frame_indexed_target_in_table(
        input,
        source_dispatch_index,
        source_key,
        &input.control_flow.expressions,
        value_expr,
    ) {
        if indexed.byte_count == byte_size {
            if std::env::var_os("OMEGA_DEBUG_CALL_RESULT").is_some() {
                eprintln!(
                    "call-result INDEXED COPY: desc+{} idx+{} elem {}B field+{} -> {:?}+{} ({} bytes)",
                    indexed.descriptor_offset,
                    indexed.index_offset,
                    indexed.element_byte_size,
                    indexed.field_byte_offset,
                    target_region,
                    target_offset,
                    byte_size,
                );
            }
            // The retired ToFrame/ToStorage split collapses: the target
            // region rides the place (rung 2c-v).
            let kind = crate::selection::runtime_dispatch::copy_places_from_indexed(
                indexed.descriptor_offset,
                indexed.index_region,
                indexed.index_offset,
                indexed.index_byte_size,
                indexed.element_byte_size,
                indexed.field_byte_offset,
                target_region,
                target_offset,
                byte_size,
            );
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: edge.statement_index,
            });
            return;
        }
    }

    // CASE/STRUCT-LITERAL terminal (`-> IoResult::Ok { count: n }`,
    // `-> UnitResult::Ok`): zero the whole result slot (construction
    // zero-initializes unnamed fields -- ZII), write the case TAG (enums), and
    // write each named payload field at its ABSOLUTE variant offset. Field
    // values serve static integers and resolvable places; anything else
    // leaves the terminal unserved and the call-result blocker refuses
    // loudly. This is the shape every wrapper result enum returns through.
    let _ = select_dispatch_case_literal_terminal_return(
        input,
        edge,
        source_key,
        source_dispatch_index,
        target_region,
        target_offset,
        byte_size,
        value_expr,
        selected_instructions,
    );
}

enum FieldWrite {
    Integer(usize, usize, i64),
    Copy(usize, RuntimeStorageRegion, usize, usize),
}

/// Zero a result slot in 8-byte steps with a sized remainder (ZII: a case/
/// struct construction zero-initializes everything it does not name).
fn zero_slot(
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    source_key: StateKey,
    statement_index: usize,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let mut zeroed = 0usize;
    while zeroed < byte_size {
        let step = match byte_size - zeroed {
            remaining if remaining >= 8 => 8,
            remaining if remaining >= 4 => 4,
            remaining if remaining >= 2 => 2,
            _ => 1,
        };
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                target_region,
                target_offset + zeroed,
                0,
                step,
            ),
            source_key,
            source_statement: statement_index,
        });
        zeroed += step;
    }
}

/// Resolve a (possibly case-)literal's named fields into slot-relative writes,
/// all-or-nothing: static integers and bools become Integer writes, resolvable
/// places (scalars AND whole fixed arrays/records) become Copies, and NESTED
/// struct literals recurse with the field's offset accumulated against the
/// nested type's Record layout. Returns false when any field cannot be served
/// (the caller then leaves the terminal unserved -> the blocker refuses).
#[allow(clippy::too_many_arguments)]
fn resolve_literal_field_writes(
    input: &InstructionSelectionInput<'_>,
    source_dispatch_index: u32,
    source_key: StateKey,
    expressions: &ExpressionTable,
    fields_span: psi_arena::HandleSpan<psi_checked_trees::expression::TableStructLiteralField>,
    layout_fields: psi_arena::HandleSpan<omega_layout::FieldLayout>,
    base_offset: usize,
    writes: &mut Vec<FieldWrite>,
) -> bool {
    for offset in 0..fields_span.count() {
        let field = expressions
            .struct_field_at_offset(fields_span, offset)
            .clone();
        let Some(layout_field) = input
            .layouts
            .fields
            .span_or_empty(layout_fields)
            .iter()
            .find(|layout_field| layout_field.name == field.name)
        else {
            return false;
        };
        let field_offset = base_offset + layout_field.offset;
        match expressions.expression(field.value) {
            ExpressionNode::Boolean(value) => {
                writes.push(FieldWrite::Integer(
                    field_offset,
                    layout_field.layout.size,
                    i64::from(*value),
                ));
                continue;
            }
            ExpressionNode::StructLiteral(nested) => {
                // The nested type's own Record layout supplies child offsets.
                let Some(nested_layout) = input
                    .layouts
                    .data_layouts
                    .iter()
                    .find(|(_, data_layout)| data_layout.name == nested.type_name)
                    .map(|(_, data_layout)| data_layout)
                else {
                    return false;
                };
                let omega_layout::DataShape::Record { fields } = &nested_layout.shape else {
                    return false;
                };
                if !resolve_literal_field_writes(
                    input,
                    source_dispatch_index,
                    source_key,
                    expressions,
                    nested.fields,
                    *fields,
                    field_offset,
                    writes,
                ) {
                    return false;
                }
                continue;
            }
            _ => {}
        }
        if let Some(value) = static_runtime_argument_value(expressions.expression(field.value)) {
            writes.push(FieldWrite::Integer(
                field_offset,
                layout_field.layout.size,
                value,
            ));
            continue;
        }
        let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            source_dispatch_index,
            source_key,
            expressions,
            field.value,
        ) else {
            return false;
        };
        if place.byte_count != layout_field.layout.size {
            return false;
        }
        writes.push(FieldWrite::Copy(
            field_offset,
            place.region,
            place.byte_offset,
            place.byte_count,
        ));
    }
    true
}

/// CASE/STRUCT-LITERAL terminal return: zero the caller's result slot, write
/// the case TAG (enum literals), then each named payload field at its
/// ABSOLUTE variant offset. Field values serve static integers and
/// resolvable places (a param/local/field payload -- the wrapper's
/// `IoResult::Ok {{ count: n }}` shape); an unresolvable field leaves the
/// terminal unserved so the call-result blocker refuses loudly.
#[allow(clippy::too_many_arguments)]
fn select_dispatch_case_literal_terminal_return(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value_expr: ExpressionHandle,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let expressions = &input.control_flow.expressions;
    // A BARE nullary case (`-> EmptyResult::Empty`): zero + tag, no fields.
    if let Some(tag) = crate::selection::storage_places::enum_variant_value_in_table(
        &input.layouts,
        expressions,
        value_expr,
    ) {
        zero_slot(
            target_region,
            target_offset,
            byte_size,
            source_key,
            edge.statement_index,
            selected_instructions,
        );
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                target_region,
                target_offset,
                tag,
                omega_layout::ENUM_TAG_BYTES,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }
    let ExpressionNode::StructLiteral(literal) = expressions.expression(value_expr) else {
        return false;
    };
    let type_name = literal.type_name.clone();
    let case_name = literal.case_name.clone();
    let fields_span = literal.fields;

    // The variant's field layouts (absolute offsets within the enum value).
    // A plain struct literal (no case) uses the data layout's own fields.
    let data_layout = input
        .layouts
        .data_layouts
        .iter()
        .find(|(_, data_layout)| data_layout.name == type_name)
        .map(|(_, data_layout)| data_layout);
    let Some(data_layout) = data_layout else {
        return false;
    };
    let (tag, payload_fields) = match (&data_layout.shape, &case_name) {
        (omega_layout::DataShape::Enum { variants, .. }, Some(case_name)) => {
            let Some(variant) = input
                .layouts
                .variants
                .span_or_empty(*variants)
                .iter()
                .find(|variant| variant.name == *case_name)
            else {
                return false;
            };
            let Some(tag) = input
                .layouts
                .variants
                .span_or_empty(*variants)
                .iter()
                .position(|variant| variant.name == *case_name)
                .and_then(|index| i64::try_from(index).ok())
            else {
                return false;
            };
            (Some(tag), variant.fields)
        }
        (omega_layout::DataShape::Record { fields }, None) => (None, *fields),
        _ => return false,
    };

    let mut writes: Vec<FieldWrite> = Vec::new();
    if !resolve_literal_field_writes(
        input,
        source_dispatch_index,
        source_key,
        expressions,
        fields_span,
        payload_fields,
        0,
        &mut writes,
    ) {
        return false;
    }

    // ZII: zero the whole slot (construction zero-initializes every field the
    // literal does not name).
    zero_slot(
        target_region,
        target_offset,
        byte_size,
        source_key,
        edge.statement_index,
        selected_instructions,
    );
    if let Some(tag) = tag {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                target_region,
                target_offset,
                tag,
                omega_layout::ENUM_TAG_BYTES,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
    }
    for write in writes {
        match write {
            FieldWrite::Integer(offset, size, value) => {
                selected_instructions.push(SelectedInstruction {
                    kind: crate::selection::runtime_dispatch::write_place_integer_direct(
                        target_region,
                        target_offset + offset,
                        value,
                        size,
                    ),
                    source_key,
                    source_statement: edge.statement_index,
                });
            }
            FieldWrite::Copy(offset, region, source_offset, size) => {
                selected_instructions.push(SelectedInstruction {
                    kind: crate::selection::runtime_dispatch::copy_places_direct(
                        region,
                        source_offset,
                        target_region,
                        target_offset + offset,
                        size,
                    ),
                    source_key,
                    source_statement: edge.statement_index,
                });
            }
        }
    }
    true
}

#[allow(clippy::too_many_arguments)]
fn select_dispatch_cast_terminal_return(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value_expr: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let expressions = &input.control_flow.expressions;
    let ExpressionNode::Cast(cast) = expressions.expression(value_expr) else {
        return false;
    };
    let Some(target_primitive) = input.program.primitive_type_reference(cast.target_type) else {
        return false;
    };
    if target_primitive.scalar_byte_size() != Some(byte_size) {
        return false;
    }
    let source_primitive = resolve_runtime_storage_primitive_type_in_table(
        input,
        source_dispatch_index,
        source_key,
        expressions,
        cast.value,
    );
    if source_primitive == Some(target_primitive)
        && let Some(source) = resolve_runtime_storage_place_in_table(
            input,
            source_dispatch_index,
            source_key,
            expressions,
            cast.value,
        )
        && source.byte_count == byte_size
    {
        selected_instructions.push(SelectedInstruction {
            kind: crate::selection::runtime_dispatch::copy_places_direct(
                source.region,
                source.byte_offset,
                target_region,
                target_offset,
                byte_size,
            ),
            source_key,
            source_statement: edge.statement_index,
        });
        return true;
    }
    let static_values = super::writes::RuntimeStaticValues::new();
    let Some(kind) = super::writes::mutation::build_runtime_convert_write(
        input,
        source_dispatch_index,
        source_key,
        edge.statement_index,
        expressions,
        target_region,
        target_offset,
        None,
        target_primitive,
        cast.value,
        cast.domain,
        &static_values,
        runtime_value_operands,
    ) else {
        return false;
    };
    selected_instructions.push(SelectedInstruction {
        kind,
        source_key,
        source_statement: edge.statement_index,
    });
    true
}

#[allow(clippy::too_many_arguments)]
fn select_dispatch_unary_terminal_return(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value_expr: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let expressions = &input.control_flow.expressions;
    let Some(kind) = super::writes::mutation::select_runtime_logical_not_write_in_table(
        input,
        source_dispatch_index,
        source_key,
        expressions,
        target_region,
        target_offset,
        byte_size,
        value_expr,
        runtime_value_operands,
    ) else {
        return false;
    };
    selected_instructions.push(SelectedInstruction {
        kind,
        source_key,
        source_statement: edge.statement_index,
    });
    true
}

#[allow(clippy::too_many_arguments)]
fn select_dispatch_scalar_operation_terminal_return(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    target_region: RuntimeStorageRegion,
    target_offset: usize,
    byte_size: usize,
    value_expr: ExpressionHandle,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    let expressions = &input.control_flow.expressions;
    let static_values = super::writes::RuntimeStaticValues::new();
    let Some(kind) = super::writes::mutation::select_runtime_storage_binary_write_in_table(
        input,
        source_dispatch_index,
        source_key,
        edge.statement_index,
        expressions,
        target_region,
        target_offset,
        byte_size,
        value_expr,
        &static_values,
        runtime_value_operands,
    ) else {
        return false;
    };
    selected_instructions.push(SelectedInstruction {
        kind,
        source_key,
        source_statement: edge.statement_index,
    });
    true
}

/// The caller's assignment TARGET resolved to a MACHINE-region place, for a
/// field-bound value call (`self.total = self.count(..)`): the caller state's
/// operation at `statement_index` carries `Assignment { target, .. }` in
/// control flow; resolve it and accept only Machine-region places (frame
/// targets belong to the result-slot path -- resolving a caller frame place
/// under the callee's dispatch context would read the wrong frame).
fn assignment_target_machine_place(
    input: &InstructionSelectionInput<'_>,
    caller_dispatch_index: u32,
    call_source_key: StateKey,
    statement_index: usize,
) -> Option<crate::selection::storage_places::RuntimeStoragePlace> {
    let state = input.control_flow.state_by_key(call_source_key)?;
    let target = input
        .control_flow
        .operations
        .span(state.operations)?
        .iter()
        .find(|operation| operation.statement_index == statement_index)
        .and_then(|operation| match operation.expressions {
            omega_control_flow::OperationExpressionRefs::Assignment { target, .. } => Some(target),
            _ => None,
        })?;
    // Resolve under the CALLER's dispatch case (the return edge's target),
    // not a dummy index: the caller may be a non-first instance whose fields
    // sit at a composed receiver base (`self.total` on the SECOND Mid is
    // mid2+8, not the by-type first-Mid pick -- probed native 71 vs interp
    // 70 on the double-nested field-binding shape).
    resolve_runtime_storage_place_in_table(
        input,
        caller_dispatch_index,
        call_source_key,
        &input.control_flow.expressions,
        target,
    )
    .filter(|place| place.region == RuntimeStorageRegion::Machine)
}

/// The terminal target-value expression of the transition at `edge_order` in
/// `source_key` (the `-> <value>` of a value-returning callee), if valid.
fn terminal_target_value_expression(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    edge_order: usize,
) -> Option<ExpressionHandle> {
    // A state SPLIT at dispatched-call boundaries carries its real transition
    // on the TAIL segment, but only the unsplit state (segment 0) exists in
    // the control-flow plan -- normalize, or a value terminal after a call
    // (`let total = self.t2.drain(..); transition { _ -> (total) }`) finds no
    // state and the return-write silently vanishes (then fences loudly).
    let state = input.control_flow.state_by_key(StateKey {
        segment_index: 0,
        ..source_key
    })?;
    let transition = input
        .control_flow
        .transitions
        .span(state.transitions)?
        .get(edge_order)?;
    let value = transition.expressions.target_value;
    value.is_valid().then_some(value)
}

fn static_terminal_target_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    edge_order: usize,
) -> Option<i64> {
    // Segment normalization: see terminal_target_value_expression.
    let state = input.control_flow.state_by_key(StateKey {
        segment_index: 0,
        ..source_key
    })?;
    let transition = input
        .control_flow
        .transitions
        .span(state.transitions)?
        .get(edge_order)?;
    let value = transition.expressions.target_value;
    if !value.is_valid() {
        return None;
    }

    static_runtime_argument_value(input.control_flow.expressions.expression(value))
}

fn select_dispatch_guard_instructions(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    source_dispatch_index: u32,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    // The flattened edge records a pointer-bearing reference local as the
    // pointer slot's bytes, which looks directly encodable but is not the
    // guard's value. Force expression selection so the Place/operand walk
    // keeps the required dereference.
    let has_pointee_operand = edge.guard_has_expression
        && guard_contains_pointee_operand(
            input,
            source_dispatch_index,
            source_key,
            &input.state_guards.expressions,
            edge.guard_expression,
        );
    let has_bit_field_operand = edge.guard_has_expression
        && guard_contains_bit_field_operand(
            input,
            source_dispatch_index,
            source_key,
            &input.state_guards.expressions,
            edge.guard_expression,
        );
    let has_stored_integer_operand = edge.guard_has_expression
        && guard_contains_stored_integer_operand(
            input,
            source_dispatch_index,
            source_key,
            &input.state_guards.expressions,
            edge.guard_expression,
        );
    if !guard_can_emit_directly(edge)
        || has_pointee_operand
        || has_bit_field_operand
        || has_stored_integer_operand
    {
        let clauses = lower_guard_conjunction(
            input.state_guards,
            input.layouts,
            input.runtime_storage,
            input.receiver_bases,
            input.state_contexts,
            input.entry_key.machine,
            source_key,
            source_key.machine,
            source_dispatch_index,
            edge.order,
        );
        if !clauses.is_empty() {
            // A descriptor-sized clause is a `String == String` place compare
            // (scalar clauses are at most 8 bytes; enum compares clamp to the
            // 4-byte tag): the raw place-vs-place compare below would load 16
            // bytes, which no encoder accepts. Re-select the conjunction from
            // its EXPRESSION instead, where operand types are visible and the
            // String clause lowers through the value-position `TextEquals`
            // content compare. Only adopted when EVERY conjunct selects (one
            // guard per clause), so a partial selection can never silently
            // weaken the guard; otherwise fall through to the loud
            // cannot-encode diagnostic.
            let string_descriptor_size = input.runtime_abi.string_descriptor_size();
            // A CompareRuntimeValue clause MISSING a storage side is an
            // operand the place-based clause model could not resolve -- a
            // member read THROUGH a reference (`d.number_of_pages` where `d`
            // is a wide-referee borrow-recast let) needs a pointer
            // dereference no flat clause can express. Re-select from the
            // EXPRESSION, whose value operands lower the read as a Pointee
            // deref; an unrescued clause still dies loudly at emission.
            let has_unresolved_runtime_compare = clauses.iter().any(|clause| {
                matches!(clause.lowering, StateGuardLowering::CompareRuntimeValue)
                    && !(clause.has_storage && clause.has_right_storage)
            });
            if edge.guard_has_expression
                && (has_unresolved_runtime_compare
                    || has_pointee_operand
                    || has_bit_field_operand
                    || has_stored_integer_operand
                    || clauses
                        .iter()
                        .any(|clause| clause.byte_size == string_descriptor_size))
            {
                let guards = select_runtime_dispatch_expression_guard_conjuncts_in_table(
                    input,
                    source_dispatch_index,
                    source_key,
                    edge.statement_index,
                    &input.state_guards.expressions,
                    edge.guard_expression,
                    runtime_value_operands,
                );
                if guards.len() == clauses.iter().count() {
                    for kind in guards {
                        selected_instructions.push(SelectedInstruction {
                            kind,
                            source_key,
                            source_statement: edge.statement_index,
                        });
                    }
                    return;
                }
            }
            let unsigned =
                guard_comparison_operands_unsigned(input, source_dispatch_index, source_key, edge);
            for clause in clauses.iter().copied() {
                // Float conjuncts take the unsigned jcc forms exactly like the
                // single-clause path below: `ucomis*` sets CF/ZF like an
                // unsigned integer `cmp` and zeroes SF/OF, so a SIGNED
                // condition misreads it (`jge` as the Less-failure branch is
                // ALWAYS taken -- `self.f < 3.15` never passed on x86_64; the
                // f32 field guard canary pins this).
                let operator = if clause.is_float || unsigned {
                    unsigned_comparison_operator(clause.operator)
                } else {
                    clause.operator
                };
                let kind = if matches!(clause.lowering, StateGuardLowering::CompareRuntimeValue)
                    && clause.has_storage
                    && clause.has_right_storage
                {
                    crate::selection::runtime_dispatch::compare_places_direct(
                        guard_storage_region(clause.storage),
                        clause.byte_offset,
                        guard_storage_region(clause.right_storage),
                        clause.right_byte_offset,
                        clause.byte_size,
                        operator,
                        // Place-vs-place float conjuncts stay a follow-on; the
                        // clause carries float-kindedness for constant-float
                        // compares.
                        clause.is_float,
                    )
                } else {
                    SelectedInstructionKind::EvaluateDispatchGuard {
                        guard_lowering: clause.lowering,
                        operator,
                        storage_region: guard_storage_region(clause.storage),
                        byte_offset: clause.byte_offset,
                        byte_size: clause.byte_size,
                        expected_value: clause.expected_value,
                        has_storage: clause.has_storage,
                        is_float: clause.is_float,
                    }
                };
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_key,
                    source_statement: edge.statement_index,
                });
            }
            return;
        }
    }

    if !guard_can_emit_directly(edge)
        || has_pointee_operand
        || has_bit_field_operand
        || has_stored_integer_operand
    {
        if edge.guard_has_expression {
            let guards = select_runtime_dispatch_expression_guard_conjuncts_in_table(
                input,
                source_dispatch_index,
                source_key,
                edge.statement_index,
                &input.state_guards.expressions,
                edge.guard_expression,
                runtime_value_operands,
            );
            if !guards.is_empty() {
                for kind in guards {
                    selected_instructions.push(SelectedInstruction {
                        kind,
                        source_key,
                        source_statement: edge.statement_index,
                    });
                }
                return;
            }
        }

        if edge.guard_has_expression
            && let Some(kind) = select_runtime_dispatch_expression_guard_in_table(
                input,
                source_dispatch_index,
                source_key,
                edge.statement_index,
                &input.state_guards.expressions,
                edge.guard_expression,
                runtime_value_operands,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: edge.statement_index,
            });
            return;
        }

        let guard = transition_guard_for_edge(input, edge);
        if let Some(kind) = select_runtime_dispatch_expression_guard(
            input,
            source_dispatch_index,
            source_key,
            edge.statement_index,
            &guard,
            runtime_value_operands,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: edge.statement_index,
            });
            return;
        }
    }

    let is_float = guard_comparison_operands_float(input, source_dispatch_index, source_key, edge);
    // `ucomisd` sets CF/ZF exactly like an unsigned integer `cmp`, so a float
    // ordering comparison must use the unsigned failure-branch conditions
    // (jae/jbe/ja/jb), not the signed ones — note F64::is_signed_integer() is
    // true, so the unsigned-operand check below does NOT cover floats. Equal/
    // NotEqual are unaffected by the unsigned swap (they stay je/jne).
    let operator = if is_float
        || guard_comparison_operands_unsigned(input, source_dispatch_index, source_key, edge)
    {
        unsigned_comparison_operator(edge.guard_operator)
    } else {
        edge.guard_operator
    };
    let guard_instruction = match edge.guard_lowering {
        StateGuardLowering::CompareRuntimeValue
            if edge.guard_has_storage && edge.guard_has_right_storage =>
        {
            crate::selection::runtime_dispatch::compare_places_direct(
                guard_storage_region(edge.guard_storage),
                edge.guard_byte_offset,
                guard_storage_region(edge.guard_right_storage),
                edge.guard_right_byte_offset,
                edge.guard_byte_size,
                operator,
                is_float,
            )
        }
        _ => SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: edge.guard_lowering,
            operator,
            storage_region: guard_storage_region(edge.guard_storage),
            byte_offset: edge.guard_byte_offset,
            byte_size: edge.guard_byte_size,
            expected_value: edge.guard_expected_value,
            has_storage: edge.guard_has_storage,
            is_float,
        },
    };
    selected_instructions.push(SelectedInstruction {
        kind: guard_instruction,
        source_key,
        source_statement: edge.statement_index,
    });
}

fn transition_guard_for_edge(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
) -> TransitionGuard {
    if edge.guard_has_expression {
        TransitionGuard::When(
            input
                .state_guards
                .expressions
                .to_tree(edge.guard_expression),
        )
    } else {
        TransitionGuard::Always
    }
}

fn guard_can_emit_directly(edge: &RuntimeDispatchLoopEdge) -> bool {
    match edge.guard_lowering {
        // ForwardBranchSkip / BranchArmsEnd never appear as a dispatch-edge guard
        // (leaf-arm only); treat them as trivially emittable.
        StateGuardLowering::NoOp
        | StateGuardLowering::ForwardBranchSkip
        | StateGuardLowering::BranchArmsEnd => true,
        StateGuardLowering::CompareStaticValue => {
            edge.guard_has_storage
                && matches!(
                    edge.guard_operator,
                    omega_abstract_operations::StateGuardOperator::Equal
                        | omega_abstract_operations::StateGuardOperator::NotEqual
                        | omega_abstract_operations::StateGuardOperator::Greater
                        | omega_abstract_operations::StateGuardOperator::GreaterOrEqual
                        | omega_abstract_operations::StateGuardOperator::Less
                        | omega_abstract_operations::StateGuardOperator::LessOrEqual
                )
                && matches!(edge.guard_byte_size, 1 | 2 | 4 | 8)
        }
        StateGuardLowering::CompareRuntimeValue => {
            edge.guard_has_storage
                && edge.guard_has_right_storage
                && matches!(
                    edge.guard_operator,
                    omega_abstract_operations::StateGuardOperator::Equal
                        | omega_abstract_operations::StateGuardOperator::NotEqual
                        | omega_abstract_operations::StateGuardOperator::Greater
                        | omega_abstract_operations::StateGuardOperator::GreaterOrEqual
                        | omega_abstract_operations::StateGuardOperator::Less
                        | omega_abstract_operations::StateGuardOperator::LessOrEqual
                )
                && matches!(edge.guard_byte_size, 1 | 2 | 4 | 8)
        }
        // The leaf-arm poison markers never appear on a dispatch edge.
        StateGuardLowering::NeedsRuntimeExpression
        | StateGuardLowering::UnresolvedInlineArmGuard
        | StateGuardLowering::UnloweredTerminalHostCall
        | StateGuardLowering::UnloweredCaseLiteralField => false,
    }
}

fn guard_storage_region(storage: StateGuardOperandStorage) -> RuntimeStorageRegion {
    match storage {
        StateGuardOperandStorage::MachineOwned | StateGuardOperandStorage::Unknown => {
            RuntimeStorageRegion::Machine
        }
        StateGuardOperandStorage::RuntimeFrame => RuntimeStorageRegion::RuntimeFrame,
    }
}
