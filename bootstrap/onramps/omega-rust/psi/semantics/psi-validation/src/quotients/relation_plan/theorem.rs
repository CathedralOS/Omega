//! Exact selected resultless theorem identity for quotient-operation planning.
//!
//! This first rung selects and fences the theorem machine application. It does
//! not yet derive or prove the expected congruence schema and therefore grants
//! no executable quotient authority.

use super::representative::RepresentativeRuntimeParameter;
use super::static_application::validate_static_application;
use super::{RelationPlanError, RepresentativeStaticApplication};
use psi_arena::HandleSpan;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::expression::QuotientOperationRequest;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::signature::SignatureContract;
use psi_typed_trees::state::State;
use psi_typed_trees::types::TypeReferenceNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::quotients) struct SelectedTheoremTermination {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::quotients) struct SelectedTheoremPurity {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::quotients) struct SelectedTheoremTelescope {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
    pub(super) parameters: Vec<RepresentativeRuntimeParameter>,
    pub(super) machine_contracts: HandleSpan<SignatureContract>,
    pub(super) state_contracts: HandleSpan<SignatureContract>,
    pub(super) static_application: RepresentativeStaticApplication,
}

pub(super) fn derive_selected_theorem_telescope(
    program: &TypedTrees,
    request: &QuotientOperationRequest,
) -> Result<SelectedTheoremTelescope, RelationPlanError> {
    let selected = &request.selected_theorem;
    let (machine, state) = selected_theorem_machine_state(program, selected.symbol)?;
    if machine.supply_mode != psi_language_semantics::MachineSupplyMode::CheckedBody
        || !machine.body_is_present
    {
        return Err(RelationPlanError::TheoremMustBeCheckedBody);
    }
    if !matches!(
        program
            .type_reference_table
            .type_reference(state.return_type),
        TypeReferenceNode::Unit
    ) {
        return Err(RelationPlanError::TheoremMustBeResultless);
    }
    let static_application = validate_static_application(
        program,
        &machine.lifetime_parameters,
        program.machine_type_parameters(machine),
        selected,
    )
    .map_err(|_| RelationPlanError::TheoremStaticApplicationInvalid)?;
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_const)
        .map(|parameter| RepresentativeRuntimeParameter {
            symbol: parameter.symbol,
            type_reference: parameter.type_reference,
            is_mutable: parameter.is_mutable,
            is_self: parameter.is_self,
        })
        .collect();
    Ok(SelectedTheoremTelescope {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        parameters,
        machine_contracts: machine.contracts,
        state_contracts: state.contracts,
        static_application,
    })
}

fn selected_theorem_machine_state(
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
    let Some(selected) = matches.next() else {
        return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
    };
    if matches.next().is_some() {
        return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
    }
    Ok(selected)
}

pub(super) fn unconditional_selected_theorem_termination(
    program: &TypedTrees,
    theorem: &SelectedTheoremTelescope,
) -> Option<SelectedTheoremTermination> {
    crate::denotational_calls::unconditionally_terminates(program, theorem.machine_symbol)
        .then_some(SelectedTheoremTermination {
            machine_symbol: theorem.machine_symbol,
            state_symbol: theorem.state_symbol,
        })
}

pub(super) fn pure_selected_theorem_effect(
    theorem: &SelectedTheoremTelescope,
    operational: &psi_effects::OperationalPlan,
    service_reaches: &psi_effects::ServiceReachInferencePlan,
) -> Option<SelectedTheoremPurity> {
    crate::denotational_calls::has_pure_effect_closure(
        theorem.machine_symbol,
        theorem.state_symbol,
        theorem
            .parameters
            .iter()
            .any(|parameter| parameter.is_mutable),
        operational,
        service_reaches,
    )
    .then_some(SelectedTheoremPurity {
        machine_symbol: theorem.machine_symbol,
        state_symbol: theorem.state_symbol,
    })
}
