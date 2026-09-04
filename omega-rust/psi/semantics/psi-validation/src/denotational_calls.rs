//! Shared eligibility for calls whose result is used denotationally.
//!
//! Quotient operations and fact-position call projections must agree on the
//! operational meaning of "pure and unconditionally terminating".  This
//! module consumes the existing whole-program summaries; it never performs a
//! second expression-local effect inference.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

pub(crate) fn unconditionally_terminates(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
) -> bool {
    let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
    else {
        return false;
    };
    matches!(
        &machine.termination_plan.checked_summary,
        psi_language_semantics::TerminationGuarantee::Terminates { premises }
            if premises.is_empty()
    )
}

/// Whole-closure purity used by denotational calls.  The selected entry must
/// have no mutable runtime parameter; its machine has no service reach,
/// suspension, or blocking behavior; and every reachable call target is one
/// exact checked machine.
pub(crate) fn has_pure_effect_closure(
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    has_mutable_parameters: bool,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
) -> bool {
    if has_mutable_parameters {
        return false;
    }

    let machine_summaries = operational
        .machines()
        .iter()
        .filter(|summary| summary.symbol == machine_symbol)
        .collect::<Vec<_>>();
    let [machine_summary] = machine_summaries.as_slice() else {
        return false;
    };
    if machine_summary.transitive_may_suspend || machine_summary.transitive_may_block {
        return false;
    }
    if operational
        .states
        .span_or_empty(machine_summary.states)
        .iter()
        .filter(|summary| summary.symbol == state_symbol)
        .count()
        != 1
    {
        return false;
    }

    let reach_summaries = service_reaches
        .machines()
        .iter()
        .filter(|summary| summary.machine == machine_symbol)
        .collect::<Vec<_>>();
    let [reach_summary] = reach_summaries.as_slice() else {
        return false;
    };
    if !service_reaches
        .services(reach_summary.inferred_transitive)
        .is_empty()
    {
        return false;
    }

    let mut pending = vec![machine_symbol];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == current)
            .collect::<Vec<_>>();
        let [summary] = summaries.as_slice() else {
            return false;
        };
        for state in operational.states.span_or_empty(summary.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.target_machine_symbol.is_valid() {
                    return false;
                }
                pending.push(call.target_machine_symbol);
            }
        }
    }
    true
}

/// A fact denotes only normal values. Any reachable published crash route
/// makes the call partial in the fact language, even though the route is
/// separately covered for executable use.
pub(crate) fn has_no_crash_routes(
    program: &TypedTrees,
    machine_symbol: SymbolHandle,
    operational: &psi_effects::OperationalPlan,
) -> bool {
    let mut pending = vec![machine_symbol];
    let mut visited = Vec::new();
    while let Some(current) = pending.pop() {
        if visited.contains(&current) {
            continue;
        }
        visited.push(current);
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == current)
        else {
            return false;
        };
        let crashes = program
            .machine_contracts(machine)
            .iter()
            .chain(
                program
                    .machine_states(machine)
                    .iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .any(|contract| {
                matches!(
                    contract.kind,
                    psi_typed_trees::signature::SignatureContractKind::Crashes { .. }
                )
            });
        if crashes {
            return false;
        }
        let summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == current)
            .collect::<Vec<_>>();
        let [summary] = summaries.as_slice() else {
            return false;
        };
        for state in operational.states.span_or_empty(summary.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.target_machine_symbol.is_valid() {
                    return false;
                }
                pending.push(call.target_machine_symbol);
            }
        }
    }
    true
}
