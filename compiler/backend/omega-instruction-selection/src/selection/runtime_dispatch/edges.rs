use super::guards::{
    select_runtime_dispatch_expression_guard,
    select_runtime_dispatch_expression_guard_conjuncts_in_table,
    select_runtime_dispatch_expression_guard_in_table,
};
use super::writes::{
    emit_runtime_frame_slot_slice_descriptor_write_in_table,
    select_runtime_frame_slot_value_write_in_table,
};
use crate::InstructionSelectionInput;
use crate::selection::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
};
use crate::selection::storage_places::{
    resolve_fixed_array_length_in_table, resolve_runtime_storage_place_in_table,
    resolve_runtime_transition_argument_call_result_place,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_checked_trees::statement::TransitionGuard;
use omega_control_flow::StateKey;
use omega_control_flow::StateParameterFlow;
use omega_core::arena::Arena;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_state_guards::{StateGuardOperandStorage, lower_guard_conjunction};

use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_abstract_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
    StateGuardLowering,
};

pub(super) fn select_runtime_dispatch_edge(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
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
        runtime_value_operands,
        selected_instructions,
    );

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            select_runtime_dispatch_argument_materialization(
                input,
                source_key,
                edge.statement_index,
                edge.target_dispatch_index,
                edge.target_arguments,
                aliases,
                alias_expressions,
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
            select_runtime_dispatch_return_value(input, edge, source_key, selected_instructions);
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_key,
                source_statement: edge.statement_index,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
}

fn select_runtime_dispatch_return_value(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(value) = static_terminal_target_value(input, source_key, edge.order) else {
        return;
    };
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteReturnRegisterInteger {
            byte_size: 4,
            value,
        },
        source_key,
        source_statement: edge.statement_index,
    });
}

fn static_terminal_target_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    edge_order: usize,
) -> Option<i64> {
    let state = input.control_flow.state_by_key(source_key)?;
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
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let source_dispatch_index = target_dispatch_index_for_source(input, source_key);

    if !guard_can_emit_directly(edge) {
        let clauses = lower_guard_conjunction(
            input.state_guards,
            input.layouts,
            input.runtime_storage,
            input.entry_key.machine,
            source_key,
            source_key.machine,
            source_dispatch_index,
            edge.order,
        );
        if !clauses.is_empty() {
            for clause in clauses.iter().copied() {
                let kind = if matches!(clause.lowering, StateGuardLowering::CompareRuntimeValue)
                    && clause.has_storage
                    && clause.has_right_storage
                {
                    SelectedInstructionKind::CompareRuntimeStorage {
                        left_region: guard_storage_region(clause.storage),
                        left_offset: clause.byte_offset,
                        right_region: guard_storage_region(clause.right_storage),
                        right_offset: clause.right_byte_offset,
                        byte_size: clause.byte_size,
                        operator: clause.operator,
                    }
                } else {
                    SelectedInstructionKind::EvaluateDispatchGuard {
                        guard_lowering: clause.lowering,
                        operator: clause.operator,
                        storage_region: guard_storage_region(clause.storage),
                        byte_offset: clause.byte_offset,
                        byte_size: clause.byte_size,
                        expected_value: clause.expected_value,
                        has_storage: clause.has_storage,
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

    if !guard_can_emit_directly(edge) {
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

    let guard_instruction = match edge.guard_lowering {
        StateGuardLowering::CompareRuntimeValue
            if edge.guard_has_storage && edge.guard_has_right_storage =>
        {
            SelectedInstructionKind::CompareRuntimeStorage {
                left_region: guard_storage_region(edge.guard_storage),
                left_offset: edge.guard_byte_offset,
                right_region: guard_storage_region(edge.guard_right_storage),
                right_offset: edge.guard_right_byte_offset,
                byte_size: edge.guard_byte_size,
                operator: edge.guard_operator,
            }
        }
        _ => SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: edge.guard_lowering,
            operator: edge.guard_operator,
            storage_region: guard_storage_region(edge.guard_storage),
            byte_offset: edge.guard_byte_offset,
            byte_size: edge.guard_byte_size,
            expected_value: edge.guard_expected_value,
            has_storage: edge.guard_has_storage,
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
        StateGuardLowering::NoOp => true,
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
                && matches!(edge.guard_byte_size, 1 | 4 | 8)
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
                && matches!(edge.guard_byte_size, 1 | 4 | 8)
        }
        StateGuardLowering::NeedsRuntimeExpression => false,
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

fn select_runtime_dispatch_argument_materialization(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    target_dispatch_index: u32,
    arguments: omega_core::arena::HandleSpan<ExpressionHandle>,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
    let Some(target_key) = dispatch_key_for_index(input, target_dispatch_index) else {
        return;
    };
    let Some(target_state) = input.control_flow.state_by_key(target_key) else {
        return;
    };
    let expressions = &input.control_flow.expressions;
    let target_arguments = expressions.expression_handles(arguments);
    let source_dispatch_index = target_dispatch_index_for_source(input, source_key);
    let static_values =
        super::writes::RuntimeStaticValues::with_capacity(input.runtime_storage.frame_slots.len());
    let mut resolved_argument_expressions = ExpressionTable::with_expression_capacity(
        target_arguments
            .len()
            .saturating_add(alias_expressions.expression_count())
            .saturating_add(4),
    );

    for (parameter_index, parameter) in input
        .control_flow
        .state_parameters(target_state)
        .iter()
        .enumerate()
    {
        let Some(argument) = target_arguments.get(parameter_index).copied() else {
            break;
        };
        let Some(slot) = runtime_parameter_slot(input, target_dispatch_index, parameter) else {
            continue;
        };

        resolved_argument_expressions.clear();
        let copied_aliases = RuntimeAliasBuffer::copy_from_bindings(
            alias_expressions,
            aliases,
            &mut resolved_argument_expressions,
        );
        let copied_argument = resolved_argument_expressions.copy_from(expressions, argument);
        let resolved_argument = resolve_runtime_alias_binding_handle(
            copied_argument,
            source_key,
            copied_aliases.bindings(),
            &mut resolved_argument_expressions,
        );
        let argument_source_key = resolved_argument.source_key;
        let argument = resolve_prior_local_initializers_in_table(
            input,
            argument_source_key,
            statement_index,
            &mut resolved_argument_expressions,
            resolved_argument.expression,
        );
        let expressions = &resolved_argument_expressions;

        if emit_runtime_frame_slot_slice_descriptor_write_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            statement_index,
            expressions,
            slot,
            argument,
            selected_instructions,
        ) {
            continue;
        }

        if matches!(expressions.expression(argument), ExpressionNode::Call(_))
            && let Some(place) = resolve_runtime_transition_argument_call_result_place(
                input,
                source_dispatch_index,
                argument_source_key,
                statement_index,
            )
        {
            if place.byte_count != slot.byte_size {
                continue;
            }
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorage {
                    source_region: place.region,
                    source_offset: place.byte_offset,
                    target_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if emit_runtime_fixed_array_slice_argument_materialization(
            input,
            argument_source_key,
            statement_index,
            source_dispatch_index,
            expressions,
            argument,
            slot,
            selected_instructions,
        ) {
            continue;
        }

        if let Some(initial_value) = source_local_initial_value(
            input,
            argument_source_key,
            statement_index,
            expressions,
            argument,
        ) {
            if emit_runtime_fixed_array_slice_argument_materialization(
                input,
                argument_source_key,
                statement_index,
                source_dispatch_index,
                &input.program.expression_table,
                initial_value,
                slot,
                selected_instructions,
            ) {
                continue;
            }
        }

        if let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            expressions,
            argument,
        ) {
            if place.byte_count != slot.byte_size {
                continue;
            }
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::CopyRuntimeStorage {
                    source_region: place.region,
                    source_offset: place.byte_offset,
                    target_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if let Some(initial_value) = source_local_initial_value(
            input,
            argument_source_key,
            statement_index,
            expressions,
            argument,
        ) && let Some(kind) = select_runtime_frame_slot_value_write_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            statement_index,
            &input.program.expression_table,
            slot,
            initial_value,
            &static_values,
            runtime_value_operands,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if let Some(value) = static_runtime_argument_value(expressions.expression(argument)) {
            if !matches!(slot.byte_size, 1 | 4 | 8) {
                continue;
            }

            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                    target_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    byte_offset: slot.byte_offset,
                    byte_size: slot.byte_size,
                    value,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if let Some(kind) = select_runtime_frame_slot_value_write_in_table(
            input,
            source_dispatch_index,
            argument_source_key,
            statement_index,
            expressions,
            slot,
            argument,
            &static_values,
            runtime_value_operands,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: statement_index,
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_runtime_fixed_array_slice_argument_materialization(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    dispatch_index: u32,
    expressions: &omega_checked_trees::expression::ExpressionTable,
    argument: ExpressionHandle,
    slot: &omega_runtime_storage::RuntimeFrameSlot,
    selected_instructions: &mut SelectedInstructionSink,
) -> bool {
    if slot.byte_size != input.runtime_abi.slice_descriptor_size() {
        return false;
    }

    let ExpressionNode::Call(call) = expressions.expression(argument) else {
        return false;
    };
    if !call.receiver.is_valid()
        || !call.arguments.is_empty()
        || (call.target.as_str() != "as_slice" && call.target.as_str() != "as_mut_slice")
    {
        return false;
    }

    let Some(source_place) = resolve_runtime_storage_place_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };
    let Some(length) = resolve_fixed_array_length_in_table(
        input,
        dispatch_index,
        source_key,
        expressions,
        call.receiver,
    ) else {
        return false;
    };

    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageAddressToRuntimeFrame {
            source_region: source_place.region,
            source_offset: source_place.byte_offset,
            target_offset: slot.byte_offset,
        },
        source_key,
        source_statement: statement_index,
    });
    selected_instructions.push(SelectedInstruction {
        kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
            target_region: RuntimeStorageRegion::RuntimeFrame,
            byte_offset: slot.byte_offset + input.runtime_abi.pointer_size,
            byte_size: input.runtime_abi.pointer_size,
            value: length as i64,
        },
        source_key,
        source_statement: statement_index,
    });
    true
}

fn source_local_initial_value(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &omega_checked_trees::expression::ExpressionTable,
    argument: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let (local_symbol, local_name) = local_root_identity(expressions, argument)?;
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

    let result = input
        .program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .take(statement_index)
        .find_map(|statement| match statement {
            StatementNode::LocalData(local_data)
                if local_data.initial_value.is_valid()
                    && ((local_symbol.is_valid() && local_data.symbol == local_symbol)
                        || local_data.name == local_name) =>
            {
                Some(local_data.initial_value)
            }
            _ => None,
        });

    result
}

fn resolve_prior_local_initializers_in_table(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    expressions: &mut ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    match expressions.expression(expression).clone() {
        ExpressionNode::Name(_) => {
            let Some(initial_value) = source_local_initial_value(
                input,
                source_key,
                statement_index,
                expressions,
                expression,
            ) else {
                return expression;
            };
            let copied = expressions.copy_from(&input.program.expression_table, initial_value);
            resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                copied,
            )
        }
        ExpressionNode::Indexed(indexed) => {
            let collection = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                indexed.collection,
            );
            let index = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                indexed.index,
            );
            if collection == indexed.collection && index == indexed.index {
                expression
            } else {
                expressions.insert(ExpressionNode::Indexed(
                    omega_checked_trees::expression::TableIndexedExpression { collection, index },
                ))
            }
        }
        ExpressionNode::Range(range) => {
            let start = if range.start.is_valid() {
                resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    range.start,
                )
            } else {
                range.start
            };
            let end = if range.end.is_valid() {
                resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    range.end,
                )
            } else {
                range.end
            };
            if start == range.start && end == range.end {
                expression
            } else {
                expressions.insert(ExpressionNode::Range(
                    omega_checked_trees::expression::TableRangeExpression { start, end },
                ))
            }
        }
        ExpressionNode::Call(call) => {
            let receiver = if call.receiver.is_valid() {
                resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    call.receiver,
                )
            } else {
                call.receiver
            };
            let arguments = expressions.reserve_expression_handles(call.arguments.count());
            let mut changed = receiver != call.receiver;
            for offset in 0..call.arguments.count() {
                let argument = expressions.expression_handle_at_offset(call.arguments, offset);
                let resolved_argument = resolve_prior_local_initializers_in_table(
                    input,
                    source_key,
                    statement_index,
                    expressions,
                    argument,
                );
                changed |= resolved_argument != argument;
                expressions.set_expression_handle_at_offset(arguments, offset, resolved_argument);
            }
            if !changed {
                expression
            } else {
                expressions.insert(ExpressionNode::Call(
                    omega_checked_trees::expression::TableCallExpression {
                        receiver,
                        target_symbol: call.target_symbol,
                        target: call.target,
                        arguments,
                    },
                ))
            }
        }
        ExpressionNode::Member(member) => {
            let receiver = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                member.receiver,
            );
            if receiver == member.receiver {
                expression
            } else {
                expressions.insert(ExpressionNode::Member(
                    omega_checked_trees::expression::TableMemberExpression {
                        receiver,
                        member_symbol: member.member_symbol,
                        member: member.member,
                    },
                ))
            }
        }
        ExpressionNode::Mutable(inner) => {
            let resolved = resolve_prior_local_initializers_in_table(
                input,
                source_key,
                statement_index,
                expressions,
                inner,
            );
            if resolved == inner {
                expression
            } else {
                expressions.insert(ExpressionNode::Mutable(resolved))
            }
        }
        _ => expression,
    }
}

fn local_root_identity(
    expressions: &omega_checked_trees::expression::ExpressionTable,
    expression: ExpressionHandle,
) -> Option<(
    omega_core::symbols::SymbolHandle,
    omega_checked_trees::name::Identifier,
)> {
    match expressions.expression(expression) {
        ExpressionNode::Name(path) => Some((
            path.head_symbol,
            expressions.name_path_members(path.members).first()?.clone(),
        )),
        ExpressionNode::Member(member) => local_root_identity(expressions, member.receiver),
        ExpressionNode::Indexed(indexed) => local_root_identity(expressions, indexed.collection),
        ExpressionNode::Mutable(inner) => local_root_identity(expressions, *inner),
        _ => None,
    }
}

fn static_runtime_argument_value(expression: &ExpressionNode) -> Option<i64> {
    match expression {
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Boolean(value) => Some(i64::from(*value)),
        _ => None,
    }
}

fn dispatch_key_for_index(
    input: &InstructionSelectionInput<'_>,
    dispatch_index: u32,
) -> Option<StateKey> {
    input
        .runtime_dispatch_loop
        .cases
        .iter()
        .find_map(|(_, case)| (case.dispatch_index == dispatch_index).then_some(case.key))
}

fn target_dispatch_index_for_source(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
) -> u32 {
    input
        .runtime_dispatch_loop
        .cases
        .iter()
        .find_map(|(_, case)| (case.key == source_key).then_some(case.dispatch_index))
        .unwrap_or_default()
}

fn runtime_parameter_slot<'a>(
    input: &'a InstructionSelectionInput<'a>,
    target_dispatch_index: u32,
    parameter: &StateParameterFlow,
) -> Option<&'a omega_runtime_storage::RuntimeFrameSlot> {
    input
        .runtime_storage
        .frame_slots
        .iter()
        .find_map(|(_, slot)| {
            (slot.dispatch_index == target_dispatch_index && slot.symbol == parameter.symbol)
                .then_some(slot)
        })
}
