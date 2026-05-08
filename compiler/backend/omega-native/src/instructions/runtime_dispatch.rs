use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::{
    RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind,
};
use crate::runtime_dispatch::loop_plan::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_core::arena::Arena;

mod branches;
mod guards;
mod writes;

use super::bindings::{
    RuntimeAliasBinding, resolve_runtime_alias_expression, set_runtime_alias,
    strip_mutable_expression,
};
use super::host_operations::{
    runtime_machine_string_descriptor_offset, runtime_text_literal_write_for_host_call,
    select_host_call,
};
use super::lookups::{host_call_for_statement, state_call_for_statement};
use super::model::{InstructionOperand, SelectedInstruction, SelectedInstructionKind};
use branches::{
    select_runtime_leaf_branch_expansions_for_operation,
    select_runtime_straight_line_branch_expansions_for_operation,
};
use writes::select_runtime_storage_write_for_operation;

pub(super) fn select_runtime_dispatch_loop_instructions(
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
    operation: &RuntimeDispatchBodyOperation,
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
