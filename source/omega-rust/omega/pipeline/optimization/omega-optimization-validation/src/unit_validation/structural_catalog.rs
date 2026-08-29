//! Structural type, domain, local, and provider catalogs.

use super::*;

pub(crate) fn index_structural_catalogs(
    unit: &PsiOptimizationUnit,
) -> Result<
    (
        BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
        BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
    ),
    OptimizationUnitValidationError,
> {
    let mut types = BTreeMap::new();
    let mut type_names = BTreeSet::new();
    for declaration in &unit.structural_types {
        if types.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralType(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty() || !type_names.insert(declaration.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
            );
        }
    }
    if unit
        .structural_types
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalStructuralTypeOrder);
    }
    for declaration in &unit.structural_types {
        match &declaration.shape {
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
            ) => {}
            psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BoundedOwned { .. },
            ) => {
                return Err(
                    OptimizationUnitValidationError::InvalidStructuralTypeIdentity(declaration.id),
                );
            }
            psi_terminal::StructuralTypeShape::FixedArray { length: 0, .. } => {
                return Err(
                    OptimizationUnitValidationError::InvalidStructuralArrayLength(declaration.id),
                );
            }
            psi_terminal::StructuralTypeShape::FixedArray { .. } => {}
            psi_terminal::StructuralTypeShape::Record { fields } => {
                validate_structural_fields(unit, declaration.id, None, fields, true)?;
            }
            psi_terminal::StructuralTypeShape::Sum { cases } => {
                validate_structural_cases(unit, declaration.id, cases)?;
            }
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => {
                validate_structural_fields(unit, declaration.id, None, fields, false)?;
                validate_structural_cases(unit, declaration.id, cases)?;
            }
        }
    }
    for declaration in &unit.structural_types {
        let referenced = match &declaration.shape {
            psi_terminal::StructuralTypeShape::ByteSequence(_) => Vec::new(),
            psi_terminal::StructuralTypeShape::Record { fields } => fields
                .iter()
                .filter_map(|field| match field.field_type {
                    psi_terminal::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
            psi_terminal::StructuralTypeShape::FixedArray { element, .. } => vec![*element],
            psi_terminal::StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| &case.fields)
                .filter_map(|field| match field.field_type {
                    psi_terminal::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
                .filter_map(|field| match field.field_type {
                    psi_terminal::StructuralFieldType::Structural(target) => Some(target),
                    _ => None,
                })
                .collect(),
        };
        if let Some(target) = referenced.iter().find(|target| !types.contains_key(target)) {
            return Err(OptimizationUnitValidationError::UnknownStructuralType(
                *target,
            ));
        }
    }
    validate_structural_type_graph(&types)?;
    let mut domains = BTreeMap::new();
    let mut names = BTreeSet::new();
    let mut semantic_domains = BTreeSet::new();
    for declaration in unit.structural_domains.iter() {
        if domains.insert(declaration.id, declaration).is_some() {
            return Err(OptimizationUnitValidationError::DuplicateStructuralDomain(
                declaration.id,
            ));
        }
        if declaration.identity.is_empty()
            || !names.insert(declaration.identity.as_str())
            || !semantic_domains.insert(declaration.semantic_domain)
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralDomainIdentity(declaration.id),
            );
        }
    }
    if unit
        .structural_domains
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(OptimizationUnitValidationError::NonCanonicalStructuralDomainOrder);
    }
    if let Some(carrier) = unit
        .structural_domains
        .iter()
        .map(|declaration| declaration.carrier)
        .find(|carrier| !types.contains_key(carrier))
    {
        return Err(OptimizationUnitValidationError::UnknownStructuralType(
            carrier,
        ));
    }
    for declaration in unit.structural_domains.iter() {
        if declaration
            .content_projection
            .as_ref()
            .is_some_and(|projection| {
                !validate_structural_content_projection(
                    declaration.semantic_domain,
                    declaration.carrier,
                    projection,
                    &types,
                )
            })
        {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralDomainContentProjection(
                    declaration.id,
                ),
            );
        }
    }
    Ok((types, domains))
}

pub(crate) fn validate_content_projection_scalar(
    value: &ContentProjectionScalar,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    depth: usize,
) -> bool {
    if depth > 256 {
        return false;
    }
    match value {
        ContentProjectionScalar::SubjectField(path)
        | ContentProjectionScalar::RuntimeScalarEmbedding(path) => {
            if path.is_empty() || path.iter().any(String::is_empty) {
                return false;
            }
            let mut current = carrier;
            for (index, segment) in path.iter().enumerate() {
                let Some(declaration) = types.get(&current) else {
                    return false;
                };
                let psi_terminal::StructuralTypeShape::Record { fields } = &declaration.shape
                else {
                    return false;
                };
                let Some(field) = fields.iter().find(|field| field.identity == *segment) else {
                    return false;
                };
                let last = index + 1 == path.len();
                match (&field.field_type, last) {
                    (psi_terminal::StructuralFieldType::Structural(next), false) => {
                        current = *next;
                    }
                    (psi_terminal::StructuralFieldType::Scalar(_), true) => {}
                    _ => return false,
                }
            }
            true
        }
        ContentProjectionScalar::Natural(value) => {
            !value.is_empty()
                && value.bytes().all(|byte| byte.is_ascii_digit())
                && (value == "0" || !value.starts_with('0'))
        }
        ContentProjectionScalar::Successor(inner) => {
            validate_content_projection_scalar(inner, carrier, types, depth + 1)
        }
        ContentProjectionScalar::Add(left, right)
        | ContentProjectionScalar::Subtract(left, right)
        | ContentProjectionScalar::Multiply(left, right) => {
            validate_content_projection_scalar(left, carrier, types, depth + 1)
                && validate_content_projection_scalar(right, carrier, types, depth + 1)
        }
    }
}

pub(crate) fn validate_content_projection_expression(
    expression: &ContentProjectionExpression,
    carrier: StructuralTypeId,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    match expression {
        ContentProjectionExpression::IntervalSet(members) => members.iter().all(|(start, end)| {
            validate_content_projection_scalar(start, carrier, types, 0)
                && validate_content_projection_scalar(end, carrier, types, 0)
        }),
        ContentProjectionExpression::CountedQuantity(magnitude) => {
            validate_content_projection_scalar(magnitude, carrier, types, 0)
        }
    }
}

pub(crate) fn validate_structural_content_projection(
    semantic_domain: psi_core::DomainSemanticId,
    carrier: StructuralTypeId,
    projection: &psi_terminal::StructuralContentProjection,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> bool {
    let shape_matches_algebra = matches!(
        (&projection.expression, projection.algebra.kind),
        (
            ContentProjectionExpression::IntervalSet(_),
            psi_core::ContentAlgebraKind::IntervalSet
        ) | (
            ContentProjectionExpression::CountedQuantity(_),
            psi_core::ContentAlgebraKind::CountedQuantity
        )
    );
    projection.identity.domain.get() == semantic_domain.get()
        && projection.identity.projection_fingerprint != 0
        && !projection.algebra.parameter.is_empty()
        && shape_matches_algebra
        && validate_content_projection_expression(&projection.expression, carrier, types)
        && psi_language_semantics::content::terminal_projection_fingerprint(
            &projection.algebra,
            &projection.expression,
        ) == projection.identity.projection_fingerprint
}

pub(crate) fn validate_structural_fields(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    case: Option<psi_core::StructuralCaseId>,
    fields: &[psi_terminal::StructuralFieldDeclaration],
    permit_provider_attachment: bool,
) -> Result<(), OptimizationUnitValidationError> {
    if fields.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(
            OptimizationUnitValidationError::NonCanonicalStructuralFieldOrder {
                structural_type,
                case,
            },
        );
    }
    let mut identities = BTreeSet::new();
    for field in fields {
        if field.identity.is_empty() || !identities.insert(field.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralFieldIdentity {
                    structural_type,
                    field: field.id,
                },
            );
        }
        let invalid_erased = || OptimizationUnitValidationError::InvalidErasedStructuralField {
            structural_type,
            field: field.id,
        };
        match (&field.field_type, field.relevance) {
            (psi_terminal::StructuralFieldType::Erased { type_identity }, _)
                if type_identity.is_empty() =>
            {
                return Err(invalid_erased());
            }
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Erased,
            ) => {}
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Relevant,
            ) if permit_provider_attachment
                && has_provider_attachment_witness(unit, structural_type, field.id) => {}
            (
                psi_terminal::StructuralFieldType::Erased { .. },
                psi_terminal::BindingRelevance::Relevant,
            ) => return Err(invalid_erased()),
            (
                psi_terminal::StructuralFieldType::Scalar(_)
                | psi_terminal::StructuralFieldType::IeeeFloat(_)
                | psi_terminal::StructuralFieldType::Structural(_),
                psi_terminal::BindingRelevance::Erased,
            ) => return Err(invalid_erased()),
            (
                psi_terminal::StructuralFieldType::Scalar(_)
                | psi_terminal::StructuralFieldType::IeeeFloat(_)
                | psi_terminal::StructuralFieldType::ByteSequence(_)
                | psi_terminal::StructuralFieldType::Structural(_),
                psi_terminal::BindingRelevance::Relevant,
            )
            | (
                psi_terminal::StructuralFieldType::ByteSequence(_),
                psi_terminal::BindingRelevance::Erased,
            ) => {}
        }
    }
    Ok(())
}

pub(crate) fn validate_structural_cases(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    cases: &[psi_terminal::StructuralCaseDeclaration],
) -> Result<(), OptimizationUnitValidationError> {
    if cases.is_empty() {
        return Err(OptimizationUnitValidationError::EmptyStructuralSum(
            structural_type,
        ));
    }
    if cases.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(
            OptimizationUnitValidationError::NonCanonicalStructuralCaseOrder(structural_type),
        );
    }
    let mut identities = BTreeSet::new();
    for case in cases {
        if case.identity.is_empty() || !identities.insert(case.identity.as_str()) {
            return Err(
                OptimizationUnitValidationError::InvalidStructuralCaseIdentity {
                    structural_type,
                    case: case.id,
                },
            );
        }
    }
    for case in cases {
        validate_structural_fields(unit, structural_type, Some(case.id), &case.fields, false)?;
    }
    Ok(())
}

pub(crate) fn has_provider_attachment_witness(
    unit: &PsiOptimizationUnit,
    structural_type: StructuralTypeId,
    field: psi_core::StructuralFieldId,
) -> bool {
    unit.functions.iter().any(|function| {
        function.attachment == Some(structural_type)
            && function.structural_places.iter().any(|place| {
                matches!(
                    place.kind,
                    StructuralPlaceKind::ProviderAttachment {
                        attachment,
                        field: provider_field,
                        ..
                    } if attachment == structural_type && provider_field == field
                )
            })
    })
}

pub(crate) fn validate_structural_type_graph(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    fn visit(
        id: StructuralTypeId,
        types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
        active: &mut BTreeSet<StructuralTypeId>,
        complete: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), OptimizationUnitValidationError> {
        if complete.contains(&id) {
            return Ok(());
        }
        if !active.insert(id) {
            return Err(OptimizationUnitValidationError::RecursiveStructuralType(id));
        }
        let declaration = types[&id];
        match &declaration.shape {
            psi_terminal::StructuralTypeShape::ByteSequence(_) => {}
            psi_terminal::StructuralTypeShape::Record { fields } => {
                for field in fields {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            psi_terminal::StructuralTypeShape::FixedArray { element, .. } => {
                visit(*element, types, active, complete)?;
            }
            psi_terminal::StructuralTypeShape::Sum { cases } => {
                for field in cases.iter().flat_map(|case| &case.fields) {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
            psi_terminal::StructuralTypeShape::Mixed { fields, cases } => {
                for field in fields
                    .iter()
                    .chain(cases.iter().flat_map(|case| &case.fields))
                {
                    if let psi_terminal::StructuralFieldType::Structural(target) = field.field_type
                    {
                        visit(target, types, active, complete)?;
                    }
                }
            }
        }
        active.remove(&id);
        complete.insert(id);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    for id in types.keys().copied() {
        visit(id, types, &mut active, &mut complete)?;
    }
    Ok(())
}

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
    if let Some(result) = function.result.structural() {
        if places.get(&result.place) != Some(&StructuralPlaceKind::Result)
            || !types.contains_key(&result.structural_type)
            || !structural_qualifications_match(
                result.structural_type,
                &result.qualifications,
                domains,
            )
        {
            return Err(mismatch());
        }
    }
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
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

pub(crate) fn validate_byte_sequence_literal_witnesses(
    function: &PsiOptimizationFunction,
    expected_literals: &[(
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )],
) -> Result<(), OptimizationUnitValidationError> {
    let mut expected = expected_literals
        .iter()
        .map(|(place, structural_type)| (place.id, (*place, structural_type)))
        .collect::<BTreeMap<_, _>>();
    let mut actual = 0_usize;
    for node in function.blocks.iter().flat_map(|block| &block.nodes) {
        let O::EstablishByteSequenceLiteral {
            place,
            structural_type,
            ..
        } = &node.operation
        else {
            continue;
        };
        actual += 1;
        if expected
            .remove(&place.id)
            .is_none_or(|(expected_place, expected_type)| {
                *place != expected_place || structural_type != expected_type
            })
        {
            return Err(
                OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                    function.machine,
                ),
            );
        }
    }
    if actual != expected_literals.len() || !expected.is_empty() {
        return Err(
            OptimizationUnitValidationError::ByteSequenceLiteralEstablishmentMismatch(
                function.machine,
            ),
        );
    }
    Ok(())
}

pub(crate) fn validate_trivial_affine_local_witnesses(
    function: &PsiOptimizationFunction,
    expected_locals: &[(
        psi_terminal::StructuralPlaceDeclaration,
        psi_terminal::StructuralTypeDeclaration,
    )],
) -> Result<(), OptimizationUnitValidationError> {
    let explicit = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            } => Some((*psi_operation, *place, structural_type)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let structural_returns = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.operation {
            O::ReturnStructural {
                trivial_affine_locals,
                trivial_affine_discards,
                ..
            } => Some((trivial_affine_locals, trivial_affine_discards)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if !explicit.is_empty() {
        let exact = structural_returns.is_empty()
            && explicit.len() == expected_locals.len()
            && explicit.iter().zip(expected_locals).all(
                |((_, actual_place, actual_type), (expected_place, expected_type))| {
                    actual_place == expected_place && *actual_type == expected_type
                },
            );
        if !exact {
            return Err(
                OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                    function.machine,
                ),
            );
        }
        return Ok(());
    }

    if !expected_locals.is_empty() && structural_returns.len() != 1 {
        return Err(
            OptimizationUnitValidationError::TrivialAffineLocalEstablishmentMismatch(
                function.machine,
            ),
        );
    }

    let executable_operations = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter(|node| !matches!(node.operation, O::ReturnStructural { .. }))
        .flat_map(|node| expected_provenance(&node.operation))
        .filter_map(|site| match site {
            PsiProvenance::Operation(operation) => Some(operation),
            PsiProvenance::Edge(_) => None,
        })
        .collect::<BTreeSet<_>>();

    for block in &function.blocks {
        for (node_index, node) in block.nodes.iter().enumerate() {
            let O::ReturnStructural {
                source,
                trivial_affine_locals,
                trivial_affine_discards,
                ..
            } = &node.operation
            else {
                continue;
            };
            if trivial_affine_locals.is_empty()
                && trivial_affine_discards.is_empty()
                && expected_locals.is_empty()
            {
                continue;
            }
            let node_index = u32::try_from(node_index).expect("unit node index fits u32");
            let mut hidden_operations = BTreeSet::new();
            if trivial_affine_locals.len() != expected_locals.len()
                || trivial_affine_locals.iter().zip(expected_locals).any(
                    |((operation, actual_place, actual_type), (expected_place, expected_type))| {
                        actual_place != expected_place
                            || actual_type != expected_type
                            || !hidden_operations.insert(*operation)
                            || executable_operations.contains(operation)
                    },
                )
            {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineLocalsMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }

            let Some(returned_parameter) = function.structural_parameters.first() else {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            };
            let Some(result) = function.result.structural() else {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            };
            if !function.parameters.is_empty()
                || returned_parameter.place != *source
                || returned_parameter.is_self
                || returned_parameter.multiplicity != psi_terminal::StructuralMultiplicity::Linear
                || result.multiplicity != psi_terminal::StructuralMultiplicity::Linear
                || returned_parameter.structural_type != result.structural_type
                || returned_parameter.qualifications != result.qualifications
                || returned_parameter.place == result.place
                || function
                    .structural_parameters
                    .iter()
                    .skip(1)
                    .any(|parameter| {
                        parameter.is_self
                            || parameter.multiplicity
                                != psi_terminal::StructuralMultiplicity::Affine
                            || !parameter.qualifications.is_empty()
                    })
            {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnTrivialAffineShapeMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }
            let expected_discards = trivial_affine_locals
                .iter()
                .rev()
                .map(|(_, local, _)| local.id)
                .chain(
                    function
                        .structural_parameters
                        .iter()
                        .skip(1)
                        .rev()
                        .map(|parameter| parameter.place),
                )
                .collect::<Vec<_>>();
            if *trivial_affine_discards != expected_discards {
                return Err(
                    OptimizationUnitValidationError::StructuralReturnAffineDiscardsMismatch {
                        machine: function.machine,
                        block: block.id,
                        node: node_index,
                    },
                );
            }
        }
    }
    Ok(())
}

/// Replay the exact specialization which replaces one relevant opaque Record
/// field with a canonical boundary-specific provider-root roster. These roots
/// are retained specialization witnesses, not direct boundary/Unit-call
/// structural arguments.
pub(crate) fn validate_provider_attachment_specialization(
    function: &PsiOptimizationFunction,
    boundary_machines: &BTreeMap<BoundaryMachineId, &psi_terminal::BoundaryMachineDeclaration>,
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let provider_roots = function
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::ProviderAttachment {
                attachment,
                field,
                boundary,
            } => Some((place.id, attachment, field, boundary)),
            _ => None,
        })
        .collect::<Vec<_>>();
    let provider_fields = function
        .attachment
        .and_then(|attachment| types.get(&attachment))
        .and_then(|attachment| match &attachment.shape {
            psi_terminal::StructuralTypeShape::Record { fields } => Some(
                fields
                    .iter()
                    .filter(|field| {
                        !field.relevance.is_erased()
                            && matches!(
                                field.field_type,
                                psi_terminal::StructuralFieldType::Erased { .. }
                            )
                    })
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .unwrap_or_default();
    if provider_fields.is_empty() && provider_roots.is_empty() {
        return Ok(());
    }

    let invalid = || {
        OptimizationUnitValidationError::InvalidProviderAttachmentSpecialization(function.machine)
    };
    let [provider_field] = provider_fields.as_slice() else {
        return Err(invalid());
    };
    let Some(attachment) = function.attachment else {
        return Err(invalid());
    };
    if provider_roots.is_empty()
        || function
            .structural_parameters
            .iter()
            .any(|parameter| parameter.is_self)
        || provider_roots.windows(2).any(|pair| pair[0].3 >= pair[1].3)
    {
        return Err(invalid());
    }

    let mut specialized_boundaries = BTreeSet::new();
    let provider_places = provider_roots
        .iter()
        .map(|(place, ..)| *place)
        .collect::<BTreeSet<_>>();
    for (_, root_attachment, field, boundary) in &provider_roots {
        let Some(boundary_declaration) = boundary_machines.get(boundary) else {
            return Err(invalid());
        };
        if *root_attachment != attachment
            || *field != provider_field.id
            || boundary_declaration.attachment.is_some()
            || !specialized_boundaries.insert(*boundary)
        {
            return Err(invalid());
        }
    }

    let mut called_boundaries = BTreeSet::new();
    for operation in function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .map(|node| &node.operation)
    {
        match operation {
            O::BoundaryCall {
                boundary,
                structural_arguments,
                ..
            } => {
                called_boundaries.insert(*boundary);
                if structural_arguments
                    .iter()
                    .any(|argument| provider_places.contains(&argument.place))
                {
                    return Err(invalid());
                }
            }
            O::CallUnit {
                structural_arguments,
                ..
            } if structural_arguments
                .iter()
                .any(|argument| provider_places.contains(&argument.place)) =>
            {
                return Err(invalid());
            }
            _ => {}
        }
    }
    if called_boundaries != specialized_boundaries {
        return Err(invalid());
    }
    Ok(())
}

pub(crate) fn structural_qualifications_match(
    carrier: StructuralTypeId,
    qualifications: &[StructuralDomainId],
    domains: &BTreeMap<StructuralDomainId, &psi_terminal::StructuralDomainDeclaration>,
) -> bool {
    !qualifications.windows(2).any(|pair| pair[0] >= pair[1])
        && qualifications.iter().all(|domain| {
            domains
                .get(domain)
                .is_some_and(|domain| domain.carrier == carrier)
        })
}

pub(crate) fn resolve_structural_path(
    types: &BTreeMap<StructuralTypeId, &psi_terminal::StructuralTypeDeclaration>,
    mut structural_type: StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
) -> Option<StructuralTypeId> {
    types.get(&structural_type)?;
    for segment in path {
        let declaration = types.get(&structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                psi_terminal::StructuralPathSegment::Field(identity),
                psi_terminal::StructuralTypeShape::Record { fields },
            ) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let psi_terminal::StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                psi_terminal::StructuralPathSegment::FixedIndex(index),
                psi_terminal::StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}
