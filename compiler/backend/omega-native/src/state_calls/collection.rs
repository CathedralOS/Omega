use crate::control_flow::{ControlFlowPlan, MachineFlow, OperationKind, StateKey};
use crate::state_analysis::StateAnalysisContext;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::StateCallResolution;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::state_calls) struct CollectedStateCall {
    pub source_key: StateKey,
    pub source_machine: ProgramName,
    pub source_state: ProgramName,
    pub statement_index: usize,
    pub receiver: ProgramName,
    pub target_key: StateKey,
    pub target_machine: ProgramName,
    pub target_state: ProgramName,
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
                receiver.as_ref(),
                target,
            );

            calls.push(CollectedStateCall {
                source_key: state.key,
                source_machine: machine.name.clone(),
                source_state: state.name.clone(),
                statement_index: operation.statement_index,
                receiver: receiver
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| ProgramName::generated("self")),
                target_machine: resolved_target
                    .as_ref()
                    .map(|target| target.machine.clone())
                    .unwrap_or_default(),
                target_key: resolved_target
                    .as_ref()
                    .map(|target| target.key)
                    .unwrap_or_default(),
                target_state: target.clone(),
                raw_arguments: arguments.clone(),
                reachable: context.runtime_state_is_reachable(&machine.name, &state.name),
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
    machine: ProgramName,
    resolution: StateCallResolution,
}

fn resolve_state_call_target(
    control_flow: &ControlFlowPlan,
    machine: &MachineFlow,
    receiver: Option<&ProgramName>,
    target_state: &ProgramName,
) -> Option<ResolvedStateCall> {
    let Some(receiver) = receiver else {
        if let Some(key) = control_flow.state_key_by_names(&machine.name, target_state) {
            return Some(ResolvedStateCall {
                key,
                machine: machine.name.clone(),
                resolution: StateCallResolution::Local,
            });
        }

        return None;
    };

    if receiver == "self" {
        let key = control_flow.state_key_by_names(&machine.name, target_state)?;
        return Some(ResolvedStateCall {
            key,
            machine: machine.name.clone(),
            resolution: StateCallResolution::Local,
        });
    }

    if let Some(contained) = machine
        .contains
        .iter()
        .find(|contained| contained.name == *receiver)
    {
        return control_flow
            .state_key_by_names(&contained.type_name, target_state)
            .map(|key| ResolvedStateCall {
                key,
                machine: contained.type_name.clone(),
                resolution: StateCallResolution::ContainedMachine,
            });
    }

    control_flow
        .state_key_by_names(receiver, target_state)
        .map(|key| ResolvedStateCall {
            key,
            machine: receiver.clone(),
            resolution: StateCallResolution::NamedMachine,
        })
}
