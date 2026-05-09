use crate::state_analysis::StateAnalysisContext;
use omega_control_flow::{ControlFlowPlan, MachineFlow, OperationKind, StateKey};
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::StateCallResolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::state_calls) struct CollectedStateCall {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub receiver: ProgramName,
    pub target_key: StateKey,
    pub raw_arguments: Vec<Expression>,
    pub reachable: bool,
    pub required: bool,
    pub resolution: StateCallResolution,
}

pub(in crate::state_calls) fn collect_machine_state_calls(
    context: &StateAnalysisContext,
    machine: &MachineFlow,
) -> Vec<CollectedStateCall> {
    let mut calls = Vec::new();

    let Some(states) = context.control_flow.states.span(machine.states) else {
        return calls;
    };

    for state in states {
        let Some(operations) = context.control_flow.operations.span(state.operations) else {
            continue;
        };

        for operation in operations {
            let OperationKind::Call {
                receiver_symbol,
                target_symbol,
                receiver,
                target,
                arguments,
            } = &operation.kind
            else {
                continue;
            };

            if context.state_statement_has_host_call_by_key(state.key, operation.statement_index) {
                continue;
            }

            let resolved_target = resolve_state_call_target(
                &context.control_flow,
                machine,
                *receiver_symbol,
                *target_symbol,
                receiver.as_ref(),
                target,
            );

            calls.push(CollectedStateCall {
                source_key: state.key,
                statement_index: operation.statement_index,
                receiver: receiver
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| ProgramName::generated("self")),
                target_key: resolved_target
                    .as_ref()
                    .map(|target| target.key)
                    .unwrap_or_default(),
                raw_arguments: arguments.clone(),
                reachable: context.runtime_state_is_reachable_by_key(state.key),
                required: false,
                resolution: resolved_target
                    .map(|target| target.resolution)
                    .unwrap_or(StateCallResolution::Unresolved),
            });
        }
    }

    calls
}

struct ResolvedStateCall {
    key: StateKey,
    resolution: StateCallResolution,
}

fn resolve_state_call_target(
    control_flow: &ControlFlowPlan,
    machine: &MachineFlow,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<&ProgramName>,
    target_state: &ProgramName,
) -> Option<ResolvedStateCall> {
    if receiver.is_none() || receiver.is_some_and(|receiver| receiver == "self") {
        return resolve_state_key_in_machine(
            control_flow,
            machine.symbol,
            target_symbol,
            target_state,
        )
        .map(|key| ResolvedStateCall {
            key,
            resolution: StateCallResolution::Local,
        });
    }

    if receiver_symbol.is_valid() {
        if let Some(contained) = machine
            .contains
            .iter()
            .find(|contained| contained.symbol == receiver_symbol)
        {
            return resolve_state_key_in_machine(
                control_flow,
                contained.type_symbol,
                target_symbol,
                target_state,
            )
            .map(|key| ResolvedStateCall {
                key,
                resolution: StateCallResolution::ContainedMachine,
            });
        }

        if let Some(target_machine) = control_flow.machine_by_symbol(receiver_symbol) {
            return resolve_state_key_in_machine(
                control_flow,
                target_machine.symbol,
                target_symbol,
                target_state,
            )
            .map(|key| ResolvedStateCall {
                key,
                resolution: StateCallResolution::NamedMachine,
            });
        }

        return None;
    }

    let _ = receiver?;
    let _ = target_state;
    None
}

fn resolve_state_key_in_machine(
    control_flow: &ControlFlowPlan,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    state_name: &ProgramName,
) -> Option<StateKey> {
    if state_symbol.is_valid() {
        control_flow.state_key_by_symbols(machine_symbol, state_symbol)
    } else {
        let _ = (control_flow, machine_symbol, state_name);
        None
    }
}
