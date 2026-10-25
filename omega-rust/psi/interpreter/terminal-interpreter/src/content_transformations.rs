//! Stage the parent side of an explicit content transaction. The ordinary
//! owned-value transaction transports the raw payload separately. Nothing is
//! removed here: every current subject, qualification and nested claim must
//! agree before the caller commits any custody or storage mutation.
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::record::storage::ConstructorValue;
use semantic_vocabulary::ClaimId;
use terminal_psi::{ContentTransformationDerivation, StructuralArgument};

impl TerminalExecution<'_> {
    pub(crate) fn content_parent_claims(
        &self,
        parents: &[StructuralArgument],
        derivations: &[ContentTransformationDerivation],
    ) -> Result<Vec<ClaimId>, TerminalInterpretError> {
        let invalid = || TerminalInterpretError::ClaimTransferMismatch;
        if parents.is_empty() && derivations.is_empty() {
            return Ok(Vec::new());
        }
        if parents.is_empty() || derivations.is_empty() {
            return Err(invalid());
        }
        let module = &self.program.catalogs;
        let mut expected = Vec::new();
        for derived in derivations {
            for input in &derived.inputs {
                let root = self
                    .structural_values
                    .get(&input.subject.root)
                    .ok_or_else(invalid)?;
                let path = terminal_semantics::content_transformations::structural_path(
                    module.structural_types.iter(),
                    &module.identities,
                    root.structural_type,
                    &input.subject.segments,
                )
                .ok_or_else(invalid)?;
                let claim = self.live_claims.get(&input.claim).ok_or_else(invalid)?;
                if claim.place != Some(input.subject.root)
                    || claim.path != path
                    || !parents.iter().any(|parent| {
                        parent.place == input.subject.root && path.starts_with(&parent.path)
                    })
                {
                    return Err(invalid());
                }
                if !expected.contains(&input.claim) {
                    expected.push(input.claim);
                }
            }
        }
        let mut actual = Vec::new();
        let types = module
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect::<std::collections::BTreeMap<_, _>>();
        let boundaries = module
            .structural_types
            .iter()
            .filter_map(|declaration| declaration.owns_linear_claim.then_some(declaration.id))
            .chain(
                module
                    .machines
                    .iter()
                    .filter_map(|machine| machine.nominal_cleanup_subject),
            )
            .collect::<std::collections::BTreeSet<_>>();
        for parent in parents {
            if parent.access != terminal_psi::StructuralAccess::Owned {
                return Err(invalid());
            }
            let selected = ConstructorValue::resolve(self, parent)?;
            if let ConstructorValue::Placed(value) = &selected
                && (crate::custody::has_live_loan(self.reference_referents.as_map(), value)
                    || self.reference_referents.keys().any(|carrier| {
                        carrier.opaque_identity == value.opaque_identity
                            && terminal_semantics::structural_paths_may_overlap(
                                &carrier.path,
                                &value.path,
                            )
                    }))
            {
                return Err(TerminalInterpretError::ContentTransformationMismatch(
                    "content parent retains a live loan or resident reference descriptor",
                ));
            }
            let claims = crate::custody::projected_ownership_claims(&self.live_claims, parent)?;
            if claims.is_empty() {
                return Err(invalid());
            }
            let root = self
                .structural_values
                .get(&parent.place)
                .ok_or_else(invalid)?;
            let accounted = expected
                .iter()
                .filter_map(|identity| self.live_claims.get(identity))
                .filter(|claim| claim.place == Some(parent.place))
                .map(|claim| claim.path.clone())
                .collect::<Vec<_>>();
            terminal_semantics::owned_structure::validate_accounted_descendants(
                &types,
                &boundaries,
                selected.structural_type(),
                &parent.path,
                &accounted,
                &|path, case| {
                    crate::structural_inputs::case_membership::stored_case_at_projection(
                        &self.program.structural_types(),
                        root,
                        self.structural_cases.as_map(),
                        path,
                    ) == Some(case)
                },
            )
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
            for (identity, _) in &claims {
                if actual.contains(identity) {
                    return Err(invalid());
                }
                actual.push(*identity);
            }
            // No nominal cleanup or affine complement may vanish merely
            // because a sibling claim has a content equation.
            if self.live_affine_frontier.iter().any(|entry| {
                entry.place == parent.place
                    && terminal_semantics::structural_paths_may_overlap(&entry.path, &parent.path)
            }) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            let (whole, projected) = selected.memberships();
            let root = self
                .structural_values
                .get(&parent.place)
                .ok_or_else(invalid)?;
            terminal_semantics::content_transformations::validate_parent_memberships(
                &module.structural_types,
                &module.structural_domains,
                &module.identities,
                root.structural_type,
                parent,
                derivations,
                whole,
                projected,
                &|relative| {
                    let mut path = parent.path.clone();
                    for segment in relative {
                        if let terminal_psi::StructuralPathSegment::Case(case) = segment
                            && crate::structural_inputs::case_membership::stored_case_at_projection(
                                &self.program.structural_types(),
                                root,
                                self.structural_cases.as_map(),
                                &path,
                            )
                            .is_some_and(|selected| selected != *case)
                        {
                            return true;
                        }
                        path.push(segment.clone());
                    }
                    false
                },
            )
            .map_err(TerminalInterpretError::ContentTransformationMismatch)?;
        }
        expected.sort();
        actual.sort();
        if expected != actual {
            return Err(invalid());
        }
        Ok(expected)
    }
}
