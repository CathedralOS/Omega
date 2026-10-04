//! Reconstruct empty-body cleanup consumption from source entry and exit custody.
//! A nominal hook is separate authority: this check conserves the exact claim
//! occurrence that the checked consuming machine promises to dispose.

use super::{CheckedNominalAffineUnitCleanupMachinePlan, CheckedTrees, LoweringError, unsupported};
use language_semantics::{
    Multiplicity, PermissionAccess, PermissionClaimIdentity, PermissionEventKind,
    PermissionEventSource, PermissionProvenance,
};

pub(super) fn validate_checked_dispositions(
    checked: &CheckedTrees,
    nominal: &CheckedNominalAffineUnitCleanupMachinePlan,
) -> Result<(), LoweringError> {
    let plan = &nominal.machine;
    let source = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == plan.machine)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup source machine is absent",
        ))?;
    let state = checked
        .typed
        .machine_states(source)
        .iter()
        .find(|state| state.symbol == plan.state)
        .ok_or(LoweringError::Unsupported(
            "nominal cleanup source state is absent",
        ))?;
    let parameters = checked.typed.state_parameters(state);
    if parameters.len() != plan.structural_parameters.len() {
        return unsupported("nominal cleanup source parameter roster changed");
    }
    for (index, (parameter, declaration)) in parameters
        .iter()
        .zip(&plan.structural_parameters)
        .enumerate()
    {
        if parameter.is_self
            || parameter.is_const
            || parameter.is_mutable
            || declaration.is_self
            || declaration.position as usize != index
            || declaration.access != checked_trees::CheckedStructuralAccess::Owned
            || declaration.type_identity
                != checked
                    .normalized_type_identity(parameter.type_reference)
                    .as_str()
            || declaration.multiplicity != checked.type_multiplicity(parameter.type_reference)
            || !declaration.qualifications.is_empty()
        {
            return unsupported("nominal cleanup substituted its source owner declaration");
        }
        let claims = plan
            .entry_claims
            .iter()
            .filter(|claim| claim.parameter_index as usize == index)
            .collect::<Vec<_>>();
        let root = facts::PlaceRoot::Symbol(parameter.symbol);
        let events = checked
            .facts
            .flow
            .ownership
            .permissions
            .iter()
            .map(|(_, event)| event)
            .filter(|event| {
                event.machine_symbol == plan.machine
                    && event.state_symbol == plan.state
                    && event.root == root
            })
            .collect::<Vec<_>>();
        let exits = events
            .iter()
            .copied()
            .filter(|event| event.source == PermissionEventSource::StateExit)
            .collect::<Vec<_>>();
        let [exit] = exits.as_slice() else {
            return unsupported("nominal cleanup lost its unique source disposition");
        };
        if exit.evaluation != checked_trees::FlowPermissionEvaluation::Statement
            || exit.access != PermissionAccess::Owned
            || !exit.segments.is_empty()
            || exit.obligation_live
            || exit.multiplicity != declaration.multiplicity
        {
            return unsupported("nominal cleanup changed source disposition custody");
        }
        match declaration.multiplicity {
            Multiplicity::Affine
                if claims.is_empty()
                    && exit.kind == PermissionEventKind::AffineDrop
                    && exit.claim_identity == PermissionClaimIdentity::Unknown
                    && exit.provenance == PermissionProvenance::Unknown => {}
            Multiplicity::Linear => {
                let [claim] = claims.as_slice() else {
                    return unsupported("linear nominal cleanup lost its exact whole entry claim");
                };
                let entries = events
                    .iter()
                    .copied()
                    .filter(|event| {
                        event.source == PermissionEventSource::StateEntry
                            && event.kind == PermissionEventKind::Establish
                    })
                    .collect::<Vec<_>>();
                let [entry] = entries.as_slice() else {
                    return unsupported("linear nominal cleanup changed source entry custody");
                };
                if !claim.path.is_empty()
                    || entry.evaluation != checked_trees::FlowPermissionEvaluation::Statement
                    || exit.kind != PermissionEventKind::Consume
                    || claim.claim_identity == PermissionClaimIdentity::Unknown
                    || entry.claim_identity != claim.claim_identity
                    || exit.claim_identity != claim.claim_identity
                    || entry.provenance != exit.provenance
                    || exit.provenance == PermissionProvenance::Unknown
                    || entry.multiplicity != Multiplicity::Linear
                    || entry.access != PermissionAccess::Owned
                    || !entry.segments.is_empty()
                    || !entry.obligation_live
                {
                    return unsupported("linear nominal cleanup substituted its entry obligation");
                }
            }
            _ => return unsupported("nominal cleanup has no authorized owned disposition"),
        }
    }
    if plan
        .entry_claims
        .iter()
        .any(|claim| claim.parameter_index as usize >= parameters.len())
    {
        return unsupported("nominal cleanup has an unbound entry claim");
    }
    Ok(())
}
