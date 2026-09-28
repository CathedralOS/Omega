//! Forwarded dynamic calls and their exact transfer paths, for either result.
//!
//! A root caller passes its selected descriptor as the one argument of an
//! ordinary call. Each helper on the path takes that descriptor as its one
//! parameter and makes one call: forwarding it to the next helper, or, at the
//! end, dispatching the requirement through it. The walk is the same for a
//! scalar and a Unit call, and so is each helper's retained body: ordered
//! immutable scalar locals around that one call. The result lane decides the
//! call's form and how the body completes. A scalar helper's call binds a
//! local and checked scalar control returns; a Unit helper's call is a
//! statement and the body returns Unit after its last local.

use super::call_plans::{AuthoredCall, build_checked_dynamic_call};
use super::result_lanes::DynamicResultLane;
use crate::execution::terminal_unit::scalar_locals::scalar_expression_local_at;
use crate::execution::terminal_unit::types::{ShapeCollector, is_unit, state_flow};
use crate::execution::terminal_unit::{
    CheckFacts, CheckedBoundaryMachinePlan, CheckedScalarExpression, CheckedUnitCallCoordinate,
    CheckedUnitScalarResultBindingPlan, StatementNode, TypedTrees,
};
use crate::semantic::calls::CallSite;

/// Build every forwarded call of one result lane: each root call that
/// transfers one selected descriptor into a helper chain ending in a dispatch.
pub(super) fn build_checked_forwarded_dynamic_calls<Lane: DynamicResultLane>(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
    binding_facts: &checked_trees::DynamicConformanceBindingFacts,
    plans: &mut checked_trees::CheckedDynamicDispatchPlans,
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
                if !passes_only_the_descriptor::<Lane>(program, &outer_site) {
                    continue;
                }
                let Some(forwarded) = resolve_forwarded_dynamic_call::<Lane>(
                    program,
                    facts,
                    &plans.transfers,
                    transfer,
                ) else {
                    continue;
                };
                let plan = build_checked_dynamic_call::<Lane>(
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
                );
                plans.calls.extend(plan);
            }
        }
    }
}

/// The root call is authored in its lane's form and passes the descriptor as
/// its one argument, with no static application or evidence terms.
fn passes_only_the_descriptor<Lane: DynamicResultLane>(
    program: &TypedTrees,
    site: &CallSite<'_>,
) -> bool {
    Lane::authors(site)
        && AuthoredCall::of(program, site).is_some_and(|call| {
            call.ordinary_route
                && call.machine_arguments.is_empty()
                && call.evidence_free
                && call.argument_count == 1
        })
}

/// Follow the descriptor from the root transfer through each helper's one
/// call until a helper dispatches through its parameter.
fn resolve_forwarded_dynamic_call<'program, 'facts, Lane: DynamicResultLane>(
    program: &'program TypedTrees,
    facts: &'facts CheckFacts,
    transfers: &[checked_trees::CheckedDynamicDescriptorTransferPlan],
    root_transfer: checked_trees::CheckedDynamicDescriptorTransferPlan,
) -> Option<ForwardedDynamicCall<'program, 'facts, Lane::HelperBody>> {
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
        if AuthoredCall::of(program, &inner_site)?.argument_count
            != usize::from(!inner_call.has_receiver)
        {
            return None;
        }
        helpers.push(Lane::helper_body(
            program,
            facts,
            target_machine,
            target_state,
            inner_call,
            &inner_site,
        )?);
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
pub(super) fn scalar_helper_body(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    inner_call: &checked_trees::FlowCallFact,
    inner_site: &CallSite<'_>,
) -> Option<checked_trees::CheckedDynamicScalarHelperPlan> {
    let (scalar_control, prefix_count) =
        crate::execution::terminal_unit::control::scalar_control(program, facts, machine, state)?;
    let statements = program
        .statement_table
        .statements(state.statement_nodes)
        .get(..prefix_count)?;
    let CallSite::Expression { expression, .. } = *inner_site else {
        return None;
    };
    let StatementNode::LocalData(call_local) = statements.get(inner_call.statement_index)? else {
        return None;
    };
    if call_local.is_mutable
        || !call_local.symbol.is_valid()
        || call_local.initial_value != expression
        || program.primitive_type_reference(call_local.type_reference)
            != Some(scalar_control.primitive_type)
    {
        return None;
    }
    // Every statement before the call is a local, so the call's binding
    // ordinal is its statement index.
    let call_statement = u32::try_from(inner_call.statement_index).ok()?;
    Some(checked_trees::CheckedDynamicScalarHelperPlan {
        machine: machine.symbol,
        state: state.symbol,
        call_result: CheckedUnitScalarResultBindingPlan {
            statement_index: call_statement,
            binding_ordinal: call_statement,
            primitive_type: scalar_control.primitive_type,
        },
        scalar_locals: helper_scalar_locals(
            program,
            facts,
            state,
            statements,
            inner_call.statement_index,
            true,
        )?,
        scalar_control,
    })
}

/// A Unit helper's call statement among ordered immutable locals. The state
/// returns Unit after its last statement, so nothing follows the locals.
pub(super) fn unit_helper_body(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    inner_call: &checked_trees::FlowCallFact,
    inner_site: &CallSite<'_>,
) -> Option<checked_trees::CheckedDynamicUnitHelperPlan> {
    let CallSite::Statement(call) = *inner_site else {
        return None;
    };
    let statements = program.statement_table.statements(state.statement_nodes);
    if !is_unit(program, state.return_type)
        || !matches!(
            statements.get(inner_call.statement_index),
            Some(StatementNode::Call(statement)) if std::ptr::eq(statement, call)
        )
    {
        return None;
    }
    Some(checked_trees::CheckedDynamicUnitHelperPlan {
        machine: machine.symbol,
        state: state.symbol,
        call_statement_index: u32::try_from(inner_call.statement_index).ok()?,
        scalar_locals: helper_scalar_locals(
            program,
            facts,
            state,
            statements,
            inner_call.statement_index,
            false,
        )?,
    })
}

/// The checked initializers of a helper's locals, in authored order. Every
/// statement but the call at `call_statement` must be an immutable scalar
/// local. Each local takes the next binding ordinal of the helper's scalar
/// namespace; the call takes one too when it binds a result.
fn helper_scalar_locals(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    call_statement: usize,
    call_binds_result: bool,
) -> Option<Vec<(CheckedUnitScalarResultBindingPlan, CheckedScalarExpression)>> {
    let mut scalar_locals = Vec::new();
    let mut binding_ordinal = 0_u32;
    for (ordinal, statement) in statements.iter().enumerate() {
        if ordinal == call_statement {
            binding_ordinal = binding_ordinal.checked_add(u32::from(call_binds_result))?;
            continue;
        }
        let StatementNode::LocalData(local) = statement else {
            return None;
        };
        if local.is_mutable || !local.symbol.is_valid() {
            return None;
        }
        scalar_locals.push(scalar_expression_local_at(
            program,
            facts,
            state,
            u32::try_from(ordinal).ok()?,
            binding_ordinal,
            local,
        )?);
        binding_ordinal = binding_ordinal.checked_add(1)?;
    }
    Some(scalar_locals)
}

pub(super) struct ForwardedDynamicCall<'program, 'facts, HelperBody> {
    /// The helper bodies, outermost first.
    pub(super) helpers: Vec<HelperBody>,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) flow_call: &'facts checked_trees::FlowCallFact,
    pub(super) call_site: CallSite<'program>,
    pub(super) transfer: checked_trees::CheckedDynamicDescriptorTransferPlan,
    pub(super) prior_transfers: Vec<checked_trees::CheckedDynamicDescriptorTransferPlan>,
}

pub(super) fn forwarded_transfer_path_is_exact<HelperBody>(
    forwarded: &ForwardedDynamicCall<'_, '_, HelperBody>,
) -> bool {
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
