//! Constructor operands move their affine complement and linear claims together.
//! Stage custody before removing any runtime cells so malformed later operands
//! cannot consume an earlier child or its still-live siblings.
use crate::custody::{consume_affine_projection, resolve_structural_arguments};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{LiveClaim, TerminalExecution};
use crate::values::StructuralRuntimePlace;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralPathSegment,
};

impl TerminalExecution {
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
            } else {
                consume_affine_projection(
                    &self.program.structural_types,
                    &self.structural_values,
                    frontier,
                    argument,
                )?;
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
        result: &StructuralOperationResult,
        operands: &[(&StructuralArgument, Vec<StructuralPathSegment>)],
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
    ) -> Result<BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::ClaimTransferMismatch;
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
            match transported.remove(&binding.claim) {
                Some(path) if path == binding.path => {}
                None if result.multiplicity == StructuralMultiplicity::Linear
                    && result.claims.len() == 1
                    && binding.path.is_empty()
                    && !self.live_claims.contains_key(&binding.claim) => {}
                _ => return Err(invalid()),
            }
            if remaining
                .insert(
                    binding.claim,
                    LiveClaim {
                        place: Some(result.place),
                        path: binding.path.clone(),
                        multiplicity: Some(StructuralMultiplicity::Linear),
                    },
                )
                .is_some()
            {
                return Err(invalid());
            }
        }
        if !transported.is_empty() {
            return Err(invalid());
        }
        if result.multiplicity == StructuralMultiplicity::Affine {
            let root = StructuralAffineDiscard {
                place: result.place,
                path: Vec::new(),
                structural_type: result.structural_type,
            };
            if !frontier.insert(root) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            // Linear descendants have their own dispositions. Exclude them
            // from the affine complement instead of owning them twice.
            for claim in &result.claims {
                let containing = frontier
                    .iter()
                    .find(|entry| {
                        entry.place == result.place && claim.path.starts_with(&entry.path)
                    })
                    .cloned()
                    .ok_or_else(invalid)?;
                frontier.remove(&containing);
                crate::custody::split_affine_frontier_at_projection(
                    &self.program.structural_types,
                    frontier,
                    containing,
                    &claim.path,
                )?;
            }
        }
        Ok(remaining)
    }

    pub(crate) fn commit_constructor_operands(
        &mut self,
        frontier: BTreeSet<StructuralAffineDiscard>,
        consumed: &[StructuralArgument],
        claims: BTreeMap<semantic_vocabulary::ClaimId, LiveClaim>,
    ) -> Result<(), TerminalInterpretError> {
        // Resolve all selected homes before removing roots, including multiple
        // disjoint operands that consume the last siblings of one root.
        let homes = consumed
            .iter()
            .map(|argument| {
                if argument.path.is_empty() && self.scalar_case_values.contains_key(&argument.place)
                {
                    return Ok(None);
                }
                let child = resolve_structural_arguments(
                    &self.program.structural_types,
                    &self.structural_values,
                    std::slice::from_ref(argument),
                )?
                .pop()
                .ok_or(malformed())?;
                Ok(Some(StructuralRuntimePlace::from(&child)))
            })
            .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
        for (argument, home) in consumed.iter().zip(homes) {
            if let Some(home) = home {
                self.prune_subtree(&home);
            }
            if !frontier.iter().any(|entry| entry.place == argument.place)
                && !claims
                    .values()
                    .any(|claim| claim.place == Some(argument.place))
            {
                self.structural_values.remove(&argument.place);
                self.scalar_case_values.remove(&argument.place);
            }
        }
        self.live_affine_frontier = frontier;
        self.live_claims = claims;
        Ok(())
    }
}
