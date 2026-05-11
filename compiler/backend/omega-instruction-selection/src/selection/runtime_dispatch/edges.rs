use crate::InstructionSelectionInput;
use crate::selection::storage_places::{
    resolve_runtime_storage_place_in_table,
};
use omega_control_flow::StateParameterFlow;
use omega_control_flow::StateKey;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_typed_trees::expression::ExpressionNode;

use crate::selection::instruction_sink::SelectedInstructionSink;
use omega_target_operations::{SelectedInstruction, SelectedInstructionKind};

pub(super) fn select_runtime_dispatch_edge(
    input: &InstructionSelectionInput<'_>,
    edge: &RuntimeDispatchLoopEdge,
    source_key: StateKey,
    selected_instructions: &mut SelectedInstructionSink,
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
        source_key,
        source_statement: edge.order,
    });

    match edge.action {
        RuntimeDispatchLoopAction::EnterState => {
            select_runtime_dispatch_argument_materialization(
                input,
                source_key,
                edge.order,
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

fn select_runtime_dispatch_argument_materialization(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    target_dispatch_index: u32,
    arguments: omega_core::arena::HandleSpan<omega_typed_trees::expression::ExpressionHandle>,
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

    for (parameter_index, parameter) in target_state.parameters.iter().enumerate() {
        let Some(argument) = target_arguments.get(parameter_index).copied() else {
            break;
        };
        let Some(slot) = runtime_parameter_slot(input, target_dispatch_index, parameter) else {
            continue;
        };

        if let Some(place) = resolve_runtime_storage_place_in_table(
            input,
            target_dispatch_index_for_source(input, source_key),
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

fn dispatch_key_for_index(input: &InstructionSelectionInput<'_>, dispatch_index: u32) -> Option<StateKey> {
    input.runtime_dispatch_loop.cases.iter().find_map(|(_, case)| {
        (case.dispatch_index == dispatch_index).then_some(case.key)
    })
}

fn target_dispatch_index_for_source(input: &InstructionSelectionInput<'_>, source_key: StateKey) -> u32 {
    input.runtime_dispatch_loop.cases.iter().find_map(|(_, case)| {
        (case.key == source_key).then_some(case.dispatch_index)
    }).unwrap_or_default()
}

fn runtime_parameter_slot<'a>(
    input: &'a InstructionSelectionInput<'a>,
    target_dispatch_index: u32,
    parameter: &StateParameterFlow,
) -> Option<&'a omega_runtime_storage::RuntimeFrameSlot> {
    input.runtime_storage.frame_slots.iter().find_map(|(_, slot)| {
        (slot.dispatch_index == target_dispatch_index && slot.symbol == parameter.symbol)
            .then_some(slot)
    })
}
