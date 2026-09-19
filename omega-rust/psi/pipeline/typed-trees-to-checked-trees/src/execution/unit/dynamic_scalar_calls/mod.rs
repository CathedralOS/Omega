//! Checked custody for calls through local named dynamic values.
//!
//! Collect descriptor transfers, build complete call plans, then compose the
//! checked calls into forwarding paths and joins. Failure to describe one call
//! omits that call, not unrelated machines' custody. Lowering still requires a
//! complete plan for every operation in the selected machine's closure.
//!
//! This owner consumes typed coordinates and joins them to checked conformance,
//! contract, value, and service-reach facts. It publishes source-handle-free
//! plans without depending on Terminal Psi. Scalar and Unit builders preserve
//! their distinct result contracts; neither may fabricate the other's result.

mod descriptor_transfers;
mod forwarded_calls;
mod join;
mod realization_bodies;
mod receivers;
mod scalar_call_plans;
mod unit;

use super::{
    BTreeMap, CheckFacts, CheckedBoundaryMachinePlan, CheckedStructuralAccess,
    CheckedUnitCallCoordinate, MachineSupplyMode, ServiceReachSummary, StatementNode, SymbolHandle,
    TypedTrees, state_flow,
};
use crate::execution::terminal_unit::ShapeCollector;
use descriptor_transfers::build_checked_dynamic_descriptor_transfers;
use forwarded_calls::build_checked_forwarded_dynamic_scalar_calls;
use receivers::{CheckedDynamicScalarCall, local_receiver_symbol, stored_dynamic_receiver};
use scalar_call_plans::build_checked_dynamic_scalar_call;
use typed_trees::name::Identifier;

pub(super) fn build_checked_dynamic_dispatch_plans(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    boundaries: &[CheckedBoundaryMachinePlan],
) -> checked_trees::CheckedDynamicDispatchPlans {
    let binding_facts = facts.dynamic_conformances.binding_facts();
    let mut plans = checked_trees::CheckedDynamicDispatchPlans {
        transfers: build_checked_dynamic_descriptor_transfers(program, facts, &binding_facts),
        ..checked_trees::CheckedDynamicDispatchPlans::default()
    };

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let Some(flow) = state_flow(facts, machine.symbol, state.symbol) else {
                continue;
            };
            for flow_call in facts.flow.control.calls.span_or_empty(flow.calls) {
                let Some(call_site) = crate::semantic_calls::find_call_site(
                    program,
                    machine.symbol,
                    state.symbol,
                    flow_call.statement_index,
                    flow_call.call_ordinal,
                ) else {
                    continue;
                };
                let receiver_symbol = local_receiver_symbol(program, &call_site);
                let stored_receiver = stored_dynamic_receiver(
                    program,
                    facts,
                    machine.symbol,
                    state.symbol,
                    flow_call.statement_index,
                    &call_site,
                );
                let is_direct_dynamic_receiver = receiver_symbol.is_some_and(|receiver_symbol| {
                    binding_facts.selections.iter().any(|selection| {
                        selection.machine == machine.symbol
                            && selection.state == state.symbol
                            && selection.statement_index < flow_call.statement_index
                            && selection.binding == receiver_symbol
                    })
                });
                if !is_direct_dynamic_receiver && stored_receiver.is_none() {
                    continue;
                }

                match &call_site {
                    crate::semantic_calls::CallSite::Statement(_) => {
                        let Some(call) = unit::build_checked_dynamic_unit_call(
                            program,
                            facts,
                            &binding_facts,
                            machine,
                            state,
                            flow_call,
                            call_site,
                            shapes,
                            None,
                        ) else {
                            continue;
                        };
                        match call {
                            unit::CheckedDynamicUnitCall::Direct(plan) => {
                                plans.direct_unit_calls.push(plan);
                            }
                            unit::CheckedDynamicUnitCall::Rebound(plan) => {
                                plans.rebound_unit_calls.push(plan);
                            }
                        }
                    }
                    _ => {
                        let Some(call) = build_checked_dynamic_scalar_call(
                            program,
                            facts,
                            &binding_facts,
                            machine,
                            state,
                            flow_call,
                            call_site,
                            shapes,
                            boundaries,
                            None,
                            stored_receiver,
                        ) else {
                            continue;
                        };
                        match call {
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
        }
    }

    build_checked_forwarded_dynamic_scalar_calls(
        program,
        facts,
        shapes,
        boundaries,
        &binding_facts,
        &mut plans,
    );
    join::promote_two_predecessor_dynamic_scalar_joins(program, facts, shapes, &mut plans);
    unit::build_checked_forwarded_dynamic_unit_calls(
        program,
        facts,
        shapes,
        &binding_facts,
        &mut plans,
    );
    join::promote_two_predecessor_dynamic_unit_joins(program, facts, shapes, &mut plans);

    plans
}
