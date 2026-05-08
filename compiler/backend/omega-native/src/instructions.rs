use crate::control_flow::StateKey;
use crate::data::NativeDataObject;
use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use crate::runtime_dispatch::branching::{
    RuntimeLeafBranchBinding, RuntimeLeafBranchBindingKind, RuntimeLeafBranchExpansion,
    RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchBinding,
    RuntimeStraightLineBranchExpansion, RuntimeStraightLineBranchOperationKind,
};
use crate::runtime_dispatch::loop_plan::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use crate::runtime_text::RuntimeTextBuilderSegmentKind;
use crate::state_guards::StateGuardOperator;
use crate::state_schedule::{build_entry_state_schedule, scheduled_state_flow};
use omega_core::arena::Arena;
use omega_typed_program::expression::Expression;

mod bindings;
mod host_operations;
mod lookups;
mod model;
mod state_bodies;
mod storage_places;

use bindings::{
    RuntimeAliasBinding, append_place_suffix, resolve_leaf_binding_expression,
    resolve_runtime_alias_expression, resolve_straight_line_binding_expression, set_runtime_alias,
    strip_mutable_expression,
};
use host_operations::{
    runtime_machine_string_descriptor_offset, runtime_text_input_buffer_for_text_place,
    runtime_text_literal_write_for_host_call, select_host_call,
};
use lookups::{
    host_call_for_statement, state_call_for_statement, state_mutation_for_statement,
    state_operations, state_parameters,
};
pub use model::{
    FunctionInstructionPlan, InstructionOperand, InstructionOperandKind, InstructionPlan,
    SelectedInstruction, SelectedInstructionKind,
};
use state_bodies::{
    machine_name_for_state, runtime_reachable_states, select_state_body_instructions,
    select_state_host_calls,
};
use storage_places::{
    enum_variant_value, resolve_machine_owned_place, resolve_runtime_storage_place,
    static_integer_value,
};

pub fn build_instruction_plan(native_plan: &NativePlan) -> InstructionPlan {
    let mut instruction_plan = InstructionPlan {
        target: native_plan.target,
        functions: Arena::new(),
        instructions: Arena::new(),
        operands: Arena::new(),
    };

    let entry_instructions = select_entry_instructions(native_plan, &mut instruction_plan.operands);
    let instructions = instruction_plan
        .instructions
        .insert_many(entry_instructions);

    instruction_plan.functions.insert(FunctionInstructionPlan {
        symbol: native_plan.object.entry_symbol.clone(),
        machine: native_plan.entry_machine.clone().into(),
        state: native_plan.entry_state.clone().into(),
        instructions,
    });

    instruction_plan
}

fn select_entry_instructions(
    native_plan: &NativePlan,
    operands: &mut Arena<InstructionOperand>,
) -> Vec<SelectedInstruction> {
    let mut selected_instructions = Vec::new();
    let state_schedule_result = build_entry_state_schedule(native_plan);
    let can_inline_state_calls = state_schedule_result.is_ok()
        && native_plan
            .state_calls
            .calls
            .iter()
            .any(|(_, call)| call.required);
    let state_schedule =
        state_schedule_result.unwrap_or_else(|_| runtime_reachable_states(native_plan));

    selected_instructions.push(entry_instruction(native_plan));

    if native_plan.runtime_dispatch_loop.needed {
        select_runtime_dispatch_loop_instructions(
            native_plan,
            operands,
            &mut selected_instructions,
        );
    } else if can_inline_state_calls {
        select_state_body_instructions(
            native_plan,
            native_plan.entry_key,
            operands,
            &mut selected_instructions,
            &mut Vec::new(),
        );
    } else {
        for scheduled_state in &state_schedule {
            if let Some(state_flow) = scheduled_state_flow(native_plan, scheduled_state) {
                let Some(machine_name) = machine_name_for_state(native_plan, state_flow) else {
                    continue;
                };
                select_state_host_calls(
                    native_plan,
                    machine_name,
                    &state_flow.name,
                    operands,
                    &mut selected_instructions,
                );
            }
        }
    }

    selected_instructions.push(exit_instruction(native_plan));
    selected_instructions
}

fn select_runtime_dispatch_loop_instructions(
    native_plan: &NativePlan,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index: native_plan.runtime_dispatch_loop.entry_dispatch_index,
            terminal_dispatch_index: native_plan.runtime_dispatch_loop.terminal_dispatch_index,
            current_state_slot: native_plan.runtime_dispatch_loop.current_state_slot.clone(),
            next_state_slot: native_plan.runtime_dispatch_loop.next_state_slot.clone(),
        },
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    });

    for (_, dispatch_case) in native_plan.runtime_dispatch_loop.cases.iter() {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::EnterDispatchCase {
                dispatch_index: dispatch_case.dispatch_index,
                label: dispatch_case.label.clone(),
            },
            source_machine: dispatch_case.machine.clone(),
            source_state: dispatch_case.state.clone(),
            source_statement: 0,
        });

        if let Some(runtime_body) = native_plan
            .runtime_bodies
            .bodies
            .iter()
            .find(|(_, body)| body.dispatch_index == dispatch_case.dispatch_index)
            .map(|(_, body)| body)
            && let Some(operations) = native_plan
                .runtime_bodies
                .operations
                .span(runtime_body.operations)
        {
            let mut runtime_aliases = Vec::new();
            let mut runtime_static_values = Vec::new();

            for operation in operations {
                bind_runtime_operation_aliases(native_plan, operation, &mut runtime_aliases);

                select_runtime_storage_write_for_operation(
                    native_plan,
                    dispatch_case.dispatch_index,
                    operation,
                    &runtime_aliases,
                    &mut runtime_static_values,
                    selected_instructions,
                );

                select_runtime_leaf_branch_expansions_for_operation(
                    native_plan,
                    dispatch_case.dispatch_index,
                    operation,
                    selected_instructions,
                );
                select_runtime_straight_line_branch_expansions_for_operation(
                    native_plan,
                    dispatch_case.dispatch_index,
                    operation,
                    selected_instructions,
                );

                if let Some(host_call) = host_call_for_statement(
                    native_plan,
                    operation.source_key,
                    operation.statement_index,
                ) {
                    if runtime_machine_string_descriptor_offset(native_plan, host_call).is_none()
                        && let Some((buffer_symbol, literal)) =
                            runtime_text_literal_write_for_host_call(native_plan, host_call)
                    {
                        selected_instructions.push(SelectedInstruction {
                            kind: SelectedInstructionKind::WriteRuntimeTextLiteral {
                                buffer_symbol,
                                literal,
                            },
                            source_machine: host_call.machine.clone(),
                            source_state: host_call.state.clone(),
                            source_statement: host_call.statement_index,
                        });
                    }
                    select_host_call(native_plan, host_call, operands, selected_instructions);
                }
            }
        }

        if let Some(edges) = native_plan
            .runtime_dispatch_loop
            .edges
            .span(dispatch_case.edges)
        {
            for edge in edges {
                select_runtime_dispatch_edge(
                    edge,
                    &dispatch_case.machine,
                    &dispatch_case.state,
                    selected_instructions,
                );
            }
        }

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::LeaveDispatchCase,
            source_machine: dispatch_case.machine.clone(),
            source_state: dispatch_case.state.clone(),
            source_statement: 0,
        });
    }

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::LeaveDispatchLoop,
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    });
}

fn select_runtime_dispatch_edge(
    edge: &RuntimeDispatchLoopEdge,
    source_machine: &str,
    source_state: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: edge.guard_lowering,
            operator: edge.guard_operator,
            byte_offset: edge.guard_byte_offset,
            byte_size: edge.guard_byte_size,
            expected_value: edge.guard_expected_value,
            has_storage: edge.guard_has_storage,
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: edge.order,
    });

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::SetDispatchState {
                    dispatch_index: edge.target_dispatch_index,
                },
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Terminate => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
}

fn bind_runtime_operation_aliases(
    native_plan: &NativePlan,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    aliases: &mut Vec<RuntimeAliasBinding>,
) {
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
        | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
        RuntimeDispatchBodyOperationKind::HostCall { .. }
        | RuntimeDispatchBodyOperationKind::LocalStorage { .. }
        | RuntimeDispatchBodyOperationKind::Mutation { .. }
        | RuntimeDispatchBodyOperationKind::Other => return,
    }

    let Some(state_call) =
        state_call_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        if argument.kind != crate::state_calls::StateCallArgumentKind::MutableAlias {
            continue;
        }

        let expression = strip_mutable_expression(resolve_runtime_alias_expression(
            &argument.expression,
            state_call.source_key,
            aliases,
        ));
        set_runtime_alias(
            aliases,
            RuntimeAliasBinding {
                source_key: state_call.target_key,
                parameter_name: argument.parameter_name.clone(),
                expression,
            },
        );
    }
}

fn select_runtime_storage_write_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut Vec<(String, i64)>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let RuntimeDispatchBodyOperationKind::Mutation { .. } = &operation.kind else {
        return;
    };
    let Some(mutation) =
        state_mutation_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };

    select_runtime_mutation_writes(
        native_plan,
        dispatch_index,
        mutation.source_key,
        &operation.source_machine,
        &operation.source_state,
        mutation.statement_index,
        &mutation.target,
        &mutation.value,
        aliases,
        static_values,
        selected_instructions,
    );
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_mutation_writes(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    target: &Expression,
    value: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &mut Vec<(String, i64)>,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let resolved_target = resolve_runtime_alias_expression(target, source_key, aliases);

    if let Expression::StructLiteral(struct_literal) = value {
        for field in &struct_literal.fields {
            let field_target =
                append_place_suffix(&resolved_target, std::slice::from_ref(&field.name));
            select_runtime_mutation_writes(
                native_plan,
                dispatch_index,
                source_key,
                source_machine,
                source_state,
                statement_index,
                &field_target,
                &field.value,
                aliases,
                static_values,
                selected_instructions,
            );
        }
        return;
    }

    if let Expression::String(value) = value {
        select_runtime_string_descriptor_write(
            native_plan,
            source_key,
            source_machine,
            source_state,
            statement_index,
            &resolved_target,
            value,
            selected_instructions,
        );
        return;
    }

    if let Some(instructions) = runtime_text_builder_write(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        &resolved_target,
        aliases,
    ) {
        for kind in instructions {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_machine: source_machine.to_owned().into(),
                source_state: source_state.to_owned().into(),
                source_statement: statement_index,
            });
        }
        return;
    }

    let resolved_value = resolve_runtime_alias_expression(value, source_key, aliases);
    if let Some(copy) = runtime_storage_copy(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
        &resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_machine: source_machine.to_owned().into(),
            source_state: source_state.to_owned().into(),
            source_statement: statement_index,
        });
        return;
    }

    let Some(value) = resolve_runtime_static_integer_value(
        native_plan,
        source_key,
        value,
        aliases,
        static_values,
    ) else {
        return;
    };
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        &resolved_target,
    ) else {
        return;
    };

    set_runtime_static_value(
        static_values,
        strip_mutable_expression(resolved_target.clone()).display_name(),
        value,
    );
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
            byte_offset,
            byte_size,
            value,
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: statement_index,
    });
}

fn runtime_text_builder_write(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    aliases: &[RuntimeAliasBinding],
) -> Option<Vec<SelectedInstructionKind>> {
    runtime_text_builder_write_with_resolver(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        statement_index,
        resolved_target,
        &|expression| resolve_runtime_alias_expression(expression, source_key, aliases),
    )
}

fn runtime_text_builder_write_with_resolver(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolve_expression: &dyn Fn(&Expression) -> Expression,
) -> Option<Vec<SelectedInstructionKind>> {
    let builder = native_plan
        .runtime_text
        .builders
        .iter()
        .find(|(_, builder)| {
            builder.source_key == source_key && builder.statement_index == statement_index
        })
        .map(|(_, builder)| builder)?;
    let segments = native_plan
        .runtime_text
        .builder_segments
        .span(builder.segments)?;
    let resolved_target = strip_mutable_expression(resolved_target.clone());
    let buffer = runtime_text_input_buffer_for_text_place(native_plan, &resolved_target)?;
    let target_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        &resolved_target,
    )?;
    if target_place.byte_count != native_plan.target.pointer_size * 2 {
        return None;
    }

    if let [prefix, suffix] = segments
        && prefix.kind == RuntimeTextBuilderSegmentKind::StaticText
        && suffix.kind == RuntimeTextBuilderSegmentKind::StoredPlace
    {
        let Expression::String(prefix) = &prefix.expression else {
            return None;
        };
        let source = resolve_expression(&suffix.expression);
        let source_place = resolve_runtime_storage_place(
            native_plan,
            dispatch_index,
            source_key,
            source_machine,
            source_state,
            &source,
        )?;
        if source_place.byte_count != native_plan.target.pointer_size * 2 {
            return None;
        }
        return Some(vec![
            SelectedInstructionKind::WriteRuntimeTextLiteralSegment {
                buffer_symbol: buffer.symbol.clone(),
                byte_offset: 0,
                literal: prefix.clone(),
            },
            SelectedInstructionKind::AppendRuntimeTextStoredSuffix {
                buffer_symbol: buffer.symbol.clone(),
                buffer_offset: prefix.len(),
                source_symbol: source_place.symbol,
                source_offset: source_place.byte_offset,
                target_symbol: target_place.symbol,
                target_offset: target_place.byte_offset,
                length_delta: prefix.len(),
            },
        ]);
    }

    let mut instructions = Vec::new();
    for segment in segments {
        match segment.kind {
            RuntimeTextBuilderSegmentKind::StoredPlace => {
                let source = resolve_expression(&segment.expression);
                let source_place = resolve_runtime_storage_place(
                    native_plan,
                    dispatch_index,
                    source_key,
                    source_machine,
                    source_state,
                    &source,
                )?;
                if source_place.byte_count != native_plan.target.pointer_size * 2 {
                    return None;
                }
                if source_place.symbol == target_place.symbol
                    && source_place.byte_offset == target_place.byte_offset
                {
                    instructions.push(SelectedInstructionKind::MaterializeRuntimeTextBuffer {
                        buffer_symbol: buffer.symbol.clone(),
                        target_symbol: target_place.symbol.clone(),
                        target_offset: target_place.byte_offset,
                    });
                    continue;
                }
                instructions.push(SelectedInstructionKind::AppendRuntimeTextStoredPlace {
                    buffer_symbol: buffer.symbol.clone(),
                    source_symbol: source_place.symbol,
                    source_offset: source_place.byte_offset,
                    target_symbol: target_place.symbol.clone(),
                    target_offset: target_place.byte_offset,
                });
            }
            RuntimeTextBuilderSegmentKind::StaticText => {
                let Expression::String(literal) = &segment.expression else {
                    return None;
                };
                instructions.push(SelectedInstructionKind::AppendRuntimeTextLiteral {
                    buffer_symbol: buffer.symbol.clone(),
                    target_symbol: target_place.symbol.clone(),
                    target_offset: target_place.byte_offset,
                    literal: literal.clone(),
                });
            }
            RuntimeTextBuilderSegmentKind::OtherExpression => return None,
        }
    }

    (!instructions.is_empty()).then_some(instructions)
}

fn runtime_storage_copy(
    native_plan: &NativePlan,
    dispatch_index: u32,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    let target_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        target,
    )?;
    let source_place = resolve_runtime_storage_place(
        native_plan,
        dispatch_index,
        source_key,
        source_machine,
        source_state,
        value,
    )?;
    if target_place.byte_count != source_place.byte_count || target_place.byte_count == 0 {
        return None;
    }

    Some(SelectedInstructionKind::CopyRuntimeStorage {
        source_symbol: source_place.symbol,
        source_offset: source_place.byte_offset,
        target_symbol: target_place.symbol,
        target_offset: target_place.byte_offset,
        byte_count: target_place.byte_count,
    })
}

fn select_runtime_string_descriptor_write(
    native_plan: &NativePlan,
    source_key: StateKey,
    source_machine: &str,
    source_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    value: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        resolved_target,
    ) else {
        return;
    };
    if byte_size != native_plan.target.pointer_size * 2 {
        return;
    }
    let Some(data_object) =
        string_literal_data_object(native_plan, source_key, statement_index, value)
    else {
        return;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeMachineString {
            byte_offset,
            data_symbol: data_object.symbol.clone(),
            byte_length: value.len(),
        },
        source_machine: source_machine.to_owned().into(),
        source_state: source_state.to_owned().into(),
        source_statement: statement_index,
    });
}

fn string_literal_data_object<'plan>(
    native_plan: &'plan NativePlan,
    source_key: StateKey,
    statement_index: usize,
    value: &str,
) -> Option<&'plan NativeDataObject> {
    native_plan
        .data
        .objects
        .iter()
        .find(|(_, data_object)| {
            data_object.source_key == source_key
                && data_object.source_statement == statement_index
                && native_plan
                    .data
                    .bytes
                    .span(data_object.bytes)
                    .is_some_and(|bytes| {
                        bytes == value.as_bytes() || (value.is_empty() && bytes == [0])
                    })
        })
        .map(|(_, data_object)| data_object)
}

fn resolve_runtime_static_integer_value(
    native_plan: &NativePlan,
    source_key: StateKey,
    expression: &Expression,
    aliases: &[RuntimeAliasBinding],
    static_values: &[(String, i64)],
) -> Option<i64> {
    match expression {
        Expression::Integer(value) => Some(*value),
        Expression::Name(_) => enum_variant_value(&native_plan.layouts, expression).or_else(|| {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression.display_name())
                .map(|(_, value)| *value)
        }),
        Expression::Indexed(_) | Expression::Mutable(_) => {
            let resolved_expression =
                resolve_runtime_alias_expression(expression, source_key, aliases);
            let resolved_expression = strip_mutable_expression(resolved_expression);
            static_values
                .iter()
                .find(|(target, _)| target == &resolved_expression.display_name())
                .map(|(_, value)| *value)
        }
        Expression::Boolean(value) => Some(i64::from(*value)),
        Expression::ArrayLiteral(_)
        | Expression::Binary(_)
        | Expression::Float(_)
        | Expression::String(_)
        | Expression::StructLiteral(_) => None,
    }
}

fn set_runtime_static_value(static_values: &mut Vec<(String, i64)>, target: String, value: i64) {
    if let Some((_, existing_value)) = static_values
        .iter_mut()
        .find(|(existing_target, _)| existing_target == &target)
    {
        *existing_value = value;
    } else {
        static_values.push((target, value));
    }
}

fn select_runtime_leaf_branch_expansions_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .leaf_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_leaf_branch_expansion(native_plan, expansion, selected_instructions);
    }
}

fn select_runtime_leaf_branch_expansion(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let mut mutation_writes = Vec::new();
    select_runtime_leaf_branch_mutation_writes(native_plan, expansion, &mut mutation_writes);
    if mutation_writes.is_empty() {
        return;
    }

    if let Some((buffer_symbol, literal)) = runtime_text_literal_guard(native_plan, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::CompareRuntimeTextLiteral {
                buffer_symbol,
                literal,
            },
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
    } else if let Some(compare) = runtime_text_storage_guard(native_plan, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: compare,
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
    } else if let Some(compare) = runtime_storage_guard(native_plan, expansion) {
        selected_instructions.push(SelectedInstruction {
            kind: compare,
            source_machine: expansion.source_machine.clone(),
            source_state: expansion.source_state.clone(),
            source_statement: expansion.statement_index,
        });
    } else {
        return;
    }
    selected_instructions.extend(mutation_writes);
}

fn select_runtime_straight_line_branch_expansions_for_operation(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation: &crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperation,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    for (_, expansion) in native_plan
        .runtime_branching_calls
        .straight_line_expansions
        .iter()
        .filter(|(_, expansion)| {
            expansion.dispatch_index == dispatch_index
                && expansion.source_key == operation.source_key
                && expansion.statement_index == operation.statement_index
        })
    {
        select_runtime_straight_line_branch_expansion(
            native_plan,
            expansion,
            selected_instructions,
        );
    }
}

fn select_runtime_straight_line_branch_expansion(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if expansion.resolved_guard != omega_typed_program::statement::TransitionGuard::Always {
        return;
    }

    select_runtime_straight_line_branch_writes(native_plan, expansion, selected_instructions);
}

fn select_runtime_leaf_branch_mutation_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(operations) = native_plan
        .runtime_branching_calls
        .leaf_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = native_plan
        .runtime_branching_calls
        .leaf_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        let RuntimeLeafBranchOperationKind::Mutation { target, value, .. } = &operation.kind else {
            continue;
        };
        let resolved_target = resolve_leaf_binding_expression(target, bindings);
        let resolved_value = resolve_leaf_binding_expression(value, bindings);

        if let Some((byte_offset, byte_size, value)) = runtime_leaf_machine_integer_write(
            native_plan,
            expansion,
            &resolved_target,
            &resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                    byte_offset,
                    byte_size,
                    value,
                },
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                source_statement: operation.statement_index,
            });
            continue;
        }

        if let Some(instructions) = runtime_text_builder_write_with_resolver(
            native_plan,
            expansion.dispatch_index,
            operation.source_key,
            &operation.source_machine,
            &operation.source_state,
            operation.statement_index,
            &resolved_target,
            &|expression| resolve_leaf_binding_expression(expression, bindings),
        ) {
            for kind in instructions {
                selected_instructions.push(SelectedInstruction {
                    kind,
                    source_machine: operation.source_machine.clone(),
                    source_state: operation.source_state.clone(),
                    source_statement: operation.statement_index,
                });
            }
            continue;
        }

        if let Some(copy) = runtime_leaf_storage_copy(
            native_plan,
            expansion,
            &operation.source_machine,
            &operation.source_state,
            &resolved_target,
            &resolved_value,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind: copy,
                source_machine: operation.source_machine.clone(),
                source_state: operation.source_state.clone(),
                source_statement: operation.statement_index,
            });
        }
    }
}

fn select_runtime_straight_line_branch_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(operations) = native_plan
        .runtime_branching_calls
        .straight_line_operations
        .span(expansion.operations)
    else {
        return;
    };
    let bindings = native_plan
        .runtime_branching_calls
        .straight_line_bindings
        .span(expansion.bindings)
        .unwrap_or(&[]);

    for operation in operations {
        match &operation.kind {
            RuntimeStraightLineBranchOperationKind::Mutation { target, value, .. } => {
                let resolved_target = resolve_straight_line_binding_expression(target, bindings);
                let resolved_value = resolve_straight_line_binding_expression(value, bindings);
                select_runtime_resolved_mutation_write(
                    native_plan,
                    expansion.dispatch_index,
                    operation.source_key,
                    &expansion.source_machine,
                    &operation.source_machine,
                    &operation.source_state,
                    operation.statement_index,
                    &resolved_target,
                    &resolved_value,
                    selected_instructions,
                );
            }
            RuntimeStraightLineBranchOperationKind::StateCall {
                target_key,
                target_machine,
                target_state,
                lowering: crate::state_calls::StateCallLowering::InlineLeaf,
                ..
            } => select_runtime_straight_line_leaf_state_call_writes(
                native_plan,
                expansion,
                operation,
                bindings,
                *target_key,
                target_machine,
                target_state,
                selected_instructions,
            ),
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_straight_line_leaf_state_call_writes(
    native_plan: &NativePlan,
    expansion: &RuntimeStraightLineBranchExpansion,
    operation: &crate::runtime_dispatch::branching::RuntimeStraightLineBranchOperation,
    straight_line_bindings: &[RuntimeStraightLineBranchBinding],
    target_key: StateKey,
    target_machine: &str,
    target_state: &str,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    let Some(state_call) =
        state_call_for_statement(native_plan, operation.source_key, operation.statement_index)
    else {
        return;
    };
    let Some(arguments) = native_plan.state_calls.arguments.span(state_call.arguments) else {
        return;
    };
    let leaf_parameters = state_parameters(native_plan, target_key);
    let leaf_bindings = leaf_parameters
        .iter()
        .enumerate()
        .filter_map(|(parameter_index, parameter_name)| {
            let argument = arguments.get(parameter_index)?;
            Some(RuntimeLeafBranchBinding {
                parameter_name: parameter_name.clone(),
                expression: resolve_straight_line_binding_expression(
                    &argument.expression,
                    straight_line_bindings,
                ),
                kind: RuntimeLeafBranchBindingKind::LeafParameter,
            })
        })
        .collect::<Vec<_>>();

    let Some(operations) = state_operations(native_plan, target_key) else {
        return;
    };
    for leaf_operation in operations {
        let Some(mutation) =
            state_mutation_for_statement(native_plan, target_key, leaf_operation.statement_index)
        else {
            continue;
        };
        let resolved_target = resolve_leaf_binding_expression(&mutation.target, &leaf_bindings);
        let resolved_value = resolve_leaf_binding_expression(&mutation.value, &leaf_bindings);
        select_runtime_resolved_mutation_write(
            native_plan,
            expansion.dispatch_index,
            target_key,
            &expansion.source_machine,
            target_machine,
            target_state,
            leaf_operation.statement_index,
            &resolved_target,
            &resolved_value,
            selected_instructions,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn select_runtime_resolved_mutation_write(
    native_plan: &NativePlan,
    dispatch_index: u32,
    operation_key: StateKey,
    source_machine: &str,
    operation_machine: &str,
    operation_state: &str,
    statement_index: usize,
    resolved_target: &Expression,
    resolved_value: &Expression,
    selected_instructions: &mut Vec<SelectedInstruction>,
) {
    if let Some((byte_offset, byte_size)) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        source_machine,
        resolved_target,
    ) && let Some(value) = static_integer_value(&native_plan.layouts, resolved_value)
    {
        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeMachineInteger {
                byte_offset,
                byte_size,
                value,
            },
            source_machine: operation_machine.to_owned().into(),
            source_state: operation_state.to_owned().into(),
            source_statement: statement_index,
        });
        return;
    }

    if let Some(copy) = runtime_storage_copy(
        native_plan,
        dispatch_index,
        operation_key,
        operation_machine,
        operation_state,
        resolved_target,
        resolved_value,
    ) {
        selected_instructions.push(SelectedInstruction {
            kind: copy,
            source_machine: operation_machine.to_owned().into(),
            source_state: operation_state.to_owned().into(),
            source_statement: statement_index,
        });
    }
}

fn runtime_text_literal_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<(String, String)> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    if binary.operator != omega_typed_program::expression::BinaryOperator::Equal {
        return None;
    }

    let (text_place, literal) = match (&binary.left, &binary.right) {
        (text_place, Expression::String(literal)) => (text_place, literal),
        (Expression::String(literal), text_place) => (text_place, literal),
        _ => return None,
    };

    let buffer = runtime_text_input_buffer_for_text_place(native_plan, text_place)?;
    Some((buffer.symbol.clone(), literal.clone()))
}

fn runtime_text_storage_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    if binary.operator != omega_typed_program::expression::BinaryOperator::Equal {
        return None;
    }
    let operator = StateGuardOperator::Equal;

    let left_place = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.left,
    );
    let right_place = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.right,
    );
    let left_buffer = runtime_text_input_buffer_for_text_place(native_plan, &binary.left);
    let right_buffer = runtime_text_input_buffer_for_text_place(native_plan, &binary.right);
    let string_descriptor_size = native_plan.target.pointer_size * 2;

    if let (Some(source_place), Some(buffer)) = (left_place.clone(), right_buffer)
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer_symbol: buffer.symbol.clone(),
            source_symbol: source_place.symbol,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    if let (Some(buffer), Some(source_place)) = (left_buffer, right_place)
        && source_place.byte_count == string_descriptor_size
    {
        return Some(SelectedInstructionKind::CompareRuntimeTextStorage {
            buffer_symbol: buffer.symbol.clone(),
            source_symbol: source_place.symbol,
            source_offset: source_place.byte_offset,
            operator,
        });
    }

    None
}

fn runtime_storage_guard(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
) -> Option<SelectedInstructionKind> {
    let omega_typed_program::statement::TransitionGuard::When(Expression::Binary(binary)) =
        &expansion.resolved_guard
    else {
        return None;
    };
    let operator = match binary.operator {
        omega_typed_program::expression::BinaryOperator::Equal => StateGuardOperator::Equal,
        omega_typed_program::expression::BinaryOperator::NotEqual => StateGuardOperator::NotEqual,
        _ => return None,
    };
    let left = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.left,
    );
    let right = resolve_runtime_storage_place(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        &expansion.source_machine,
        &expansion.source_state,
        &binary.right,
    );

    if let (Some(left), Some(right)) = (left.clone(), right.clone()) {
        if left.byte_count != right.byte_count {
            return None;
        }

        return Some(SelectedInstructionKind::CompareRuntimeStorage {
            left_symbol: left.symbol,
            left_offset: left.byte_offset,
            right_symbol: right.symbol,
            right_offset: right.byte_offset,
            byte_size: left.byte_count,
            operator,
        });
    }

    if let Some(place) = left
        && let Some(expected_value) = enum_variant_value(&native_plan.layouts, &binary.right)
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            symbol: place.symbol,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    if let Some(place) = right
        && let Some(expected_value) = enum_variant_value(&native_plan.layouts, &binary.left)
    {
        return Some(SelectedInstructionKind::CompareRuntimeStorageValue {
            symbol: place.symbol,
            byte_offset: place.byte_offset,
            byte_size: place.byte_count,
            expected_value,
            operator,
        });
    }

    None
}

fn runtime_leaf_machine_integer_write(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    target: &Expression,
    value_expression: &Expression,
) -> Option<(usize, usize, i64)> {
    let (byte_offset, byte_size) = resolve_machine_owned_place(
        &native_plan.layouts,
        &native_plan.entry_machine,
        &expansion.source_machine,
        target,
    )?;
    let value = static_integer_value(&native_plan.layouts, value_expression)?;

    Some((byte_offset, byte_size, value))
}

fn runtime_leaf_storage_copy(
    native_plan: &NativePlan,
    expansion: &RuntimeLeafBranchExpansion,
    operation_machine: &str,
    operation_state: &str,
    target: &Expression,
    value: &Expression,
) -> Option<SelectedInstructionKind> {
    runtime_storage_copy(
        native_plan,
        expansion.dispatch_index,
        expansion.source_key,
        operation_machine,
        operation_state,
        target,
        value,
    )
}

fn entry_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::EnterFunction,
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    }
}

fn exit_instruction(native_plan: &NativePlan) -> SelectedInstruction {
    SelectedInstruction {
        kind: SelectedInstructionKind::LeaveFunction,
        source_machine: native_plan.entry_machine.clone().into(),
        source_state: native_plan.entry_state.clone().into(),
        source_statement: 0,
    }
}
