//! Preserve producer-authored custody when publishing a completed source value.

use super::{
    Emission, LoweringError, OperationBuffer, PlaceId, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, unsupported,
};

impl Emission<'_, '_, '_> {
    /// A private value edge changes storage identity, not membership.
    /// Carry the actual source declaration into its new parameter instead of
    /// erasing predicates or treating a receiver annotation as evidence.
    pub(super) fn transported_parameter(
        &self,
        source: &StructuralArgument,
        place: PlaceId,
        position: u32,
        structural_type: semantic_vocabulary::StructuralTypeId,
        multiplicity: StructuralMultiplicity,
    ) -> Result<StructuralParameterDeclaration, LoweringError> {
        let (qualifications, projected_qualifications) = self
            .completed_memberships(source.place)
            .ok_or(LoweringError::Unsupported(
            "owned transport has no source membership declaration",
        ))?;
        let (qualifications, projected_qualifications) =
            terminal_psi::StructuralPathQualification::select(
                qualifications,
                projected_qualifications,
                &source.path,
            );
        Ok(StructuralParameterDeclaration {
            place,
            position,
            structural_type,
            multiplicity,
            access: StructuralAccess::Owned,
            qualifications,
            projected_qualifications,
        })
    }

    /// Producer and ingress homes retain their actual occurrence-relative
    /// memberships. A receiving annotation is never a source contract.
    pub(super) fn completed_memberships(
        &self,
        place: PlaceId,
    ) -> Option<(
        &[semantic_vocabulary::StructuralDomainId],
        &[terminal_psi::StructuralPathQualification],
    )> {
        self.operations
            .iter()
            .filter_map(|operation| operation.result.structural())
            .find(|result| result.place == place)
            .map(|result| {
                (
                    result.qualifications.as_slice(),
                    result.projected_qualifications.as_slice(),
                )
            })
            .or_else(|| {
                // A literal view's establishment names its place and the
                // memberships it was established with.
                self.operations
                    .iter()
                    .find_map(|operation| match &operation.kind {
                        terminal_psi::OperationKind::EstablishByteSequenceLiteral {
                            destination,
                            qualifications,
                            ..
                        } if *destination == place => Some((qualifications.as_slice(), &[][..])),
                        _ => None,
                    })
            })
            .or_else(|| {
                self.evaluation
                    .structural_root_parameters()
                    .find(|parameter| parameter.place == place)
                    .map(|parameter| {
                        (
                            parameter.qualifications.as_slice(),
                            parameter.projected_qualifications.as_slice(),
                        )
                    })
            })
    }

    /// The Terminal domains of an occurrence's checked memberships on this
    /// emission's carrier.
    pub(super) fn checked_membership_domains(
        &self,
        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
    ) -> Result<Vec<semantic_vocabulary::StructuralDomainId>, LoweringError> {
        let mut domains = self
            .checked
            .facts
            .qualifications
            .value_formations
            .span(decisions)
            .ok_or(LoweringError::Unsupported(
                "operand membership decisions are stale",
            ))?
            .iter()
            .map(|decision| {
                crate::terminal_identities::lookup_domain_id_for_carrier(
                    self.domain_ids,
                    decision.semantic_domain,
                    self.structural_type,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        domains.sort();
        domains.dedup();
        Ok(domains)
    }

    /// A fresh construction publishes its occurrence-bound source judgments
    /// before joining a continuation. This never relabels a parameter, source
    /// home or call ABI result, and grants no additional claim or disposal right.
    /// Both explicit fields and omitted-field zero provisioning use this join.
    pub(super) fn complete_construction(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        place: PlaceId,
        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
        continuation: Option<&super::ValueContinuation>,
    ) -> Result<PlaceId, LoweringError> {
        let domains = self.checked_membership_domains(decisions)?;
        let result = self
            .operations
            .operations
            .iter_mut()
            .filter_map(|operation| match &mut operation.result {
                terminal_psi::OperationResult::Structural(result) => Some(result),
                _ => None,
            })
            .find(|result| result.place == place)
            .ok_or(LoweringError::Unsupported(
                "fresh construction has no producer",
            ))?;
        result.qualifications.extend(domains);
        result.qualifications.sort();
        result.qualifications.dedup();
        self.completed(identities, place, continuation)
    }

    /// Publish checked operand facts without relabeling its producer's ABI
    /// contract or an existing home. An unrestricted capture retains additional
    /// facts through the ordinary value-copy operation. Already qualified values
    /// need no second copy. A membership judgment never authorizes affine copying.
    pub(super) fn retain_checked_memberships(
        &mut self,
        identities: &mut terminal_psi::TerminalIdentityTableBuilder,
        source: StructuralArgument,
        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
    ) -> Result<StructuralArgument, LoweringError> {
        let memberships = self
            .checked
            .facts
            .qualifications
            .value_formations
            .span(decisions)
            .ok_or(LoweringError::Unsupported(
                "operand membership decisions are stale",
            ))?;
        if memberships.is_empty() {
            return Ok(source);
        }
        // Source memberships qualify a reference's referent, not its affine
        // permission descriptor. Rejoin the already captured rows at that
        // boundary; a decision cannot mint a domain on the pointer carrier.
        let mut membership_path = source.path.clone();
        let membership_carrier =
            match terminal_psi::find_structural_type(self.structural_types, self.structural_type)
                .map(|declaration| &declaration.shape)
            {
                Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) => {
                    membership_path.push(terminal_psi::StructuralPathSegment::Referent);
                    *referent
                }
                _ => self.structural_type,
            };
        let (qualifications, projected_qualifications) = self
            .completed_memberships(source.place)
            .ok_or(LoweringError::Unsupported(
            "completed operand membership source is missing",
        ))?;
        let (qualifications, _) = terminal_psi::StructuralPathQualification::select(
            qualifications,
            projected_qualifications,
            &membership_path,
        );
        let mut missing = false;
        for decision in memberships {
            let domain = crate::terminal_identities::lookup_domain_id_for_carrier(
                self.domain_ids,
                decision.semantic_domain,
                membership_carrier,
            )?;
            missing |= !qualifications.contains(&domain);
        }
        if !missing {
            return Ok(source);
        }
        if self.multiplicity != StructuralMultiplicity::Unrestricted {
            return unsupported("affine operand must retain its checked membership roster");
        }
        self.copied_leaf_place(identities, &source, decisions, None)
            .map(|place| StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            })
    }
}

/// Registration retains the completed storage contract, rather than creating
/// an unqualified approximation of the value. Source replay and each emitted
/// operation own establishment; a continuation owns its arrival declaration.
pub(super) fn completed_result<'a>(
    place: PlaceId,
    declaration: &StructuralPlaceDeclaration,
    operations: &OperationBuffer,
    mut parameters: impl Iterator<Item = &'a StructuralParameterDeclaration> + Clone,
    arrival_claims: &[terminal_psi::StructuralResultClaimBinding],
) -> Result<terminal_psi::StructuralOperationResult, LoweringError> {
    let mut producers = operations.iter().filter_map(|operation| {
        operation
            .result
            .structural()
            .filter(|result| result.place == place)
            .map(|result| (operation.id, result))
    });
    if let Some((producer, result)) = producers.next() {
        if producers.next().is_some()
            || parameters.clone().any(|parameter| parameter.place == place)
            || declaration.kind
                != (StructuralPlaceKind::OperationResult {
                    producer,
                    structural_type: result.structural_type,
                })
        {
            return unsupported("completed value does not have one exact producer");
        }
        return Ok(result.clone());
    }
    let StructuralPlaceDeclaration {
        kind: StructuralPlaceKind::BlockParameter { .. },
        ..
    } = declaration
    else {
        return unsupported("completed value has no producer or arrival declaration");
    };
    let parameter = parameters
        .find(|parameter| parameter.place == place)
        .ok_or(LoweringError::Unsupported(
            "completed value has no current arrival",
        ))?;
    if arrival_claims.windows(2).any(|pair| pair[0] >= pair[1])
        || (parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            && arrival_claims.is_empty())
    {
        return unsupported("completed linear arrival lost its exact claim frontier");
    }
    Ok(terminal_psi::StructuralOperationResult {
        place,
        structural_type: parameter.structural_type,
        multiplicity: parameter.multiplicity,
        qualifications: parameter.qualifications.clone(),
        projected_qualifications: parameter.projected_qualifications.clone(),
        qualification_establishments: Vec::new(),
        claims: arrival_claims.to_vec(),
    })
}
