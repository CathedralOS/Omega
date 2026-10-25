//! Stage a complete owned subtree for a transaction that retains its custody.
//!
//! Image bytes cannot stand in for the original value: zero-byte children,
//! qualifications and cleanup obligations survive even when no bit represents
//! them. The constructor storage transaction snapshots the complete payload;
//! the ordinary transfer checks preserve its live claim and affine partition.
//! A nominal whole claim transports its children without exposing a separate
//! claim for each one. Destructive descendant accounting is a different operation.
//! Nothing here consumes the source or publishes a new owner. The caller must
//! commit source retirement, identity allocation and the receiving transaction
//! together. Retained custody uses relative paths, never a caller's ClaimIds.

use super::storage::{ConstructorStorage, ConstructorValue};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::TerminalExecution;
use crate::values::{StructuralRuntimePlace, TerminalStructuralValue};
use semantic_vocabulary::StructuralTypeId;
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralResultClaimBinding,
};

/// Detached payload and its complete semantic obligations. This is staging,
/// not a live owner, until the enclosing transaction consumes its source.
pub(crate) struct StagedOwnedValue {
    pub(crate) value: TerminalStructuralValue,
    pub(crate) storage: ConstructorStorage,
    pub(crate) claim_paths: Vec<Vec<StructuralPathSegment>>,
    pub(crate) affine_regions: Vec<OwnedAffineRegion>,
}

pub(crate) struct OwnedAffineRegion {
    pub(crate) path: Vec<StructuralPathSegment>,
    pub(crate) structural_type: StructuralTypeId,
}

impl TerminalExecution<'_> {
    pub(crate) fn stage_owned_value(
        &self,
        source: &StructuralArgument,
        structural_type: StructuralTypeId,
        multiplicity: StructuralMultiplicity,
        identity_cursor: &mut Option<u64>,
    ) -> Result<StagedOwnedValue, TerminalInterpretError> {
        if source.access != StructuralAccess::Owned {
            return Err(TerminalInterpretError::ClaimTransferMismatch);
        }
        let child = ConstructorValue::resolve(self, source)?;
        if child.structural_type() != structural_type {
            return Err(malformed());
        }
        if let ConstructorValue::Placed(value) = &child
            && crate::custody::has_live_loan(&self.reference_referents, value)
        {
            return Err(TerminalInterpretError::ContentTransformationMismatch(
                "owned value cannot transfer while its storage is borrowed",
            ));
        }
        let claims = crate::custody::projected_ownership_claims(&self.live_claims, source)?;
        let claim_paths = claims
            .iter()
            .map(|(_, claim)| claim.path.clone())
            .collect::<Vec<_>>();
        let (qualifications, projected_qualifications) = child.memberships();
        let retained = TerminalStructuralValue {
            opaque_identity: self
                .local_structural_identities
                .allocate_staged(identity_cursor)?,
            structural_type,
            qualifications: qualifications.to_vec(),
            projected_qualifications: projected_qualifications.to_vec(),
            path: Vec::new(),
        };
        let transfers_custody = !claims.is_empty()
            || self.live_affine_frontier.iter().any(|region| {
                region.place == source.place
                    && terminal_semantics::structural_paths_may_overlap(&region.path, &source.path)
            });
        if multiplicity != StructuralMultiplicity::Unrestricted && !transfers_custody {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        let mut storage = ConstructorStorage::new(self.account.as_ref());
        storage.stage(
            self,
            source,
            &child,
            &StructuralRuntimePlace::from(&retained),
            transfers_custody,
        )?;

        // Use the constructor's complete receiving partition as the expected
        // shape, then compare it to actual source debt. These temporary result
        // coordinates are never published and establish no new qualification.
        let expected = StructuralOperationResult {
            place: source.place,
            structural_type,
            multiplicity,
            qualifications: retained.qualifications.clone(),
            projected_qualifications: retained.projected_qualifications.clone(),
            qualification_establishments: Vec::new(),
            claims: claims
                .iter()
                .map(|(identity, claim)| StructuralResultClaimBinding {
                    claim: *identity,
                    path: claim.path.clone(),
                })
                .collect(),
        };
        let membership = |region: &StructuralAffineDiscard| {
            storage.case_at_projection(self, &retained, &region.path)
        };
        let mut frontier = BTreeSet::new();
        self.stage_result_affine_frontier(&expected, &mut frontier, &membership)?;
        self.validate_transferred_affine_complement(source, &expected, &frontier, &membership)?;
        let affine_regions = frontier
            .into_iter()
            .map(|region| OwnedAffineRegion {
                path: region.path,
                structural_type: region.structural_type,
            })
            .collect();

        Ok(StagedOwnedValue {
            value: retained,
            storage,
            claim_paths,
            affine_regions,
        })
    }
}
