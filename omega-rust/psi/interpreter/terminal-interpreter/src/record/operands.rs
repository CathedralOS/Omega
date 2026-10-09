//! Constructor operands move their affine complement and linear claims together.
//! Stage custody before removing any runtime cells so malformed later operands
//! cannot consume an earlier child or its still-live siblings.
use crate::custody::resolve_single_structural_argument;
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{LiveClaim, TerminalExecution};
use crate::values::StructuralRuntimePlace;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity, StructuralOperationResult,
    StructuralPathSegment,
};

impl TerminalExecution<'_> {
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
                self.consume_affine_projection(frontier, argument)?;
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

    /// Stage `argument` and reject custody transfer into an unrestricted
    /// result: only affine or claimed operands may hand custody over.
    pub(crate) fn stage_unrestricted_operand(
        &self,
        argument: &StructuralArgument,
        frontier: &mut BTreeSet<StructuralAffineDiscard>,
        consumed: &mut Vec<StructuralArgument>,
        result: &StructuralOperationResult,
    ) -> Result<bool, TerminalInterpretError> {
        let transfers_custody = self.stage_constructor_operand(argument, frontier, consumed)?;
        if transfers_custody && result.multiplicity == StructuralMultiplicity::Unrestricted {
            return Err(malformed());
        }
        Ok(transfers_custody)
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
        case_membership: &impl Fn(
            &StructuralAffineDiscard,
        ) -> Option<semantic_vocabulary::StructuralCaseId>,
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
        self.stage_result_affine_frontier(result, frontier, case_membership)?;
        Ok(remaining)
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
                result
                    .claims
                    .iter()
                    .map(|claim| (result.place, claim.path.as_slice())),
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
        // Resolve all selected homes before removing roots, including multiple
        // disjoint operands that consume the last siblings of one root.
        let homes = consumed
            .iter()
            .map(|argument| {
                let child = resolve_single_structural_argument(
                    &self.program.structural_types(),
                    &self.structural_values,
                    &argument,
                )?;
                Ok(StructuralRuntimePlace::from(&child))
            })
            .collect::<Result<Vec<_>, TerminalInterpretError>>()?;
        for (argument, home) in consumed.iter().zip(homes) {
            self.prune_subtree(&home);
            // Only a whole transfer consumes the source binding. Empty
            // resource custody after a partial transfer does not erase the
            // initialized storage of disjoint scalar siblings. Frame completion
            // retires that binding after its checked cleanup schedule finishes.
            if argument.path.is_empty() {
                self.structural_values.remove(&argument.place);
            }
        }
        self.live_affine_frontier = frontier;
        self.live_claims = claims;
        Ok(())
    }
}
