//! Independent origin, provider-completion and ownership consistency.
use crate::{
    LegalizedCallSourceError, LegalizedScalarArgument, LegalizedScalarCall, NativeCallOrigin,
};
use optimization_unit::OwnershipEvent;
use terminal_psi::ClaimTransfer;
pub(super) fn validate(
    call: &LegalizedScalarCall,
    ownership: &[OwnershipEvent],
) -> Result<(), LegalizedCallSourceError> {
    match &call.source {
        NativeCallOrigin::Authored => {
            let claims = call
                .claim_transfers
                .iter()
                .map(|transfer| transfer.claim)
                .collect::<Vec<_>>();
            ((claims.is_empty() && ownership.is_empty())
                || ownership == [OwnershipEvent::ClaimTransfer(claims)])
            .then_some(())
            .ok_or(LegalizedCallSourceError::OwnershipMismatch)
        }
        NativeCallOrigin::InstalledProvider {
            boundary,
            provider,
            assumption,
            completion_claim_sources,
            completion_receipts,
        } => {
            if assumption
                .as_ref()
                .map_or(provider.boundary != *boundary, |row| {
                    !row.matches(*boundary, provider)
                })
                || call.target.machine() != Some(provider.candidate)
            {
                return Err(LegalizedCallSourceError::ProviderIdentityMismatch);
            }
            let structural_arguments = call
                .arguments
                .iter()
                .filter_map(|argument| match argument {
                    LegalizedScalarArgument::Structural {
                        semantic, target, ..
                    } => Some((semantic, target)),
                    LegalizedScalarArgument::Scalar { .. } => None,
                })
                .collect::<Vec<_>>();
            if provider.signature.parameters.len() != structural_arguments.len()
                || call
                    .arguments
                    .iter()
                    .skip_while(|argument| {
                        matches!(argument, LegalizedScalarArgument::Scalar { .. })
                    })
                    .any(|argument| matches!(argument, LegalizedScalarArgument::Scalar { .. }))
                || structural_arguments
                    .iter()
                    .zip(&provider.signature.parameters)
                    .any(|((semantic, target), parameter)| {
                        semantic.access != parameter.access
                            || target.place != semantic.place
                            || target.path != semantic.path
                            || target.access != parameter.access
                            || target.structural_type != parameter.structural_type
                    })
            {
                return Err(LegalizedCallSourceError::ArgumentSignatureMismatch);
            }
            validate_completion(
                call,
                ownership,
                completion_claim_sources,
                completion_receipts,
                structural_arguments.len(),
            )
        }
        NativeCallOrigin::TargetFamily {
            member,
            target,
            completion_claim_sources,
            completion_receipts,
            ..
        } => {
            if call.target.machine() != Some(*member) || target.is_empty() {
                return Err(LegalizedCallSourceError::ProviderIdentityMismatch);
            }
            let structural_count = call
                .arguments
                .iter()
                .filter(|argument| matches!(argument, LegalizedScalarArgument::Structural { .. }))
                .count();
            validate_completion(
                call,
                ownership,
                completion_claim_sources,
                completion_receipts,
                structural_count,
            )
        }
    }
}

fn validate_completion(
    call: &LegalizedScalarCall,
    ownership: &[OwnershipEvent],
    completion_claim_sources: &[abstract_operations::CompletionClaimSource],
    completion_receipts: &[terminal_psi::CompletionReceipt],
    structural_count: usize,
) -> Result<(), LegalizedCallSourceError> {
    let transfers = completion_receipts
        .iter()
        .map(|receipt| ClaimTransfer {
            claim: receipt.claim,
            argument_index: receipt.argument_index,
            path: receipt.path.clone(),
        })
        .collect::<Vec<_>>();
    let unique_source_claims = completion_claim_sources
        .iter()
        .map(|source| source.claim)
        .collect::<std::collections::BTreeSet<_>>();
    // Installation already joined these receipts to candidate entry claims.
    // Caller entry provenance describes historical ingress, not current
    // custody: a fresh local or transferred result need not have an entry row.
    // Preserve the installed transfers and ownership event instead of
    // reconstructing a second, narrower entry-only custody judgement.
    if unique_source_claims.len() != completion_claim_sources.len()
        || transfers != call.claim_transfers
        || completion_receipts
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || completion_receipts
            .iter()
            .any(|receipt| receipt.argument_index as usize >= structural_count)
    {
        return Err(LegalizedCallSourceError::CompletionEvidenceMismatch);
    }
    let completed = completion_receipts.to_vec();
    ((completed.is_empty() && ownership.is_empty())
        || ownership == [OwnershipEvent::ClaimCompletion(completed)])
    .then_some(())
    .ok_or(LegalizedCallSourceError::OwnershipMismatch)
}
