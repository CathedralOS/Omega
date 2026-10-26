//! Constructor operands move their affine complement and linear claims together.
//! Stage custody before removing any runtime cells so malformed later operands
//! cannot consume an earlier child or its still-live siblings.
use crate::custody::resolve_single_structural_argument;
use crate::errors::TerminalInterpretError;
use crate::execution::{LiveClaim, TerminalExecution};
use crate::values::StructuralRuntimePlace;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralPathSegment,
};

/// A new value may own an authored establishment; transporting a value must
/// continue the original identities, including case-guarded descendants.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResultClaimOrigin {
    Construction,
    Transfer,
}

/// Resolved retirement homes and the complete receiving custody partition.
/// Preparation is fallible; committing this transaction performs no lookup
/// that can refuse after another part of a compound operation has committed.
pub(crate) struct PreparedConstructorOperands {
    homes: Vec<(StructuralArgument, StructuralRuntimePlace)>,
    frontier: BTreeSet<StructuralAffineDiscard>,
    claims: BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>,
}

impl PreparedConstructorOperands {
    pub(crate) fn commit(self, execution: &mut TerminalExecution<'_>) {
        for (argument, home) in self.homes {
            execution.prune_subtree(&home);
            // Partial transfers retain the initialized storage of disjoint
            // siblings, even when the parent's resource custody becomes empty.
            if argument.path.is_empty() {
                execution.structural_values.remove(&argument.place);
            }
        }
        execution.live_affine_frontier = self.frontier;
        execution.live_claims = self.claims;
    }
}

impl TerminalExecution<'_> {
    /// Compare an owned transfer's actual affine complement with its declared
    /// result, without reconstructing missing source children. The source
    /// classifier decides the selected multiplicity; this ordered check keeps
    /// that declaration from erasing debt or reviving a partially moved value.
    pub(crate) fn validate_transferred_affine_complement(
        &self,
        source: &StructuralArgument,
        result: &StructuralOperationResult,
        result_frontier: &BTreeSet<StructuralAffineDiscard>,
        result_case: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
    ) -> Result<(), TerminalInterpretError> {
        let types = self.program.structural_types();
        // A nominal linear claim cannot be divided by projecting its fields.
        for (_, claim) in crate::custody::projected_ownership_claims(&self.live_claims, source)? {
            if claim.path.is_empty() && claim.multiplicity != Some(result.multiplicity) {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
        }
        let mut actual = self.live_affine_frontier.clone();
        if let Some(containing) = actual
            .iter()
            .find(|entry| {
                entry.place == source.place
                    && source.path.starts_with(&entry.path)
                    && entry.path != source.path
            })
            .cloned()
        {
            actual.remove(&containing);
            crate::custody::split_affine_frontier_at_projection(
                &types,
                &mut actual,
                containing,
                &source.path,
                &|current| {
                    crate::structural_inputs::case_membership::stored_case_at_projection(
                        &types,
                        self.structural_values.get(&current.place)?,
                        self.structural_cases.as_map(),
                        &current.path,
                    )
                },
            )?;
            actual.insert(StructuralAffineDiscard {
                place: source.place,
                path: source.path.clone(),
                structural_type: result.structural_type,
            });
        }
        let mut remaining = result_frontier
            .iter()
            .filter(|entry| entry.place == result.place)
            .cloned()
            .collect::<BTreeSet<_>>();
        for entry in actual
            .iter()
            .filter(|entry| entry.place == source.place && entry.path.starts_with(&source.path))
        {
            let path = &entry.path[source.path.len()..];
            let containing = remaining
                .iter()
                .find(|expected| path.starts_with(&expected.path))
                .cloned()
                .ok_or(TerminalInterpretError::AffineFrontierMismatch)?;
            let selected =
                crate::custody::resolve_structural_path_type(&types, result.structural_type, path)?;
            if selected != entry.structural_type {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            remaining.remove(&containing);
            crate::custody::split_affine_frontier_at_projection(
                &types,
                &mut remaining,
                containing,
                path,
                result_case,
            )?;
        }
        if !remaining.is_empty() {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        Ok(())
    }

    pub(crate) fn stage_constructor_operand(
        &self,
        argument: &StructuralArgument,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        consumed: &mut Vec<StructuralArgument>,
    ) -> Result<bool, TerminalInterpretError> {
        let affine = self.live_affine_frontier.iter().any(|entry| {
            entry.place == argument.place
                && terminal_semantics::structural_paths_may_overlap(&entry.path, &argument.path)
        });
        if affine {
            if argument.path.is_empty() {
                // A complete claimed owner has a residual affine complement,
                // not an overlapping whole-root debt. Moving it transfers all
                // those rows along with the independently staged linear claims.
                let before = frontier.len();
                frontier.retain(|entry| entry.place != argument.place);
                if frontier.len() == before {
                    return Err(TerminalInterpretError::AffineFrontierMismatch);
                }
            } else if !crate::custody::consume_projected_affine_complement(
                &self.program.structural_types(),
                &self.structural_values,
                self.structural_cases.as_map(),
                frontier,
                argument,
            )? {
                // The original frontier identified custody, so an earlier
                // staged operand must not consume it a second time.
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            consumed.push(argument.clone());
        }
        let claimed = self.live_claims.values().any(|claim| {
            claim.place == Some(argument.place) && claim.path.starts_with(&argument.path)
        });
        if claimed {
            if consumed.iter().any(|previous| {
                previous.place == argument.place
                    && (previous.path.starts_with(&argument.path)
                        || argument.path.starts_with(&previous.path))
            }) && !affine
            {
                return Err(TerminalInterpretError::ClaimTransferMismatch);
            }
            if !affine {
                consumed.push(argument.clone());
            }
        } else if !affine && !argument.path.is_empty() {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        Ok(affine || claimed)
    }

    /// Stage the constructor's ownership transaction beside its staged bytes.
    /// A child keeps its identity when its path is rebased into the parent.
    /// Only an authored whole linear establishment can introduce a new claim;
    /// projected claims must already belong to an exact consumed operand.
    pub(crate) fn stage_constructor_claims(
        &self,
        origin: ResultClaimOrigin,
        result: &StructuralOperationResult,
        operands: &[(&StructuralArgument, Vec<StructuralPathSegment>)],
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        case_membership: &impl Fn(
            &StructuralAffineDiscard,
        ) -> Option<semantic_vocabulary::StructuralCaseId>,
    ) -> Result<BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::ClaimTransferMismatch;
        // Only an admitted nominal data construction can establish a distinct
        // whole-owner obligation. Arrays transport their element claims; a
        // result label alone cannot turn their frontier into a fresh root.
        let nominal_root = match result.claims.as_slice() {
            [binding]
                if origin == ResultClaimOrigin::Construction
                    && result.multiplicity == StructuralMultiplicity::Linear
                    && binding.path.is_empty()
                    && !self.live_claims.contains_key(&binding.claim)
                    && self
                        .program
                        .structural_types()
                        .get(&result.structural_type)
                        .is_some_and(|declaration| {
                            matches!(
                                declaration.shape,
                                terminal_psi::StructuralTypeShape::Record { .. }
                                    | terminal_psi::StructuralTypeShape::Sum { .. }
                                    | terminal_psi::StructuralTypeShape::Mixed { .. }
                            )
                        }) =>
            {
                Some(binding.claim)
            }
            _ => None,
        };
        let mut remaining = self.live_claims.clone();
        let mut transported = BTreeMap::new();
        for (argument, destination) in operands {
            for (identity, claim) in &self.live_claims {
                if claim.place != Some(argument.place) {
                    continue;
                }
                let Some(suffix) = claim.path.strip_prefix(argument.path.as_slice()) else {
                    continue;
                };
                let mut path = destination.clone();
                path.extend_from_slice(suffix);
                if transported.insert(*identity, path).is_some()
                    || remaining.remove(identity).is_none()
                {
                    return Err(invalid());
                }
            }
        }
        for binding in &result.claims {
            if !self.constructor_claim_is_active(result, &binding.path, case_membership)? {
                // A catalogue row can describe a nonresident payload, but a
                // live operand claim cannot disappear behind that guard.
                if transported.contains_key(&binding.claim)
                    || self.live_claims.contains_key(&binding.claim)
                {
                    return Err(invalid());
                }
                continue;
            }
            match transported.remove(&binding.claim) {
                Some(path) if path == binding.path => {}
                None if nominal_root == Some(binding.claim) => {}
                _ => return Err(invalid()),
            }
            if remaining
                .insert(
                    binding.claim,
                    LiveClaim::at(result.place, binding.path.clone(), result.multiplicity),
                )
                .is_some()
            {
                return Err(invalid());
            }
        }
        for path in transported.values() {
            // Each child was consumed once from an actual owned operand above.
            // A nominal root encapsulates that exact active subtree and its
            // retained storage/cleanup; it neither renames the child as the
            // parent nor publishes overlapping root and descendant claims.
            // Transparent construction must instead match every child identity
            // and typed destination path in the result roster.
            if nominal_root.is_none()
                || crate::custody::resolve_structural_path_type(
                    &self.program.structural_types(),
                    result.structural_type,
                    path,
                )
                .is_err()
                || !self.constructor_claim_is_active(result, path, case_membership)?
            {
                return Err(invalid());
            }
        }
        self.stage_result_affine_frontier(result, frontier, case_membership)?;
        Ok(remaining)
    }

    pub(crate) fn constructor_claim_is_active(
        &self,
        result: &StructuralOperationResult,
        path: &[StructuralPathSegment],
        membership: &impl Fn(&StructuralAffineDiscard) -> Option<semantic_vocabulary::StructuralCaseId>,
    ) -> Result<bool, TerminalInterpretError> {
        let types = self.program.structural_types();
        crate::structural_inputs::case_membership::case_projection_with_membership_is_active(
            &types,
            result.structural_type,
            path,
            &|prefix| {
                let structural_type = crate::custody::resolve_structural_path_type(
                    &types,
                    result.structural_type,
                    prefix,
                )
                .ok()?;
                membership(&StructuralAffineDiscard {
                    place: result.place,
                    path: prefix.to_vec(),
                    structural_type,
                })
            },
        )
    }

    /// Establish only the affine complement of a completed value. Constructors
    /// and borrowed extractions have the same result partition: each declared
    /// linear descendant owns its own claim, never an overlapping affine debt.
    pub(crate) fn stage_result_affine_frontier(
        &self,
        result: &StructuralOperationResult,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        case_membership: &impl Fn(
            &StructuralAffineDiscard,
        ) -> Option<semantic_vocabulary::StructuralCaseId>,
    ) -> Result<(), TerminalInterpretError> {
        if result.multiplicity == StructuralMultiplicity::Affine {
            let mut active_claims = Vec::new();
            for claim in &result.claims {
                if self.constructor_claim_is_active(result, &claim.path, case_membership)? {
                    active_claims.push((result.place, claim.path.as_slice()));
                }
            }
            let root = StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            };
            if !frontier.insert(root) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            crate::custody::exclude_linear_regions(
                &self.program.structural_types(),
                frontier,
                active_claims,
                case_membership,
            )?;
        }
        Ok(())
    }

    pub(crate) fn commit_constructor_operands(
        &mut self,
        frontier: BTreeSet<StructuralAffineDiscard>,
        consumed: &[StructuralArgument],
        claims: BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>,
    ) -> Result<(), TerminalInterpretError> {
        self.prepare_constructor_operands(frontier, consumed, claims)?
            .commit(self);
        Ok(())
    }

    pub(crate) fn prepare_constructor_operands(
        &self,
        frontier: BTreeSet<StructuralAffineDiscard>,
        consumed: &[StructuralArgument],
        claims: BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>,
    ) -> Result<PreparedConstructorOperands, TerminalInterpretError> {
        // Resolve all selected homes before removing roots, including multiple
        // disjoint operands that consume the last siblings of one root.
        let homes = consumed
            .iter()
            .map(|argument| {
                let child = resolve_single_structural_argument(
                    &self.program.structural_types(),
                    &self.structural_values,
                    argument,
                )?;
                Ok((argument.clone(), StructuralRuntimePlace::from(&child)))
            })
            .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
        Ok(PreparedConstructorOperands {
            homes,
            frontier,
            claims,
        })
    }
}
