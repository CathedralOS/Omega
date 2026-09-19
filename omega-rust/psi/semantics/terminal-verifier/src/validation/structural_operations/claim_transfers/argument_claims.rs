//! One argument's claim correspondence with its callee parameter.

use crate::validation::structural_operations::structural_arguments::{
    is_unrestricted_mutable_subloan, is_unrestricted_shared_subloan,
    is_unrestricted_write_only_subloan, linear_call_result,
};
use crate::validation::{
    ModuleError, OperationId, OperationKind, StructuralArgument, StructuralMultiplicity,
    StructuralPathSegment, TerminalMachine, TerminalModule, is_partial_affine_path,
    partial_affine_root_type,
};
use terminal_psi::StructuralParameterDeclaration;

use super::is_record_loan;

/// One structural argument's claims against its callee parameter: a
/// projected argument carries no claims, and otherwise the caller's live
/// claim paths and content claims on the argument (or its linear call
/// result) must match the callee's exactly.
pub(super) fn validate_argument_claims(
    module: &TerminalModule,
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    operation: OperationId,
    argument_index: usize,
    argument: &StructuralArgument,
    parameter: &StructuralParameterDeclaration,
) -> Result<(), ModuleError> {
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
            is_unrestricted_mutable_subloan(module, caller, parameter, argument)
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
    Ok(())
}
