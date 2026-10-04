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
    is_runtime_indexed_borrow_path, resolve_structural_path,
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

/// A non-consuming call loan presents one exact completed subtree. This checks
/// its static signature only; live custody and overlapping loans replay at the
/// call, and qualifications remain the argument checker's responsibility.
pub(crate) fn is_record_loan(
    module: &TerminalModule,
    caller: &TerminalMachine,
    parameter: &StructuralParameterDeclaration,
    argument: &StructuralArgument,
) -> bool {
    argument.access != StructuralAccess::Owned
        && parameter.access == argument.access
        && parameter.multiplicity == StructuralMultiplicity::Unrestricted
        && caller
            .structural_parameters
            .iter()
            .find(|source| source.place == argument.place)
            .filter(|source| {
                // Incoming unrestricted storage can lend a readable subtree.
                // Restricted ingress and write-only projection retain their
                // separate custody/material contracts, not this presentation.
                source.multiplicity == StructuralMultiplicity::Unrestricted
                    && source.access == StructuralAccess::Owned
                    && matches!(
                        argument.access,
                        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                    )
            })
            .map(|source| source.structural_type)
            .or_else(|| {
                crate::validation::structural::result_contracts::operation_result_loan_signature(
                    module,
                    caller,
                    argument.place,
                )
                .filter(|(_, access)| {
                    super::structural_arguments::structural_access_can_supply(
                        *access,
                        argument.access,
                    )
                })
                .map(|(signature, _)| signature.structural_type)
            })
            .or_else(|| {
                // A joined owned carrier still owns its fields. Shared field
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
                    })
                    .map(|source| source.structural_type)
            })
            .is_some_and(|root_type| {
                // A byte window presents the same initialized backing through
                // a view rather than its subtree type. Reuse the argument
                // check's exact geometry; availability and paired-loan checks
                // still establish that this source is live and may be lent.
                let backing = StructuralParameterDeclaration {
                    place: argument.place,
                    position: parameter.position,
                    is_self: false,
                    structural_type: root_type,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: argument.access,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                };
                let completed_result = caller
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .any(|operation| {
                        operation
                            .result
                            .structural()
                            .is_some_and(|result| result.place == argument.place)
                    });
                if completed_result
                    && terminal_semantics::fixed_byte_array_window(
                        module.structural_types.iter(),
                        &backing,
                        argument,
                        parameter,
                    )
                    .is_some()
                {
                    return true;
                }
                // A runtime-indexed loan of the record's exact element is the
                // same subtree signature the static resolver mints for a
                // literal index; the runtime resolver alone accepts the
                // `RuntimeIndex` hop.
                let exact_subtree = if is_runtime_indexed_borrow_path(&argument.path) {
                    crate::validation::foundation::resolve_runtime_projection(
                        module,
                        root_type,
                        &argument.path,
                    )
                } else {
                    resolve_structural_path(module, root_type, &argument.path)
                };
                exact_subtree == Some(parameter.structural_type)
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
