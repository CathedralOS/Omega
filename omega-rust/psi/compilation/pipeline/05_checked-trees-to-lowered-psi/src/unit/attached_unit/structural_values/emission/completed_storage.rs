//! Preserve producer-authored custody when publishing a completed source value.

use super::{
    Emission, LoweringError, OperationBuffer, PlaceId, StructuralAccess, StructuralArgument,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, unsupported,
};

impl Emission<'_, '_, '_> {
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
        self.checked
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
            .collect()
    }

    /// Publish checked operand facts without relabeling its producer's ABI
    /// contract or an existing home. An unrestricted capture retains additional
    /// facts through the ordinary value-copy operation. Already qualified values
    /// need no second copy. A membership judgment never authorizes affine copying.
    pub(super) fn retain_checked_memberships(
        &mut self,
        place: PlaceId,
        decisions: arena::HandleSpan<checked_trees::CheckedValueQualification>,
    ) -> Result<PlaceId, LoweringError> {
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
            return Ok(place);
        }
        let (qualifications, _) =
            self.completed_memberships(place)
                .ok_or(LoweringError::Unsupported(
                    "completed operand membership source is missing",
                ))?;
        let mut missing = false;
        for decision in memberships {
            let domain = crate::terminal_identities::lookup_domain_id_for_carrier(
                self.domain_ids,
                decision.semantic_domain,
                self.structural_type,
            )?;
            missing |= !qualifications.contains(&domain);
        }
        if !missing {
            return Ok(place);
        }
        if self.multiplicity != StructuralMultiplicity::Unrestricted {
            return unsupported("affine operand must retain its checked membership roster");
        }
        self.copied_leaf_place(
            &StructuralArgument {
                place,
                path: Vec::new(),
                access: StructuralAccess::Owned,
            },
            decisions,
            None,
        )
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
