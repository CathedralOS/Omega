//! Forwarded dynamic scalar calls and their exact transfer paths.

use crate::execution::terminal_unit::dynamic_scalar_calls::receivers::CheckedDynamicScalarCall;
use crate::execution::terminal_unit::dynamic_scalar_calls::scalar_call_plans::build_checked_dynamic_scalar_call;
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedUnitCallCoordinate, ShapeCollector,
    StatementNode, TypedTrees, state_flow,
};

pub(crate) fn build_checked_forwarded_dynamic_scalar_calls(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    binding_facts: &checked_trees::DynamicConformanceBindingFacts,
    plans: &mut checked_trees::CheckedDynamicDispatchPlans,
) -> Option<()> {
    for machine in program.machines() {
        if !machine.attached_data_symbol.is_valid() {
            continue;
        }
        for state in program.machine_states(machine) {
            let Some(flow) = state_flow(facts, machine.symbol, state.symbol) else {
                continue;
            };
            for outer_call in facts.flow.control.calls.span_or_empty(flow.calls) {
                if outer_call.has_receiver || outer_call.call_ordinal != 0 {
                    continue;
                }
                let Ok(statement_index) = u32::try_from(outer_call.statement_index) else {
                    continue;
                };
                let Ok(call_ordinal) = u32::try_from(outer_call.call_ordinal) else {
                    continue;
                };
                let coordinate = CheckedUnitCallCoordinate {
                    statement_index,
                    call_ordinal,
                };
                let matching_transfers = plans
                    .transfers
                    .iter()
                    .filter(|transfer| {
                        transfer.caller_machine == machine.symbol
                            && transfer.caller_state == state.symbol
                            && transfer.coordinate == coordinate
                    })
                    .collect::<Vec<_>>();
                let [transfer] = matching_transfers.as_slice() else {
                    continue;
                };
                let transfer = (*transfer).clone();
                if transfer.source
                    != checked_trees::CheckedDynamicDescriptorTransferSource::Selection
                {
                    continue;
                }
                let Some(outer_site) = crate::semantic_calls::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    outer_call.statement_index,
                    outer_call.call_ordinal,
                ) else {
                    continue;
                };
                let crate::semantic_calls::CallSite::Expression { call, .. } = &outer_site else {
                    continue;
                };
                if program
                    .expression_table
                    .expression_handles(call.arguments)
                    .len()
                    != 1
                {
                    continue;
                }

                let Some(forwarded) = resolve_forwarded_dynamic_scalar_call(
                    program,
                    facts,
                    &plans.transfers,
                    transfer,
                ) else {
                    continue;
                };
                let Some(plan) = build_checked_dynamic_scalar_call(
                    program,
                    facts,
                    binding_facts,
                    machine,
                    state,
                    outer_call,
                    outer_site,
                    shapes,
                    boundaries,
                    Some(forwarded),
                    None,
                ) else {
                    continue;
                };
                match plan {
                    CheckedDynamicScalarCall::Direct(plan) => {
                        plans.direct_scalar_calls.push(plan);
                    }
                    CheckedDynamicScalarCall::Rebound(plan) => {
                        plans.rebound_scalar_calls.push(plan);
                    }
                    CheckedDynamicScalarCall::Stored(plan) => {
                        plans.stored_scalar_calls.push(plan);
                    }
                }
            }
        }
    }
    Some(())
}

fn resolve_forwarded_dynamic_scalar_call<'program, 'facts>(
    program: &'program TypedTrees,
    facts: &'facts CheckFacts,
    transfers: &[checked_trees::CheckedDynamicDescriptorTransferPlan],
    root_transfer: checked_trees::CheckedDynamicDescriptorTransferPlan,
) -> Option<ForwardedDynamicCall<'program, 'facts>> {
    let mut current = root_transfer.clone();
    let mut prior_transfers = Vec::new();
    let mut helpers = Vec::new();
    let mut visited = Vec::new();
    loop {
        if visited.iter().any(|&(machine, state)| {
            machine == current.target_machine && state == current.target_state
        }) {
            return None;
        }
        visited.push((current.target_machine, current.target_state));
        let target_state = crate::semantic_calls::find_state(program, current.target_state)?;
        let target_machine = program.machines().iter().find(|candidate| {
            candidate.symbol == current.target_machine
                && program
                    .machine_states(candidate)
                    .iter()
                    .any(|candidate_state| candidate_state.symbol == target_state.symbol)
        })?;
        let [parameter] = program.state_parameters(target_state) else {
            return None;
        };
        if parameter.is_self
            || parameter.is_const
            || !parameter.symbol.is_valid()
            || parameter.symbol != current.parameter
            || current.parameter_position != 0
            || !program.state_contracts(target_state).is_empty()
        {
            return None;
        }
        let helper_flow = state_flow(facts, target_machine.symbol, target_state.symbol)?;
        let [inner_call] = facts.flow.control.calls.span_or_empty(helper_flow.calls) else {
            return None;
        };
        if inner_call.call_ordinal != 0 {
            return None;
        }
        let (scalar_control, prefix_count) =
            crate::execution::terminal_unit::control::scalar_control(
                program,
                facts,
                target_machine,
                target_state,
            )?;
        if inner_call.statement_index >= prefix_count {
            return None;
        }
        let statements = program
            .statement_table
            .statements(target_state.statement_nodes);
        let mut scalar_locals = Vec::new();
        let mut call_result = None;
        for (ordinal, statement) in statements.iter().take(prefix_count).enumerate() {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            if local.is_mutable || !local.symbol.is_valid() {
                return None;
            }
            let coordinate = u32::try_from(ordinal).ok()?;
            if ordinal == inner_call.statement_index {
                let primitive_type = program.primitive_type_reference(local.type_reference)?;
                if primitive_type != scalar_control.primitive_type {
                    return None;
                }
                call_result = Some(checked_trees::CheckedUnitScalarResultBindingPlan {
                    statement_index: coordinate,
                    binding_ordinal: coordinate,
                    primitive_type,
                });
            } else {
                scalar_locals.push(crate::execution::terminal_unit::scalar_expression_local_at(
                    program,
                    facts,
                    target_state,
                    coordinate,
                    coordinate,
                    local,
                )?);
            }
        }
        let StatementNode::LocalData(helper_result) = &statements[inner_call.statement_index]
        else {
            return None;
        };
        helpers.push(checked_trees::CheckedDynamicScalarHelperPlan {
            machine: target_machine.symbol,
            state: target_state.symbol,
            call_result: call_result?,
            scalar_locals,
            scalar_control,
        });
        let inner_site = crate::semantic_calls::find_call_site(
            program,
            target_machine.symbol,
            target_state.symbol,
            inner_call.statement_index,
            inner_call.call_ordinal,
        )?;
        let crate::semantic_calls::CallSite::Expression {
            expression,
            call: inner_expression_call,
        } = &inner_site
        else {
            return None;
        };
        if helper_result.initial_value != *expression
            || program
                .expression_table
                .expression_handles(inner_expression_call.arguments)
                .len()
                != usize::from(!inner_call.has_receiver)
        {
            return None;
        }
        if inner_call.has_receiver {
            if inner_call.receiver_symbol != parameter.symbol {
                return None;
            }
            return Some(ForwardedDynamicCall {
                machine: target_machine,
                state: target_state,
                flow_call: inner_call,
                call_site: inner_site,
                transfer: root_transfer,
                prior_transfers,
                helpers,
            });
        }
        let coordinate = CheckedUnitCallCoordinate {
            statement_index: u32::try_from(inner_call.statement_index).ok()?,
            call_ordinal: u32::try_from(inner_call.call_ordinal).ok()?,
        };
        let matching = transfers
            .iter()
            .filter(|transfer| {
                transfer.caller_machine == target_machine.symbol
                    && transfer.caller_state == target_state.symbol
                    && transfer.coordinate == coordinate
                    && transfer.source_binding == parameter.symbol
                    && transfer.source
                        == checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                            parameter_position: 0,
                        }
            })
            .collect::<Vec<_>>();
        let [next] = matching.as_slice() else {
            return None;
        };
        current = (*next).clone();
        prior_transfers.push(current.clone());
    }
}

pub(crate) struct ForwardedDynamicCall<'program, 'facts> {
    pub(crate) helpers: Vec<checked_trees::CheckedDynamicScalarHelperPlan>,
    pub(crate) machine: &'program typed_trees::machine::Machine,
    pub(crate) state: &'program typed_trees::state::State,
    pub(crate) flow_call: &'facts checked_trees::FlowCallFact,
    pub(crate) call_site: crate::semantic_calls::CallSite<'program>,
    pub(crate) transfer: checked_trees::CheckedDynamicDescriptorTransferPlan,
    pub(crate) prior_transfers: Vec<checked_trees::CheckedDynamicDescriptorTransferPlan>,
}

pub(crate) fn forwarded_transfer_path_is_exact(forwarded: &ForwardedDynamicCall<'_, '_>) -> bool {
    if forwarded.transfer.source != checked_trees::CheckedDynamicDescriptorTransferSource::Selection
    {
        return false;
    }
    let [root_path] = forwarded.transfer.source_paths.as_slice() else {
        return false;
    };
    let mut expected_path = root_path.clone();
    let mut machine = forwarded.transfer.target_machine;
    let mut state = forwarded.transfer.target_state;
    for transfer in &forwarded.prior_transfers {
        if transfer.caller_machine != machine
            || transfer.caller_state != state
            || transfer.source
                != (checked_trees::CheckedDynamicDescriptorTransferSource::Parameter {
                    parameter_position: 0,
                })
        {
            return false;
        }
        expected_path.edges.push(transfer.edge());
        if !transfer.source_paths.contains(&expected_path) {
            return false;
        }
        machine = transfer.target_machine;
        state = transfer.target_state;
    }
    let dispatch_parameter = forwarded
        .prior_transfers
        .last()
        .map(|transfer| transfer.parameter)
        .unwrap_or(forwarded.transfer.parameter);
    machine == forwarded.machine.symbol
        && state == forwarded.state.symbol
        && dispatch_parameter == forwarded.flow_call.receiver_symbol
}
