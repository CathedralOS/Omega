//! A window separates the caller's storage debt from its detached value.
//! Owned projections transport the held claim identities and affine complement.
//! An exclusive loan does not import its parent's claims into the callee: those
//! remain on the suspended caller. Extraction establishes the checked result's
//! machine-local claims, and repair returns their custody to that exact loan.
//! Stage both frontiers before relocating cells so a failed join commits neither.
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{LiveClaim, TerminalExecution};
use crate::values::StructuralRuntimePlace;
use semantic_vocabulary::{ClaimId, PlaceId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment,
};

impl TerminalExecution<'_> {
    /// Return the owned root-relative projection, or no owned projection when
    /// the operation crosses an exclusive loan. Runtime address equality alone
    /// cannot convert the borrowed referent into this frame's owned storage.
    fn owned_window_projection(
        &self,
        root: PlaceId,
        path: &[StructuralPathSegment],
        hole: &StructuralRuntimePlace,
    ) -> Result<Option<StructuralArgument>, TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        if path.first() == Some(&StructuralPathSegment::Referent)
            || machine
                .structural_parameter(root)
                .is_some_and(|parameter| parameter.access == StructuralAccess::MutableBorrow)
        {
            return Ok(None);
        }
        let value = self.structural_values.get(&root).ok_or_else(malformed)?;
        if hole.opaque_identity != value.opaque_identity {
            return Err(malformed());
        }
        let path = hole
            .path
            .strip_prefix(value.path.as_slice())
            .ok_or_else(malformed)?
            .to_vec();
        Ok(Some(StructuralArgument {
            place: root,
            path,
            access: StructuralAccess::Owned,
        }))
    }

    pub(crate) fn stage_window_move_custody(
        &self,
        source: PlaceId,
        path: &[StructuralPathSegment],
        hole: &StructuralRuntimePlace,
        result: &StructuralOperationResult,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
    ) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
        if let Some(argument) = self.owned_window_projection(source, path, hole)? {
            let selected = self
                .live_claims
                .iter()
                .filter(|(_, claim)| {
                    claim.place == Some(source) && claim.path.starts_with(&argument.path)
                })
                .collect::<Vec<_>>();
            if selected.len() != result.claims.len()
                || result.claims.iter().any(|binding| {
                    !selected.iter().any(|(identity, claim)| {
                        **identity == binding.claim
                            && claim.path.strip_prefix(argument.path.as_slice())
                                == Some(binding.path.as_slice())
                    })
                })
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            // Copyable contents carry no custody: extracting an unrestricted
            // field of an owned value moves cells, never an affine complement
            // or a claim, so there is nothing to stage.
            if result.multiplicity == StructuralMultiplicity::Unrestricted
                && selected.is_empty()
                && !self.live_affine_frontier.iter().any(|entry| {
                    entry.place == source
                        && terminal_semantics::structural_paths_may_overlap(
                            &entry.path,
                            &argument.path,
                        )
                })
            {
                return Ok(self.live_claims.clone());
            }
            self.stage_constructor_operand(&argument, frontier, &mut Vec::new())?;
            return self.stage_constructor_claims(result, &[(&argument, Vec::new())], frontier);
        }
        let mut claims = self.live_claims.clone();
        for binding in &result.claims {
            super::bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
        }
        self.stage_result_affine_frontier(result, frontier)?;
        Ok(claims)
    }

    pub(crate) fn stage_window_store_claims(
        &self,
        destination: PlaceId,
        path: &[StructuralPathSegment],
        hole: &StructuralRuntimePlace,
        value: PlaceId,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
    ) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
        let destination = self.owned_window_projection(destination, path, hole)?;
        let mut claims = self.live_claims.clone();
        for (identity, claim) in &self.live_claims {
            if claim.place != Some(value) {
                continue;
            }
            claims.remove(identity);
            let Some(destination) = &destination else {
                // This frame's detached claim ends at repair. The exclusive
                // parent keeps its own machine-local claim namespace.
                continue;
            };
            let mut restored = claim.clone();
            restored.place = Some(destination.place);
            restored.path = destination
                .path
                .iter()
                .chain(&claim.path)
                .cloned()
                .collect();
            if claims.values().any(|held| {
                held.place == restored.place
                    && terminal_semantics::structural_paths_may_overlap(&held.path, &restored.path)
            }) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            if let Some(containing) = frontier
                .iter()
                .find(|entry| {
                    entry.place == destination.place && restored.path.starts_with(&entry.path)
                })
                .cloned()
            {
                frontier.remove(&containing);
                super::split_affine_frontier_at_projection(
                    &self.program.structural_types(),
                    frontier,
                    containing,
                    &restored.path,
                    &|_| None,
                )?;
            }
            claims.insert(*identity, restored);
        }
        Ok(claims)
    }
}
