//! Optimizer module role: executable entrance. Representation-owned validation for retained legalized-operation values.

use crate::model::*;
use omega_optimization_unit::OwnershipEvent;
use psi_terminal::ClaimTransfer;

impl LegalizedCallUnit {
    /// Validate only invariants owned by this representation. Upstream source,
    /// target, optimizer, and installation replay remain independently
    /// required before construction.
    pub fn validate_source(&self) -> Result<(), LegalizedCallSourceError> {
        match &self.source {
            LegalizedCallUnitSource::AuthoredCallUnit => {
                let claims = self
                    .claim_transfers
                    .iter()
                    .map(|transfer| transfer.claim)
                    .collect::<Vec<_>>();
                (self.ownership == [OwnershipEvent::ClaimTransfer(claims)])
                    .then_some(())
                    .ok_or(LegalizedCallSourceError::OwnershipMismatch)
            }
            LegalizedCallUnitSource::InstalledProvider {
                boundary,
                provider,
                completion_claim_sources,
                completion_receipts,
            } => {
                if provider.boundary != *boundary || provider.candidate != self.callee {
                    return Err(LegalizedCallSourceError::ProviderIdentityMismatch);
                }
                if provider.signature.parameters.len() != self.arguments.len()
                    || self
                        .arguments
                        .iter()
                        .zip(&provider.signature.parameters)
                        .any(|(argument, parameter)| {
                            !argument.semantic.path.is_empty()
                                || argument.semantic.access != parameter.access
                                || argument.target.access != parameter.access
                                || argument.target.structural_type != parameter.structural_type
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
                    || transfers != self.claim_transfers
                    || completion_receipts
                        .windows(2)
                        .any(|pair| pair[0] >= pair[1])
                    || completion_receipts.iter().any(|receipt| {
                        let Some(argument) = self.arguments.get(receipt.argument_index as usize)
                        else {
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
                            || source.input() != argument.semantic.place
                            || source
                                .entry
                                .as_ref()
                                .is_some_and(|entry| entry.path != argument.semantic.path)
                    })
                {
                    return Err(LegalizedCallSourceError::CompletionEvidenceMismatch);
                }
                let completed = completion_receipts
                    .iter()
                    .map(|receipt| receipt.claim)
                    .collect::<Vec<_>>();
                (self.ownership == [OwnershipEvent::ClaimCompletion(completed)])
                    .then_some(())
                    .ok_or(LegalizedCallSourceError::OwnershipMismatch)
            }
        }
    }
}
