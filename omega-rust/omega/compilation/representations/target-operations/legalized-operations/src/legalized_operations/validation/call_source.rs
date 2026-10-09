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
            completion_claim_sources,
            completion_receipts,
        } => {
            if provider.boundary != *boundary || call.target.machine() != Some(provider.candidate) {
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
            let transfers = completion_receipts
                .iter()
                .map(|receipt| ClaimTransfer {
                    claim: receipt.claim,
                    argument_index: receipt.argument_index,
                    path: Vec::new(),
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
                    .any(|receipt| receipt.argument_index as usize >= structural_arguments.len())
            {
                return Err(LegalizedCallSourceError::CompletionEvidenceMismatch);
            }
            let completed = completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect::<Vec<_>>();
            ((completed.is_empty() && ownership.is_empty())
                || ownership == [OwnershipEvent::ClaimCompletion(completed)])
            .then_some(())
            .ok_or(LegalizedCallSourceError::OwnershipMismatch)
        }
    }
}
