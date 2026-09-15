//! Service reach and Unit call claim transfers.

use crate::validation::structural_operations::structural_arguments::{
    is_unrestricted_mutable_subloan, is_unrestricted_shared_subloan,
    is_unrestricted_write_only_subloan, linear_call_result,
};
use crate::validation::{
    BTreeMap, BTreeSet, ClaimId, ClaimTransfer, ModuleError, OperationId, OperationKind, PlaceId,
    ServiceId, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, TerminalMachine, TerminalModule,
    is_nonempty_field_path, is_partial_affine_path, partial_affine_root_type,
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
        if !argument.path.is_empty() {
            let callee_claims = callee
                .entry_claims
                .iter()
                .filter(|claim| claim.input == parameter.place)
                .collect::<Vec<_>>();
            // A reference carrier's affine permission is not a projected
            // ownership claim on the primitive referent passed to this call.
            let claim_free_reference =
                crate::validation::references::is_reference_projection(module, caller, argument)
                    && crate::validation::references::source_type(module, caller, argument)
                        == Some(parameter.structural_type)
                    && parameter.access == argument.access
                    && parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    && callee_claims.is_empty()
                    && caller
                        .entry_claims
                        .iter()
                        .all(|claim| claim.input != argument.place);
            let claim_free_unrestricted_write_only_field =
                is_unrestricted_write_only_subloan(module, caller, parameter, argument)
                    && callee_claims.is_empty()
                    && caller
                        .entry_claims
                        .iter()
                        .all(|claim| claim.input != argument.place);
            let claim_free_unrestricted_shared_field =
                is_unrestricted_shared_subloan(caller, parameter, argument)
                    && callee_claims.is_empty()
                    && caller
                        .entry_claims
                        .iter()
                        .all(|claim| claim.input != argument.place);
            let claim_free_unrestricted_mutable_field =
                is_unrestricted_mutable_subloan(caller, parameter, argument)
                    && callee_claims.is_empty()
                    && caller
                        .entry_claims
                        .iter()
                        .all(|claim| claim.input != argument.place);
            // Parameter projections include borrowed dynamic-provider fields;
            // they do not require an owned partial-cleanup root. Only the new
            // result route must establish plain affine call-result custody.
            let root_type = caller
                .structural_parameters
                .iter()
                .find(|actual| actual.place == argument.place)
                .map(|actual| actual.structural_type)
                .or_else(|| partial_affine_root_type(caller, argument.place));
            let claim_free_direct_affine = root_type.is_some_and(|structural_type| {
                is_partial_affine_path(module, structural_type, &argument.path)
            }) && parameter.multiplicity
                == StructuralMultiplicity::Affine
                && callee_claims.is_empty()
                && caller
                    .entry_claims
                    .iter()
                    .all(|claim| claim.input != argument.place);
            if !(is_record_loan(module, caller, parameter, argument) && callee_claims.is_empty())
                && !claim_free_unrestricted_write_only_field
                && !claim_free_reference
                && !claim_free_unrestricted_shared_field
                && !claim_free_unrestricted_mutable_field
                && !claim_free_direct_affine
                && !matches!(callee_claims.as_slice(), [claim] if claim.path.is_empty())
            {
                return Err(ModuleError::UnitCallClaimPresenceMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
            if caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == argument.place)
                || callee
                    .content_entry_claims
                    .iter()
                    .any(|claim| claim.input.root == parameter.place)
            {
                return Err(ModuleError::UnitCallContentClaimMismatch {
                    operation,
                    argument_index: argument_index as u32,
                });
            }
        }
        let mut caller_claim_paths = caller
            .entry_claims
            .iter()
            .filter(|claim| claim.input == argument.place && claim.path.starts_with(&argument.path))
            .map(|claim| &claim.path[argument.path.len()..])
            .collect::<Vec<_>>();
        if let Some((_, result)) = linear_call_result(caller, argument.place) {
            caller_claim_paths.extend(
                result
                    .claims
                    .iter()
                    .filter(|claim| claim.path.starts_with(&argument.path))
                    .map(|claim| &claim.path[argument.path.len()..]),
            );
        }
        let mut callee_claim_paths = callee
            .entry_claims
            .iter()
            .filter(|claim| claim.input == parameter.place)
            .map(|claim| claim.path.as_slice())
            .collect::<Vec<_>>();
        caller_claim_paths.sort();
        callee_claim_paths.sort();
        if caller_claim_paths != callee_claim_paths {
            return Err(ModuleError::UnitCallClaimPresenceMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
        let mut caller_content = caller
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == argument.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        if let Some((producer, result)) = linear_call_result(caller, argument.place) {
            let mismatch = || ModuleError::UnitCallContentClaimMismatch {
                operation,
                argument_index: argument_index as u32,
            };
            let (OperationKind::CallStructural {
                callee: producer_callee,
                returned_claim_transfers,
                ..
            }
            | OperationKind::CallStructuralWithScalarArguments {
                callee: producer_callee,
                returned_claim_transfers,
                ..
            }) = &producer.kind
            else {
                return Err(mismatch());
            };
            let source = module
                .machines
                .iter()
                .find(|machine| machine.id == *producer_callee)
                .ok_or_else(mismatch)?;
            let source_result = source.result.structural().ok_or_else(mismatch)?;
            for claim in &result.claims {
                let mut returned = returned_claim_transfers
                    .iter()
                    .filter(|transfer| transfer.caller_claim == claim.claim);
                let transfer = returned.next().ok_or_else(mismatch)?;
                if returned.next().is_some() {
                    return Err(mismatch());
                }
                let Some(binding) = source
                    .content_entry_claims
                    .iter()
                    .find(|binding| binding.claim == transfer.callee_claim)
                else {
                    continue;
                };
                // A returned permission identity alone does not establish a
                // content theorem. Rebase only the producer callee's exact
                // validated identity guarantee at its successful result.
                let mut identities = source
                    .content_identity_reshuffles
                    .iter()
                    .filter(|identity| identity.claim == transfer.callee_claim);
                let identity = identities.next().ok_or_else(mismatch)?;
                if identities.next().is_some() || identity.input != binding.input
                    || identity.projections != binding.projections
                    || identity.output.root != source_result.place
                    || identity.output.version != semantic_vocabulary::ContentPlaceVersion::Current
                    || identity.output.segments.len() != claim.path.len()
                    || identity.output.segments.iter().zip(&claim.path).any(|(content, structural)| !matches!((content, structural),
                        (semantic_vocabulary::ContentPlaceSegment::Field(left), StructuralPathSegment::Field(right)) if left == right)
                        && !matches!((content, structural),
                            (semantic_vocabulary::ContentPlaceSegment::FixedIndex(left), StructuralPathSegment::FixedIndex(right)) if left == right))
                {
                    return Err(mismatch());
                }
                caller_content.push((&identity.output.segments, &identity.projections));
            }
        }
        let mut callee_content = callee
            .content_entry_claims
            .iter()
            .filter(|binding| binding.input.root == parameter.place)
            .map(|binding| (&binding.input.segments, &binding.projections))
            .collect::<Vec<_>>();
        caller_content.sort();
        callee_content.sort();
        if caller_content != callee_content {
            return Err(ModuleError::UnitCallContentClaimMismatch {
                operation,
                argument_index: argument_index as u32,
            });
        }
    }
    let callee_claims = callee
        .entry_claims
        .iter()
        .map(|claim| (claim.claim, claim.input))
        .chain(
            callee
                .content_entry_claims
                .iter()
                .map(|claim| (claim.claim, claim.input.root)),
        )
        .collect::<BTreeMap<_, _>>();
    for (claim, input) in &callee_claims {
        if !callee
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == *input)
        {
            return Err(ModuleError::UnitCallClaimHasNoStructuralArgument {
                operation,
                claim: *claim,
            });
        }
    }
    if transfers.len() != callee_claims.len() {
        return Err(ModuleError::UnitCallClaimTransferCountMismatch {
            operation,
            expected: callee_claims.len(),
            actual: transfers.len(),
        });
    }
    let mut caller_claims = BTreeSet::new();
    for transfer in transfers {
        if !caller_claims.insert(transfer.claim) {
            return Err(ModuleError::DuplicateUnitCallClaimTransfer(operation));
        }
        let Some(argument) = arguments.get(transfer.argument_index as usize) else {
            return Err(ModuleError::ClaimActionArgumentOutOfRange {
                operation,
                argument_index: transfer.argument_index,
            });
        };
        let result_claim = linear_call_result(caller, argument.place).and_then(|(_, result)| {
            result
                .claims
                .iter()
                .find(|binding| binding.claim == transfer.claim)
                .map(|binding| (result.place, binding.path.as_slice()))
        });
        let Some((claim_input, claim_path)) =
            result_claim.or_else(|| claim_input(caller, transfer.claim))
        else {
            return Err(ModuleError::UnknownClaimAtOperation {
                operation,
                claim: transfer.claim,
            });
        };
        let target_place = callee
            .structural_parameters
            .get(transfer.argument_index as usize)
            .map(|parameter| parameter.place);
        let structural_path_matches = claim_path.starts_with(&argument.path)
            && callee.entry_claims.iter().any(|claim| {
                Some(claim.input) == target_place && claim.path == claim_path[argument.path.len()..]
            });
        let content_matches = argument.path.is_empty()
            && caller
                .content_entry_claims
                .iter()
                .any(|claim| claim.claim == transfer.claim && claim.input.root == argument.place)
            && callee
                .content_entry_claims
                .iter()
                .any(|claim| Some(claim.input.root) == target_place);
        if claim_input != argument.place || (!structural_path_matches && !content_matches) {
            return Err(ModuleError::ClaimActionPlaceMismatch {
                operation,
                claim: transfer.claim,
                argument_index: transfer.argument_index,
            });
        }
    }
    for input in callee_claims.into_values() {
        let argument_index = callee
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == input)
            .expect("callee entry claims were validated against its signature")
            as u32;
        if !transfers
            .iter()
            .any(|transfer| transfer.argument_index == argument_index)
        {
            return Err(ModuleError::MissingUnitCallClaimTransfer {
                operation,
                argument_index,
            });
        }
    }
    if transfers.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ModuleError::NonCanonicalUnitCallClaimTransfers(operation));
    }
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
