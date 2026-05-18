use super::guards::{
    select_runtime_dispatch_expression_guard, select_runtime_dispatch_expression_guard_in_table,
};
use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    resolve_runtime_storage_place_in_table, resolve_runtime_transition_argument_call_result_place,
};
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_checked_trees::statement::TransitionGuard;
use omega_control_flow::StateKey;
use omega_control_flow::StateParameterFlow;
use omega_core::arena::Arena;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_state_guards::{StateGuardOperandStorage, lower_guard_conjunction};

use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_target_operations::{
    RuntimeStorageRegion, RuntimeValueOperand, SelectedInstruction, SelectedInstructionKind,
    StateGuardLowering,
};

pub(super) fn select_runtime_dispatch_edge(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
) {
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
                selected_instructions,
            );

            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::SetDispatchState {
                    dispatch_index: edge.target_dispatch_index,
                },
                source_key,
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Terminate => {
            selected_instructions.push(SelectedInstruction {
                kind: SelectedInstructionKind::TerminateDispatch,
                source_key,
                source_statement: edge.order,
            });
        }
        RuntimeDispatchLoopAction::Unknown => {}
    }
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
                    source_statement: edge.order,
                });
            }
            return;
        }
    }

    if !guard_can_emit_directly(edge) {
        if edge.guard_has_expression
            && let Some(kind) = select_runtime_dispatch_expression_guard_in_table(
                input,
                source_dispatch_index,
                source_key,
                edge.order,
                &input.state_guards.expressions,
                edge.guard_expression,
                runtime_value_operands,
            )
        {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: edge.order,
            });
            return;
        }

        let guard = transition_guard_for_edge(input, edge);
        if let Some(kind) = select_runtime_dispatch_expression_guard(
            input,
            source_dispatch_index,
            source_key,
            edge.order,
            &guard,
            runtime_value_operands,
        ) {
            selected_instructions.push(SelectedInstruction {
                kind,
                source_key,
                source_statement: edge.order,
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
        source_statement: edge.order,
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
                    omega_target_operations::StateGuardOperator::Equal
                        | omega_target_operations::StateGuardOperator::NotEqual
                        | omega_target_operations::StateGuardOperator::Greater
                        | omega_target_operations::StateGuardOperator::GreaterOrEqual
                        | omega_target_operations::StateGuardOperator::Less
                        | omega_target_operations::StateGuardOperator::LessOrEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4 | 8)
        }
        StateGuardLowering::CompareRuntimeValue => {
            edge.guard_has_storage
                && edge.guard_has_right_storage
                && matches!(
                    edge.guard_operator,
                    omega_target_operations::StateGuardOperator::Equal
                        | omega_target_operations::StateGuardOperator::NotEqual
                        | omega_target_operations::StateGuardOperator::Greater
                        | omega_target_operations::StateGuardOperator::GreaterOrEqual
                        | omega_target_operations::StateGuardOperator::Less
                        | omega_target_operations::StateGuardOperator::LessOrEqual
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

        if matches!(expressions.expression(argument), ExpressionNode::Call(_))
            && let Some(place) = resolve_runtime_transition_argument_call_result_place(
                input,
                source_dispatch_index,
                source_key,
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
                    target_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        if let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            source_dispatch_index,
            source_key,
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
                    target_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                    target_offset: slot.byte_offset,
                    byte_count: slot.byte_size,
                },
                source_key,
                source_statement: statement_index,
            });
            continue;
        }

        let Some(value) = static_runtime_argument_value(expressions.expression(argument)) else {
            continue;
        };
        if !matches!(slot.byte_size, 1 | 4 | 8) {
            continue;
        }

        selected_instructions.push(SelectedInstruction {
            kind: SelectedInstructionKind::WriteRuntimeStorageInteger {
                target_region: omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: slot.byte_offset,
                byte_size: slot.byte_size,
                value,
            },
            source_key,
            source_statement: statement_index,
        });
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
