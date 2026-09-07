//! Independent origin, provider-completion and ownership consistency.
use crate::{
    LegalizedCallSourceError, LegalizedCallUnitSource, LegalizedScalarArgument, LegalizedScalarCall,
};
use optimization_unit::OwnershipEvent;
use terminal_psi::ClaimTransfer;
pub(super) fn validate(
    call: &LegalizedScalarCall,
    ownership: &[OwnershipEvent],
) -> Result<(), LegalizedCallSourceError> {
    match &call.source {
        LegalizedCallUnitSource::AuthoredCallUnit => {
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
        LegalizedCallUnitSource::InstalledProvider {
            boundary,
            provider,
            completion_claim_sources,
            completion_receipts,
        } => {
            if provider.boundary != *boundary || provider.candidate != call.callee {
                return Err(LegalizedCallSourceError::ProviderIdentityMismatch);
            }
            if provider.signature.parameters.len() != call.arguments.len()
                || call
                    .arguments
                    .iter()
                    .zip(&provider.signature.parameters)
                    .any(|(argument, parameter)| {
                        let LegalizedScalarArgument::Structural { semantic, target } = argument
                        else {
                            return true;
                        };
                        !semantic.path.is_empty()
                            || semantic.access != parameter.access
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
                })
                .collect::<Vec<_>>();
            let unique_source_claims = completion_claim_sources
                .iter()
                .map(|source| source.claim)
                .collect::<std::collections::BTreeSet<_>>();
            if unique_source_claims.len() != completion_claim_sources.len()
                || transfers != call.claim_transfers
                || completion_receipts
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
                || completion_receipts.iter().any(|receipt| {
                    let Some(argument) = call.arguments.get(receipt.argument_index as usize) else {
                        return true;
                    };
                    let LegalizedScalarArgument::Structural { semantic, .. } = argument else {
                        return true;
                    };
                    let matching = completion_claim_sources
                        .iter()
                        .filter(|source| source.claim == receipt.claim)
                        .collect::<Vec<_>>();
                    let [source] = matching.as_slice() else {
                        return true;
                    };
                    (source.entry.is_none() && source.content.is_none())
                        || source.input() != semantic.place
                        || source
                            .entry
                            .as_ref()
                            .is_some_and(|entry| entry.path != semantic.path)
                })
            {
                return Err(LegalizedCallSourceError::CompletionEvidenceMismatch);
            }
            let completed = completion_receipts
                .iter()
                .map(|receipt| receipt.claim)
                .collect::<Vec<_>>();
            (ownership == [OwnershipEvent::ClaimCompletion(completed)])
                .then_some(())
                .ok_or(LegalizedCallSourceError::OwnershipMismatch)
        }
    }
}
