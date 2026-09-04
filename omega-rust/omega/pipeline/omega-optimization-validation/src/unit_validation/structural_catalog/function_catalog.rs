use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ValidatorStructuralRootKey {
    Parameter(u32),
    Result,
    OperationResult(OperationId),
    ByteSequenceLiteral(u32),
    ProviderAttachment(
        StructuralTypeId,
        psi_core::StructuralFieldId,
        BoundaryMachineId,
    ),
    TrivialAffineLocal(u32),
}

pub(crate) fn structural_root_key(kind: StructuralPlaceKind) -> ValidatorStructuralRootKey {
    match kind {
        StructuralPlaceKind::Parameter { position, .. } => {
            ValidatorStructuralRootKey::Parameter(position)
        }
        StructuralPlaceKind::Result => ValidatorStructuralRootKey::Result,
        StructuralPlaceKind::OperationResult { producer, .. } => {
            ValidatorStructuralRootKey::OperationResult(producer)
        }
        StructuralPlaceKind::ByteSequenceLiteral {
            declaration_ordinal,
            ..
        } => ValidatorStructuralRootKey::ByteSequenceLiteral(declaration_ordinal),
        StructuralPlaceKind::ProviderAttachment {
            attachment,
            field,
            boundary,
        } => ValidatorStructuralRootKey::ProviderAttachment(attachment, field, boundary),
        StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            ..
        } => ValidatorStructuralRootKey::TrivialAffineLocal(declaration_ordinal),
    }
}

pub(crate) fn validate_function_structural_catalog(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> Result<
    (
        Vec<(
            psi_terminal::StructuralPlaceDeclaration,
            psi_terminal::StructuralTypeDeclaration,
        )>,
        Vec<(
            psi_terminal::StructuralPlaceDeclaration,
            psi_terminal::StructuralTypeDeclaration,
        )>,
    ),
    OptimizationUnitValidationError,
> {
    let mismatch = || OptimizationUnitValidationError::StructuralCatalogMismatch {
        machine: Some(function.machine),
    };
    if !structural_signature_matches(
        &function.structural_parameters,
        function.attachment,
        types,
        domains,
    ) {
        return Err(mismatch());
    }
    let mut parameter_places = BTreeSet::new();
    for (position, parameter) in function.structural_parameters.iter().enumerate() {
        if parameter.position != u32::try_from(position).map_err(|_| mismatch())?
            || !parameter_places.insert(parameter.place)
            || !types.contains_key(&parameter.structural_type)
            || !structural_qualifications_match(
                parameter.structural_type,
                &parameter.qualifications,
                domains,
            )
            || !structural_projected_qualifications_match(
                parameter.structural_type,
                &parameter.projected_qualifications,
                types,
                domains,
            )
        {
            return Err(mismatch());
        }
    }
    let mut places = BTreeMap::new();
    for place in &function.structural_places {
        if places.insert(place.id, place.kind).is_some() {
            return Err(mismatch());
        }
        let known_type = match place.kind {
            StructuralPlaceKind::Parameter { position, is_self } => function
                .structural_parameters
                .get(position as usize)
                .is_some_and(|parameter| {
                    parameter.place == place.id && parameter.is_self == is_self
                }),
            StructuralPlaceKind::Result => function
                .result
                .structural()
                .is_some_and(|result| result.place == place.id),
            StructuralPlaceKind::OperationResult {
                producer,
                structural_type,
            } => {
                types.contains_key(&structural_type)
                    && function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.nodes)
                        .any(|node| {
                            matches!(
                                &node.operation,
                                O::EstablishPayloadlessCase {
                                    psi_operation,
                                    result,
                                    ..
                                }
                                | O::EstablishAffineScalarRecord {
                                    psi_operation,
                                    result,
                                    ..
                                }
                                | O::CallStructural { psi_operation, result, .. }
                                    if *psi_operation == producer
                                        && result.place == place.id
                                        && result.structural_type == structural_type
                            )
                        })
            }
            StructuralPlaceKind::ByteSequenceLiteral {
                structural_type: _, ..
            } => true,
            StructuralPlaceKind::TrivialAffineLocal { .. } => true,
            StructuralPlaceKind::ProviderAttachment { attachment, .. } => {
                types.contains_key(&attachment) && function.attachment == Some(attachment)
            }
        };
        if !known_type {
            return Err(mismatch());
        }
    }
    for parameter in &function.structural_parameters {
        if places.get(&parameter.place)
            != Some(&StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            })
        {
            return Err(mismatch());
        }
        if parameter.multiplicity == psi_terminal::StructuralMultiplicity::Linear
            && !function
                .entry_claim_declarations
                .iter()
                .any(|claim| claim.input == parameter.place)
        {
            return Err(mismatch());
        }
    }
    if let Some(result) = function.result.structural()
        && (places.get(&result.place) != Some(&StructuralPlaceKind::Result)
            || !types.contains_key(&result.structural_type)
            || !structural_qualifications_match(
                result.structural_type,
                &result.qualifications,
                domains,
            )
            || !structural_projected_qualifications_match(
                result.structural_type,
                &result.projected_qualifications,
                types,
                domains,
            ))
    {
        return Err(mismatch());
    }
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        let structural_result = match &node.operation {
            O::EstablishPayloadlessCase { result, .. }
            | O::EstablishAffineScalarRecord { result, .. }
            | O::CallStructural { result, .. } => Some(result),
            _ => None,
        };
        if let Some(result) = structural_result
            && (!structural_qualifications_match(
                result.structural_type,
                &result.qualifications,
                domains,
            ) || !structural_projected_qualifications_match(
                result.structural_type,
                &result.projected_qualifications,
                types,
                domains,
            ))
        {
            return Err(mismatch());
        }
        let expected = match &node.operation {
            O::EstablishByteSequenceLiteral { place, .. } => Some((place.id, place.kind)),
            // Trivial affine locals have two faithful representations: an
            // executable establishment in Unit lowering, or an exact typed
            // tuple compressed into ReturnStructural. Their one-to-one
            // recognition is validated together below.
            O::EstablishTrivialAffineLocal { .. } => None,
            O::EstablishPayloadlessCase {
                psi_operation,
                result,
                ..
            }
            | O::EstablishAffineScalarRecord {
                psi_operation,
                result,
                ..
            }
            | O::CallStructural {
                psi_operation,
                result,
                ..
            } => Some((
                result.place,
                StructuralPlaceKind::OperationResult {
                    producer: *psi_operation,
                    structural_type: result.structural_type,
                },
            )),
            _ => None,
        };
        if expected.is_some_and(|(place, kind)| places.get(&place) != Some(&kind)) {
            return Err(mismatch());
        }
    }
    let mut claim_inputs = Vec::new();
    for (index, claim) in function.entry_claim_declarations.iter().enumerate() {
        let expected = ClaimId::new(
            u64::try_from(index)
                .map_err(|_| mismatch())?
                .checked_add(1)
                .ok_or_else(mismatch)?,
        )
        .ok_or_else(mismatch)?;
        let Some(parameter) = function
            .structural_parameters
            .iter()
            .find(|parameter| parameter.place == claim.input)
        else {
            return Err(mismatch());
        };
        if claim.claim != expected
            || parameter.multiplicity == psi_terminal::StructuralMultiplicity::Unrestricted
            || resolve_structural_path(types, parameter.structural_type, &claim.path).is_none()
            || claim_inputs
                .iter()
                .any(|previous: &&psi_terminal::EntryClaim| {
                    previous.input == claim.input
                        && (previous.path.starts_with(&claim.path)
                            || claim.path.starts_with(&previous.path))
                })
        {
            return Err(mismatch());
        }
        claim_inputs.push(claim);
    }
    if function
        .content_entry_claims
        .iter()
        .enumerate()
        .any(|(index, claim)| {
            let expected = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .and_then(ClaimId::new);
            let structural_binding_matches = function
                .entry_claim_declarations
                .iter()
                .find(|entry| entry.claim == claim.claim)
                .is_none_or(|entry| {
                    entry.input == claim.input.root
                        && claim.input.segments
                            == entry
                                .path
                                .iter()
                                .map(|segment| match segment {
                                    psi_terminal::StructuralPathSegment::Field(identity) => {
                                        psi_core::ContentPlaceSegment::Field(identity.clone())
                                    }
                                    psi_terminal::StructuralPathSegment::FixedIndex(index) => {
                                        psi_core::ContentPlaceSegment::FixedIndex(*index)
                                    }
                                })
                                .collect::<Vec<_>>()
                });
            expected != Some(claim.claim)
                || claim.input.version != psi_core::ContentPlaceVersion::Entry
                || !parameter_places.contains(&claim.input.root)
                || claim.projections.is_empty()
                || claim.projections.windows(2).any(|pair| pair[0] >= pair[1])
                || !structural_binding_matches
        })
    {
        return Err(mismatch());
    }
    for projection in function
        .content_entry_claims
        .iter()
        .flat_map(|claim| &claim.projections)
    {
        let owner = domains.values().find_map(|domain| {
            domain
                .content_projection
                .as_ref()
                .filter(|owner| owner.identity.domain == projection.projection.domain)
        });
        if !owner.is_some_and(|owner| {
            owner.identity == projection.projection && owner.algebra == projection.algebra
        }) {
            return Err(
                OptimizationUnitValidationError::ContentProjectionOwnerMismatch(
                    projection.projection,
                ),
            );
        }
    }
    let mut byte_sequence_literals = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal,
                structural_type,
            } => Some((*place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    byte_sequence_literals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
    if byte_sequence_literals
        .iter()
        .enumerate()
        .any(|(expected, (_, declaration_ordinal, _))| {
            u32::try_from(expected).ok() != Some(*declaration_ordinal)
        })
    {
        return Err(
            OptimizationUnitValidationError::NonCanonicalByteSequenceLiterals(function.machine),
        );
    }
    let byte_sequence_literals = byte_sequence_literals
        .into_iter()
        .map(|(place, _, structural_type)| {
            let declaration = types.get(&structural_type).ok_or(
                OptimizationUnitValidationError::UnknownStructuralType(structural_type),
            )?;
            if !matches!(
                declaration.shape,
                psi_terminal::StructuralTypeShape::ByteSequence(
                    psi_terminal::ByteSequenceCarrier::BorrowedView
                )
            ) {
                return Err(
                    OptimizationUnitValidationError::ByteSequenceLiteralDeclarationRequiresBorrowedView {
                        machine: function.machine,
                        place: place.id,
                    },
                );
            }
            Ok((place, (*declaration).clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut trivial_affine_locals = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                ..
            } => Some((*place, declaration_ordinal, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    trivial_affine_locals.sort_by_key(|(_, declaration_ordinal, _)| *declaration_ordinal);
    if trivial_affine_locals
        .iter()
        .enumerate()
        .any(|(expected, (_, declaration_ordinal, _))| {
            u32::try_from(expected).ok() != Some(*declaration_ordinal)
        })
    {
        return Err(
            OptimizationUnitValidationError::NonCanonicalTrivialAffineLocals(function.machine),
        );
    }
    let trivial_affine_locals = trivial_affine_locals
        .into_iter()
        .map(|(place, _, structural_type)| {
            let declaration = types.get(&structural_type).ok_or(
                OptimizationUnitValidationError::UnknownStructuralType(structural_type),
            )?;
            if !matches!(
                declaration.shape,
                psi_terminal::StructuralTypeShape::Record { ref fields } if fields.is_empty()
            ) {
                return Err(
                    OptimizationUnitValidationError::TrivialAffineLocalDeclarationRequiresEmptyRecord {
                        machine: function.machine,
                        place: place.id,
                    },
                );
            }
            Ok((place, (*declaration).clone()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((byte_sequence_literals, trivial_affine_locals))
}
