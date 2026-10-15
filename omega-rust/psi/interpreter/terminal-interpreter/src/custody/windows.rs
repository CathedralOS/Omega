//! A window separates the caller's storage debt from its detached value.
//! Owned projections transport the held claim identities and affine complement.
//! An exclusive loan does not import its parent's claims into the callee: those
//! remain on the suspended caller. Extraction establishes the checked result's
//! machine-local claims, and repair returns their custody to that exact loan.
//! Stage both frontiers before relocating cells so a failed join commits neither.
//! Normal return joins the caller's checked region catalogue against stored case
//! membership. Local extraction cannot manufacture the caller's claim identities,
//! and refreshing one region cannot re-establish disjoint moved siblings.
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
    /// Capture the normal-return catalogue of the exact operation. Ordinary
    /// calls save it with their frame; host boundaries join after writeback.
    pub(crate) fn loan_completions_for_operation(
        &self,
        operation: semantic_vocabulary::OperationId,
    ) -> Result<Vec<terminal_psi::LoanCompletion>, TerminalInterpretError> {
        let machine = self.program.verified_machine(self.current_machine)?;
        Ok(machine
            .loan_completions
            .iter()
            .filter(|completion| completion.operation == operation)
            .cloned()
            .collect())
    }
    /// The callee closed every window before its normal return. Rejoin the
    /// caller's checked namespace at the exact returned region, selecting
    /// actual guards rather than renaming a stale payload claim. Stage the
    /// entire roster so an invalid stored tag or duplicate claim publishes
    /// neither half a join nor custody at an unrelated sibling.
    pub(crate) fn complete_loan_custody(
        &mut self,
        completions: &[terminal_psi::LoanCompletion],
    ) -> Result<(), TerminalInterpretError> {
        let mut claims = self.live_claims.clone();
        let mut frontier = self.live_affine_frontier.clone();
        for completion in completions {
            let owner = crate::custody::verified_structural_place(
                &self.structural_values,
                completion.place,
            )?;
            // Authenticate the captured region even when its active case has
            // no linear claims. Stale bytes are never a restored owner.
            let returned = crate::custody::project_structural_value(
                &self.program.structural_types(),
                owner,
                &completion.path,
            )?;
            if completion
                .claims
                .iter()
                .any(|binding| !binding.path.starts_with(&completion.path))
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            claims.retain(|_, claim| {
                claim.place != Some(completion.place) || !claim.path.starts_with(&completion.path)
            });
            let active_claims = self.active_claims_in_value(owner, &completion.claims)?;
            for binding in &active_claims {
                if claims
                    .insert(
                        binding.claim,
                        LiveClaim {
                            place: Some(completion.place),
                            path: binding.path.clone(),
                            multiplicity: Some(StructuralMultiplicity::Linear),
                        },
                    )
                    .is_some()
                {
                    return Err(TerminalInterpretError::ClaimTransferMismatch);
                }
            }
            let case_membership = |current: &StructuralAffineDiscard| {
                crate::structural_inputs::case_membership::stored_case_at_projection(
                    &self.program.structural_types(),
                    self.structural_values.get(&current.place)?,
                    &self.structural_cases,
                    &current.path,
                )
            };
            // Partition a containing residual before replacing the region.
            // Disjoint moved siblings must not be re-established by rebuilding
            // the containing root's entire frontier from its type.
            if let Some(containing) = frontier
                .iter()
                .find(|entry| {
                    entry.place == completion.place
                        && entry.path != completion.path
                        && completion.path.starts_with(&entry.path)
                })
                .cloned()
            {
                frontier.remove(&containing);
                crate::custody::split_affine_frontier_at_projection(
                    &self.program.structural_types(),
                    &mut frontier,
                    containing,
                    &completion.path,
                    &case_membership,
                )?;
            }
            frontier.retain(|entry| {
                entry.place != completion.place || !entry.path.starts_with(&completion.path)
            });
            if completion.multiplicity == StructuralMultiplicity::Affine {
                frontier.insert(StructuralAffineDiscard {
                    place: completion.place,
                    path: completion.path.clone(),
                    structural_type: returned.structural_type,
                });
                crate::custody::exclude_linear_regions(
                    &self.program.structural_types(),
                    &mut frontier,
                    active_claims
                        .iter()
                        .map(|claim| (completion.place, claim.path.as_slice())),
                    &case_membership,
                )?;
            }
        }
        self.live_claims = claims;
        self.live_affine_frontier = frontier;
        Ok(())
    }

    /// Return the owned root-relative projection, or no owned projection when
    /// the operation crosses an exclusive loan, including a mutable element
    /// view's loan of its collection. Runtime address equality alone cannot
    /// convert the borrowed referent into this frame's owned storage.
    pub(super) fn owned_window_projection(
        &self,
        root: PlaceId,
        value: &crate::values::TerminalStructuralValue,
        path: &[StructuralPathSegment],
        hole: &StructuralRuntimePlace,
    ) -> Result<Option<StructuralArgument>, TerminalInterpretError> {
        let machine = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or_else(malformed)?;
        if path.contains(&StructuralPathSegment::Referent)
            || self.element_view_values.contains_key(&root)
            || machine
                .structural_parameter(root)
                .is_some_and(|parameter| parameter.access == StructuralAccess::MutableBorrow)
        {
            return Ok(None);
        }
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
        // Extraction retains the selected backing. Its tags are already live,
        // even though the result place has not been published yet.
        let selected = crate::values::TerminalStructuralValue {
            opaque_identity: hole.opaque_identity,
            structural_type: result.structural_type,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            path: hole.path.clone(),
        };
        // A sum's result catalogue includes alternative payloads. Establish
        // custody and exclude affine debt only for the actual extracted value.
        let mut active_result = result.clone();
        active_result.claims = self.active_claims_in_value(&selected, &result.claims)?;
        let result = &active_result;
        let case_membership = |current: &StructuralAffineDiscard| {
            crate::structural_inputs::case_membership::stored_case_at_projection(
                &self.program.structural_types(),
                &selected,
                &self.structural_cases,
                &current.path,
            )
        };
        let storage = self.structural_values.get(&source).ok_or_else(malformed)?;
        if let Some(argument) = self.owned_window_projection(source, storage, path, hole)? {
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
            return self.stage_constructor_claims(
                result,
                &[(&argument, Vec::new())],
                frontier,
                &case_membership,
            );
        }
        let mut claims = self.live_claims.clone();
        for binding in &result.claims {
            super::bind_result_claim(&mut claims, binding, result.place, result.multiplicity)?;
        }
        self.stage_result_affine_frontier(result, frontier, &case_membership)?;
        Ok(claims)
    }

    pub(crate) fn stage_window_store_claims(
        &self,
        destination: PlaceId,
        storage: &crate::values::TerminalStructuralValue,
        path: &[StructuralPathSegment],
        hole: &StructuralRuntimePlace,
        value: PlaceId,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
    ) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
        let destination = self.owned_window_projection(destination, storage, path, hole)?;
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
