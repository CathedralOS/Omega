//! Exact representative entry identity plus initial termination and purity
//! certificates for quotient-operation planning.
//!
//! These certificates consume existing checked summaries. They do not perform
//! a second local effect analysis, discharge progress premises, or establish
//! result flow, contracts, `Respects`, or custody preservation.

use super::{RelationPlanError, RepresentativeStaticApplication};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::QuotientOperationRequest;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::SignatureContract;
use psi_typed_trees::state::State;
use psi_typed_trees::types::TypeReferenceHandle;

use super::static_application::derive_exact_representative_static_application;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::quotients) struct RepresentativeTermination {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::quotients) struct RepresentativePurity {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepresentativeRuntimeParameter {
    pub(super) symbol: SymbolHandle,
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) is_mutable: bool,
    pub(super) is_self: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct RepresentativeTelescope {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
    pub(super) parameters: Vec<RepresentativeRuntimeParameter>,
    pub(super) return_type: TypeReferenceHandle,
    pub(super) machine_contracts: HandleSpan<SignatureContract>,
    pub(super) state_contracts: HandleSpan<SignatureContract>,
    pub(super) static_application: RepresentativeStaticApplication,
}

pub(super) fn derive_representative_telescope(
    program: &TypedTrees,
    request: &QuotientOperationRequest,
) -> Result<RepresentativeTelescope, RelationPlanError> {
    let (machine, state) =
        representative_machine_state(program, request.representative_operation.symbol)?;
    let static_application = derive_exact_representative_static_application(program, request)?;
    if !state.return_type.is_valid() {
        return Err(RelationPlanError::RepresentativeResultTypeIsUnresolved);
    }
    let parameters = program
        .state_parameters(state)
        .iter()
        // This is only the RUNTIME telescope. Exact static/const argument
        // correspondence remains a later obligation over the retained static
        // application; filtering here does not discharge it.
        .filter(|parameter| !parameter.is_const)
        .map(|parameter| RepresentativeRuntimeParameter {
            symbol: parameter.symbol,
            type_reference: parameter.type_reference,
            is_mutable: parameter.is_mutable,
            is_self: parameter.is_self,
        })
        .collect();
    Ok(RepresentativeTelescope {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        parameters,
        return_type: state.return_type,
        machine_contracts: machine.contracts,
        state_contracts: state.contracts,
        static_application,
    })
}

pub(super) fn representative_machine_state(
    program: &TypedTrees,
    state_symbol: SymbolHandle,
) -> Result<(&Machine, &State), RelationPlanError> {
    let mut matches = program.machines().iter().flat_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == state_symbol)
            .map(move |state| (machine, state))
    });
    let Some((machine, state)) = matches.next() else {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    };
    if matches.next().is_some() {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    }
    Ok((machine, state))
}

/// Retain the exact local termination summary only when it is unconditional.
/// Progress-profile premises are observable admission dependencies and cannot
/// be silently discharged by the initial quotient wrapper.
pub(super) fn unconditional_representative_termination(
    program: &TypedTrees,
    representative: &RepresentativeTelescope,
) -> Option<RepresentativeTermination> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == representative.machine_symbol)?;
    match &machine.termination_plan.checked_summary {
        psi_language_semantics::TerminationGuarantee::Terminates { premises }
            if premises.is_empty() =>
        {
            Some(RepresentativeTermination {
                machine_symbol: representative.machine_symbol,
                state_symbol: representative.state_symbol,
            })
        }
        psi_language_semantics::TerminationGuarantee::NoGuarantee
        | psi_language_semantics::TerminationGuarantee::Terminates { .. } => None,
    }
}

/// Consume the shared whole-program operational and service-reach fixed points
/// to retain the initial pure-representative certificate. The exact entry must
/// have no mutable/out parameter, the enclosing machine must have no recursive
/// service, suspension, or blocking behavior, and every concrete call reachable
/// from that machine must retain a resolved machine target. Custody occurrence
/// remains a separate fence.
pub(in crate::quotients) fn pure_representative_effect(
    representative: &RepresentativeTelescope,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
) -> Option<RepresentativePurity> {
    if representative
        .parameters
        .iter()
        .any(|parameter| parameter.is_mutable)
    {
        return None;
    }
    let machine_summaries = operational
        .machines()
        .iter()
        .filter(|summary| summary.symbol == representative.machine_symbol)
        .collect::<Vec<_>>();
    let [machine_summary] = machine_summaries.as_slice() else {
        return None;
    };
    if machine_summary.transitive_may_suspend || machine_summary.transitive_may_block {
        return None;
    }
    let entry_summaries = operational
        .states
        .span_or_empty(machine_summary.states)
        .iter()
        .filter(|summary| summary.symbol == representative.state_symbol)
        .collect::<Vec<_>>();
    if entry_summaries.len() != 1 {
        return None;
    }
    let reach_summaries = service_reaches
        .machines()
        .iter()
        .filter(|summary| summary.machine == representative.machine_symbol)
        .collect::<Vec<_>>();
    let [reach_summary] = reach_summaries.as_slice() else {
        return None;
    };
    if !service_reaches
        .services(reach_summary.inferred_transitive)
        .is_empty()
    {
        return None;
    }

    let mut pending = vec![representative.machine_symbol];
    let mut visited = Vec::new();
    while let Some(machine_symbol) = pending.pop() {
        if visited.contains(&machine_symbol) {
            continue;
        }
        visited.push(machine_symbol);
        let summaries = operational
            .machines()
            .iter()
            .filter(|summary| summary.symbol == machine_symbol)
            .collect::<Vec<_>>();
        let [summary] = summaries.as_slice() else {
            return None;
        };
        for state in operational.states.span_or_empty(summary.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                if !call.target_machine_symbol.is_valid() {
                    return None;
                }
                pending.push(call.target_machine_symbol);
            }
        }
    }

    Some(RepresentativePurity {
        machine_symbol: representative.machine_symbol,
        state_symbol: representative.state_symbol,
    })
}
