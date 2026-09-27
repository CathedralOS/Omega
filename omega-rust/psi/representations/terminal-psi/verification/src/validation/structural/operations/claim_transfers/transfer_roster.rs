//! The callee's entry claims and the transfer roster that covers them.

use crate::validation::structural::operations::structural_arguments::linear_call_result;
use crate::validation::{
    BTreeMap, BTreeSet, ClaimTransfer, ModuleError, OperationId, StructuralArgument,
    TerminalMachine,
};
use semantic_vocabulary::{ClaimId, PlaceId};

use super::claim_input;

/// The callee's entry and content entry claims by claim id, each bound to
/// one of its structural parameters.
pub(in crate::validation) fn callee_claims(
    callee: &TerminalMachine,
    operation: OperationId,
) -> Result<BTreeMap<ClaimId, PlaceId>, ModuleError> {
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
    Ok(callee_claims)
}

/// The transfer roster covers the callee's claims exactly: one transfer per
/// claim, each naming a live caller claim once and an argument index the
/// claim's parameter occupies, in canonical order.
pub(in crate::validation) fn validate_transfer_roster(
    caller: &TerminalMachine,
    callee: &TerminalMachine,
    arguments: &[StructuralArgument],
    transfers: &[ClaimTransfer],
    operation: OperationId,
    callee_claims: BTreeMap<ClaimId, PlaceId>,
) -> Result<(), ModuleError> {
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
