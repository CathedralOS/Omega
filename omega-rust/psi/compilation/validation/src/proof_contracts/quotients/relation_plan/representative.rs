//! Exact representative entry identity for quotient-operation planning.
//!
//! The representative runtime telescope and its exact static application are
//! consumed by the shared termination and purity fences in `relation_plan`,
//! which read existing checked summaries. They do not perform a second local
//! effect analysis, discharge progress premises, or establish result flow,
//! contracts, `Respects`, or custody preservation.

use super::{RelationPlanError, RepresentativeStaticApplication};
use arena::HandleSpan;
use symbols::SymbolHandle;
use typed_trees::expression::QuotientOperationRequest;
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContract;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

use super::static_application::derive_exact_representative_static_application;
pub(in crate::proof_contracts::quotients) use super::theorem::MachineStateCoordinate;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RepresentativeRuntimeParameter {
    pub(super) symbol: SymbolHandle,
    pub(super) type_reference: TypeReferenceHandle,
    pub(super) is_mutable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::proof_contracts::quotients) struct RepresentativeTelescope {
    pub(super) machine_symbol: SymbolHandle,
    pub(super) state_symbol: SymbolHandle,
    pub(super) parameters: Vec<RepresentativeRuntimeParameter>,
    pub(super) return_type: TypeReferenceHandle,
    pub(super) machine_contracts: HandleSpan<SignatureContract>,
    pub(super) state_contracts: HandleSpan<SignatureContract>,
    pub(super) static_application: RepresentativeStaticApplication,
}

pub(super) fn derive_representative_telescope(
    program: &dyn crate::CheckingInput,
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
    program: &dyn crate::CheckingInput,
    state_symbol: SymbolHandle,
) -> Result<(&Machine, &State), RelationPlanError> {
    let mut matches = program.machine_states_named(state_symbol);
    let Some((machine, state)) = matches.next() else {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    };
    if matches.next().is_some() {
        return Err(RelationPlanError::RepresentativeEntryDoesNotResolveExactly);
    }
    Ok((machine, state))
}
