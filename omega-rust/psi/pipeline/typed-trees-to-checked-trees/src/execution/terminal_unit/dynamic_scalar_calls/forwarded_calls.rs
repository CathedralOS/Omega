//! Forwarded dynamic calls and their exact transfer paths, for either result.
//!
//! A root caller passes its selected descriptor as the one argument of an
//! ordinary call. Each helper on the path takes that descriptor as its one
//! parameter and makes one call: forwarding it to the next helper, or, at the
//! end, dispatching the requirement through it. The walk is the same for a
//! scalar and a Unit call; only the helper body differs. A scalar helper
//! binds its call's result among ordered pure locals and returns through
//! checked scalar control, and that body is retained. A Unit helper retains
//! no body plan, so its forwarding call must be its entire body.

use crate::execution::terminal_unit::dynamic_scalar_calls::scalar_call_plans::build_checked_dynamic_scalar_call;
use crate::execution::terminal_unit::dynamic_scalar_calls::unit::build_checked_dynamic_unit_call;
use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, state_flow};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedUnitCallCoordinate, StatementNode, TypedTrees,
};
use crate::semantic::calls::CallSite;

/// The result a forwarded call returns to its root caller, which fixes how
/// every call on its path is authored: a call statement for a Unit
/// requirement, or a call expression that a scalar local binds.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForwardedCallResult {
    Scalar,
    Unit,
}

/// Build every forwarded call of one result: each root call that transfers
/// one selected descriptor into a helper chain ending in a dispatch.
pub(crate) fn build_checked_forwarded_dynamic_calls(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    binding_facts: &checked_trees::DynamicConformanceBindingFacts,
    plans: &mut checked_trees::CheckedDynamicDispatchPlans,
    result: ForwardedCallResult,
) {
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
                let Some(outer_site) = crate::semantic::calls::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    outer_call.statement_index,
                    outer_call.call_ordinal,
                ) else {
                    continue;
                };
                if !passes_only_the_descriptor(program, &outer_site, result) {
                    continue;
                }
                let Some(forwarded) = resolve_forwarded_dynamic_call(
                    program,
                    facts,
                    &plans.transfers,
                    transfer,
                    result,
                ) else {
                    continue;
                };
                let plan = match result {
                    ForwardedCallResult::Scalar => build_checked_dynamic_scalar_call(
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
                    )
                    .map(checked_trees::CheckedDynamicDispatchPlan::Scalar),
                    ForwardedCallResult::Unit => build_checked_dynamic_unit_call(
                        program,
                        facts,
                        binding_facts,
                        machine,
                        state,
                        outer_call,
                        outer_site,
                        shapes,
                        Some(forwarded),
                    )
                    .map(checked_trees::CheckedDynamicDispatchPlan::Unit),
                };
                plans.calls.extend(plan);
            }
        }
    }
}

/// The root call is authored in its result's form and passes the descriptor
/// as its one argument, with no static application or evidence terms.
fn passes_only_the_descriptor(
    program: &TypedTrees,
    site: &CallSite<'_>,
    result: ForwardedCallResult,
) -> bool {
    match (result, site) {
        (ForwardedCallResult::Scalar, CallSite::Expression { call, .. }) => {
            call.selects_only_nominal_route()
                && call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty()
                && call_site_argument_count(program, site) == Some(1)
        }
        (ForwardedCallResult::Unit, CallSite::Statement(call)) => {
            call.static_requirement_dispatch.is_none()
                && call.machine_arguments.is_empty()
                && call.evidence_arguments.is_empty()
                && !call.discards_result
                && call_site_argument_count(program, site) == Some(1)
        }
        _ => false,
    }
}

fn call_site_argument_count(program: &TypedTrees, site: &CallSite<'_>) -> Option<usize> {
    match site {
        CallSite::Statement(call) => Some(
            program
                .statement_table
                .expression_handles(call.arguments)
                .len(),
        ),
        CallSite::Expression { call, .. } => Some(
            program
                .expression_table
                .expression_handles(call.arguments)
                .len(),
        ),
        CallSite::TransitionNamed { .. } => None,
    }
}

/// Follow the descriptor from the root transfer through each helper's one
/// call until a helper dispatches through its parameter.
fn resolve_forwarded_dynamic_call<'program, 'facts>(
    program: &'program TypedTrees,
    facts: &'facts CheckFacts,
    transfers: &[checked_trees::CheckedDynamicDescriptorTransferPlan],
    root_transfer: checked_trees::CheckedDynamicDescriptorTransferPlan,
    result: ForwardedCallResult,
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
        let target_state = crate::semantic::calls::find_state(program, current.target_state)?;
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
        let inner_site = crate::semantic::calls::find_call_site(
            program,
            target_machine.symbol,
            target_state.symbol,
            inner_call.statement_index,
            inner_call.call_ordinal,
        )?;
        if call_site_argument_count(program, &inner_site)? != usize::from(!inner_call.has_receiver)
        {
            return None;
        }
        match result {
            ForwardedCallResult::Scalar => helpers.push(scalar_helper_body(
                program,
                facts,
                target_machine,
                target_state,
                inner_call,
                &inner_site,
            )?),
            ForwardedCallResult::Unit => {
                unit_helper_body(program, target_state, inner_call, &inner_site)?
            }
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

/// A scalar helper's ordered immutable locals, one of which binds its call's
/// result, followed by checked scalar control of the call's result type.
fn scalar_helper_body(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    inner_call: &checked_trees::FlowCallFact,
    inner_site: &CallSite<'_>,
) -> Option<checked_trees::CheckedDynamicScalarHelperPlan> {
    let (scalar_control, prefix_count) =
        crate::execution::terminal_unit::control::scalar_control(program, facts, machine, state)?;
    if inner_call.statement_index >= prefix_count {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
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
            scalar_locals.push(
                crate::execution::terminal_unit::scalar_locals::scalar_expression_local_at(
                    program, facts, state, coordinate, coordinate, local,
                )?,
            );
        }
    }
    let StatementNode::LocalData(helper_result) = &statements[inner_call.statement_index] else {
        return None;
    };
    let CallSite::Expression { expression, .. } = inner_site else {
        return None;
    };
    if helper_result.initial_value != *expression {
        return None;
    }
    Some(checked_trees::CheckedDynamicScalarHelperPlan {
        machine: machine.symbol,
        state: state.symbol,
        call_result: call_result?,
        scalar_locals,
        scalar_control,
    })
}

/// A Unit helper returns Unit and retains no body plan: lowering emits only
/// its forwarding call, so that call statement must be its entire body.
fn unit_helper_body(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    inner_call: &checked_trees::FlowCallFact,
    inner_site: &CallSite<'_>,
) -> Option<()> {
    let [StatementNode::Call(helper_call)] =
        program.statement_table.statements(state.statement_nodes)
    else {
        return None;
    };
    let CallSite::Statement(inner_statement_call) = inner_site else {
        return None;
    };
    (is_unit(program, state.return_type)
        && inner_call.statement_index == 0
        && std::ptr::eq(*inner_statement_call, helper_call))
    .then_some(())
}

pub(crate) struct ForwardedDynamicCall<'program, 'facts> {
    /// The scalar helper bodies, outermost first; empty on a Unit path.
    pub(crate) helpers: Vec<checked_trees::CheckedDynamicScalarHelperPlan>,
    pub(crate) machine: &'program typed_trees::machine::Machine,
    pub(crate) state: &'program typed_trees::state::State,
    pub(crate) flow_call: &'facts checked_trees::FlowCallFact,
    pub(crate) call_site: CallSite<'program>,
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
