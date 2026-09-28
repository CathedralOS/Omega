//! Service reach and Unit call claim transfers.
//!
//! `validate_unit_call_claim_transfers` checks each argument's claims against
//! its callee parameter (`argument_claims`), then that the transfer roster
//! covers the callee's entry claims exactly (`transfer_roster`).

mod argument_claims;
mod transfer_roster;

use crate::validation::{
    ClaimId, ClaimTransfer, ModuleError, OperationId, PlaceId, ServiceId, StructuralAccess,
    StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, TerminalMachine, TerminalModule, is_nonempty_field_path,
    resolve_structural_path,
};

pub(crate) fn validate_service_reach(
    operation: OperationId,
    caller: &[ServiceId],
    reached: &[ServiceId],
) -> Result<(), ModuleError> {
    if let Some(service) = reached.iter().find(|service| !caller.contains(service)) {
        return Err(ModuleError::OperationServiceOutsidePublishedCeiling {
            operation,
            service: *service,
        });
    }
    Ok(())
}

pub(crate) fn is_record_loan(
    module: &TerminalModule,
    caller: &TerminalMachine,
    parameter: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    argument.access != StructuralAccess::Owned
        && parameter.access == argument.access
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && crate::validation::record::completed_source(module, caller, argument.place)
            .map(|result| result.structural_type)
            .or_else(|| {
                // A joined plain record still owns its fields. Shared field
                // access borrows that selected storage rather than transferring
                // it. Block declaration, dominance and live-frontier checks
                // remain mandatory; this is only the exact loan signature.
                crate::validation::block_views::parameter(caller, argument.place)
                    .filter(|source| {
                        argument.access == StructuralAccess::SharedBorrow
                            && is_nonempty_field_path(&argument.path)
                            && source.access == StructuralAccess::Owned
                            && matches!(
                                source.multiplicity,
                                StructuralMultiplicity::Affine
                                    | StructuralMultiplicity::Unrestricted
                            )
                            && source.qualifications.is_empty()
                            && source.projected_qualifications.is_empty()
                            && crate::validation::record::plain_type(module, source.structural_type)
                    })
                    .map(|source| source.structural_type)
            })
            .is_some_and(|root_type| {
                resolve_structural_path(module, root_type, &argument.path)
                    == Some(parameter.structural_type)
            })
}

pub(crate) fn validate_unit_call_claim_transfers(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    operation: OperationId,
) -> Result<(), ModuleError> {
    for (argument_index, (argument, parameter)) in arguments
        .iter()
        .zip(&callee.structural_parameters)
        .enumerate()
    {
        argument_claims::validate_argument_claims(
            module,
            caller,
            callee,
            operation,
            argument_index,
            argument,
            parameter,
        )?;
    }
    let callee_claims = transfer_roster::callee_claims(callee, operation)?;
    transfer_roster::validate_transfer_roster(
        caller,
        callee,
        arguments,
        transfers,
        operation,
        callee_claims,
    )?;
    Ok(())
}

pub(crate) fn claim_input(
    machine: &TerminalMachine,
    claim: ClaimId,
) -> Option<(PlaceId, &[StructuralPathSegment])> {
    machine
        .entry_claims
        .iter()
        .find_map(|candidate| {
            (candidate.claim == claim).then_some((candidate.input, candidate.path.as_slice()))
        })
        .or_else(|| {
            machine.content_entry_claims.iter().find_map(|candidate| {
                (candidate.claim == claim)
                    .then_some((candidate.input.root, &[] as &[StructuralPathSegment]))
            })
        })
}
