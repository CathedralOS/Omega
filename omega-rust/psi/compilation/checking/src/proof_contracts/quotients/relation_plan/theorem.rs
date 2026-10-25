//! Exact selected resultless theorem identity for quotient-operation planning.
//!
//! This owner selects and fences the theorem machine application. Structural
//! schema verification is separate and neither operation proves congruence or
//! grants executable quotient authority.

use super::representative::RepresentativeRuntimeParameter;
use super::static_application::validate_static_application;
use super::{RelationPlanError, RepresentativeStaticApplication};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContract;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceNode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct MachineStateCoordinate {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct SelectedTheoremTelescope {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
    pub(super) parameters: Vec<RepresentativeRuntimeParameter>,
    pub(super) machine_contracts: HandleSpan<SignatureContract>,
    pub(super) state_contracts: HandleSpan<SignatureContract>,
    pub(super) static_application: RepresentativeStaticApplication,
}

pub(super) fn derive_selected_theorem_telescope(
    program: &dyn crate::CheckingInput,
    selected: &StaticMachineArgument,
) -> Result<SelectedTheoremTelescope, RelationPlanError> {
    let (machine, state) = selected_theorem_machine_state(program, selected.symbol)?;
    if machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
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
        .public_state_parameters(state)
        .map(|parameter| RepresentativeRuntimeParameter {
            symbol: parameter.symbol,
            type_reference: parameter.type_reference,
            is_mutable: parameter.is_mutable,
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
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
) -> Result<(&Machine, &State), RelationPlanError> {
    let mut matches = program.machine_states_named(state_symbol);
    let Some(selected) = matches.next() else {
        return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
    };
    if matches.next().is_some() {
        return Err(RelationPlanError::TheoremEntryDoesNotResolveExactly);
    }
    Ok(selected)
}
