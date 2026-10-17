//! Structural member paths, terms, float and byte-sequence fields and sum
//! subjects.

use crate::proofs::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatFormat,
    IeeeFloatStructuralField, LoweringError, PlaceId, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralFieldType, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, unsupported,
};

/// Resolve type meaning, not access or bounds evidence. Fixed arrays own a
/// static extent; views keep their independently admitted live-length
/// obligation while continuing through the same element/member namespace.
fn indexed_element_type(
    shape: &StructuralTypeShape,
    element_index: u64,
) -> Result<StructuralTypeId, LoweringError> {
    match shape {
        StructuralTypeShape::FixedArray { element, length } => {
            if element_index >= *length {
                return unsupported("structural contract fixed index is out of bounds");
            }
            Ok(*element)
        }
        StructuralTypeShape::ElementView { element, .. } => Ok(*element),
        _ => unsupported("structural contract index receiver is not a collection"),
    }
}

pub(crate) fn lower_structural_member_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<
    (
        PlaceId,
        Vec<CanonicalStructuralPathSegment>,
        StructuralFieldType,
    ),
    LoweringError,
> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural scalar contract names a non-structural parameter",
        ))?;
    if path.is_empty() {
        if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
            return unsupported("structural scalar contract requires readable access");
        }
        let declaration =
            terminal_psi::find_structural_type(structural_types, parameter.structural_type).ok_or(
                LoweringError::Unsupported("structural scalar contract root type is absent"),
            )?;
        let leaf = match declaration.shape {
            StructuralTypeShape::PrimitiveScalar(scalar) => StructuralFieldType::Scalar(scalar),
            _ => StructuralFieldType::Structural(parameter.structural_type),
        };
        return Ok((parameter.place, Vec::new(), leaf));
    }
    let mut structural_type = parameter.structural_type;
    let mut terminal_path = Vec::with_capacity(path.len());
    let mut selected_case_fields = None;
    for (index, segment) in path.iter().enumerate() {
        let declaration = terminal_psi::find_structural_type(structural_types, structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural scalar contract path type is absent",
            ))?;
        if let checked_trees::CheckedStructuralPredicatePathSegment::Case(identity) = segment {
            if selected_case_fields.is_some() || index + 1 == path.len() {
                return unsupported("structural scalar contract has a malformed case path");
            }
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => {
                    return unsupported("structural scalar contract case receiver is not a sum");
                }
            };
            let case = terminal_psi::structural_case_by_identity(
                cases,
                identities.intern(&identity.spelling()),
            )
            .ok_or(LoweringError::Unsupported(
                "structural scalar contract case is absent",
            ))?;
            terminal_path.push(CanonicalStructuralPathSegment::Case(case.id));
            selected_case_fields = Some(&case.fields);
            continue;
        }
        if let checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index) =
            segment
        {
            // A literal index selects an element. Below
            // the root it continues into the element's structure; as the
            // final segment it names one primitive scalar element, the leaf
            // a collection invariant's per-element row observes.
            if selected_case_fields.is_some() {
                return unsupported("structural scalar contract has a malformed indexed path");
            }
            let element = indexed_element_type(&declaration.shape, *element_index)?;
            terminal_path.push(CanonicalStructuralPathSegment::FixedIndex(*element_index));
            structural_type = element;
            if index + 1 == path.len() {
                let element_shape =
                    terminal_psi::find_structural_type(structural_types, structural_type)
                        .map(|declaration| &declaration.shape);
                let field = match element_shape {
                    Some(StructuralTypeShape::PrimitiveScalar(scalar)) => {
                        StructuralFieldType::Scalar(*scalar)
                    }
                    Some(_) => StructuralFieldType::Structural(structural_type),
                    None => return unsupported("structural contract element type is absent"),
                };
                // Path resolution can stop at an aggregate before a later
                // runtime selector. Scalar consumers separately require a
                // scalar endpoint; geometry itself does not impose that use.
                return Ok((parameter.place, terminal_path, field));
            }
            continue;
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            return unsupported(
                "structural contract runtime index requires occurrence-bound evidence",
            );
        };
        let fields = if let Some(fields) = selected_case_fields.take() {
            fields
        } else {
            match &declaration.shape {
                terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                _ => {
                    return unsupported(
                        "structural scalar contract field receiver is not a record",
                    );
                }
            }
        };
        let field =
            terminal_psi::field_by_identity(fields, identities.intern(&identity.spelling()))
                .filter(|field| !field.relevance.is_erased())
                .ok_or(LoweringError::Unsupported(
                    "structural scalar contract path field is absent or erased",
                ))?;
        terminal_path.push(CanonicalStructuralPathSegment::Field(field.id));
        let is_last = index + 1 == path.len();
        match (&field.field_type, is_last) {
            (StructuralFieldType::Structural(next), false) => structural_type = *next,
            (_, true) => return Ok((parameter.place, terminal_path, field.field_type.clone())),
            _ => {
                return unsupported(
                    "structural scalar contract path does not end at a retained leaf",
                );
            }
        }
    }
    unreachable!("nonempty structural path returns at its final field")
}

/// A descriptor extent retains the full structural path. Carrier checking
/// distinguishes element counts from byte storage; neither field names nor
/// an equally numbered scalar parameter supply descriptor authority.
pub(crate) fn lower_sequence_extent_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "closed sequence extent names a non-structural parameter",
        ))?;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("closed sequence extent requires readable access");
    }
    let (root, path, carrier) = if path.is_empty() {
        (
            parameter.place,
            Vec::new(),
            terminal_psi::StructuralFieldType::Structural(parameter.structural_type),
        )
    } else {
        crate::proofs::crash_routes::lower_structural_member_path(
            identities,
            parameter_position,
            path,
            parameters,
            structural_types,
        )?
    };
    let carrier = match carrier {
        terminal_psi::StructuralFieldType::Structural(identity) => {
            let declaration = terminal_psi::find_structural_type(structural_types, identity)
                .ok_or(LoweringError::Unsupported(
                    "closed sequence extent type is absent",
                ))?;
            match &declaration.shape {
                terminal_psi::StructuralTypeShape::ElementView { .. } => None,
                terminal_psi::StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                _ => return unsupported("closed sequence extent leaf is not a descriptor"),
            }
        }
        leaf => Some(
            leaf.byte_sequence_carrier()
                .ok_or(LoweringError::Unsupported(
                    "closed sequence extent leaf is not a descriptor",
                ))?,
        ),
    };
    if matches!(
        carrier,
        Some(
            terminal_psi::ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
            } | terminal_psi::ByteSequenceCarrier::BoundedView {
                access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                ..
            }
        )
    ) {
        return unsupported("closed sequence extent requires readable access");
    }
    let integer = crate::emission::scalar_types::terminal_scalar_type(
        checked_trees::types::PrimitiveType::U64,
    )?
    .as_integer()
    .ok_or(LoweringError::Unsupported(
        "sequence extent is not an integer",
    ))?;
    Ok(ScalarTerm::view_extent(root, path, integer))
}

/// Indexed observations retain each selector in the contract's scalar
/// namespace. Static runs use the same declared-field resolver as ordinary
/// member contracts; a dynamic step changes only collection selection.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_structural_indexed_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    expression: &checked_trees::CheckedScalarExpression,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
    lower_selector: &mut impl FnMut(
        &mut terminal_psi::TerminalIdentityTableBuilder,
        &checked_trees::CheckedScalarExpression,
    ) -> Result<ScalarTerm, LoweringError>,
) -> Result<ScalarTerm, LoweringError> {
    use checked_trees::CheckedScalarReadProjection;
    use semantic_vocabulary::ScalarReadProjection;
    let checked_trees::CheckedScalarExpression::StructuralParameterIndexedRead {
        root,
        path,
        index,
        element_path,
        primitive_type,
    } = expression
    else {
        return unsupported("indexed contract observation has no selected read");
    };
    let position = root.parameter().ok_or(LoweringError::Unsupported(
        "indexed contract observation requires a structural formal",
    ))?;
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == position)
        .ok_or(LoweringError::Unsupported(
            "indexed contract formal is absent",
        ))?;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return unsupported("indexed contract observation requires readable access");
    }
    let mut projections = path
        .iter()
        .cloned()
        .map(CheckedScalarReadProjection::Static)
        .collect::<Vec<_>>();
    projections.push(CheckedScalarReadProjection::RuntimeIndex((**index).clone()));
    projections.extend_from_slice(element_path);
    let mut terminal_path = Vec::new();
    let mut selected = StructuralFieldType::Structural(parameter.structural_type);
    let mut cursor = 0;
    while cursor < projections.len() {
        if matches!(&projections[cursor], CheckedScalarReadProjection::Static(_)) {
            let mut static_path = Vec::new();
            while let Some(CheckedScalarReadProjection::Static(segment)) = projections.get(cursor) {
                static_path.push(segment.clone());
                cursor += 1;
            }
            let StructuralFieldType::Structural(structural_type) = selected else {
                return unsupported("indexed contract member follows a scalar leaf");
            };
            let namespace = StructuralParameterDeclaration {
                structural_type,
                ..parameter.clone()
            };
            let (_, path, field) = lower_structural_member_path(
                identities,
                position,
                &static_path,
                &[namespace],
                structural_types,
            )?;
            terminal_path.extend(path.into_iter().map(ScalarReadProjection::Static));
            selected = field;
            continue;
        }
        let CheckedScalarReadProjection::RuntimeIndex(selector) = &projections[cursor] else {
            unreachable!();
        };
        let selector = lower_selector(identities, &selector.expression)?;
        if !matches!(selector.scalar_type(), ScalarType::Integer(integer) if !integer.is_address())
        {
            return unsupported("indexed contract selector is not a fixed integer");
        }
        let byte_carrier = match &selected {
            StructuralFieldType::Structural(identity) => {
                let declaration = terminal_psi::find_structural_type(structural_types, *identity)
                    .ok_or(LoweringError::Unsupported(
                    "indexed contract collection type is absent",
                ))?;
                match &declaration.shape {
                    StructuralTypeShape::FixedArray { element, .. }
                    | StructuralTypeShape::ElementView { element, .. } => {
                        selected = StructuralFieldType::Structural(*element);
                        None
                    }
                    StructuralTypeShape::ByteSequence(carrier) => Some(*carrier),
                    _ => return unsupported("indexed contract receiver is not a collection"),
                }
            }
            field => Some(
                field
                    .byte_sequence_carrier()
                    .ok_or(LoweringError::Unsupported(
                        "indexed contract receiver is not a collection",
                    ))?,
            ),
        };
        if let Some(carrier) = byte_carrier {
            if matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                } | terminal_psi::ByteSequenceCarrier::BoundedView {
                    access: Some(terminal_psi::StructuralAccess::WriteOnlyBorrow),
                    ..
                }
            ) {
                return unsupported("indexed contract byte observation requires readable access");
            }
            selected =
                StructuralFieldType::Scalar(crate::emission::scalar_types::terminal_scalar_type(
                    checked_trees::types::PrimitiveType::U8,
                )?);
        }
        let fixed = match &selector {
            ScalarTerm::Integer {
                value: semantic_vocabulary::IntegerValue::Unsigned(value),
                ..
            } => u64::try_from(*value).ok(),
            ScalarTerm::Integer {
                value: semantic_vocabulary::IntegerValue::Signed(value),
                ..
            } => u64::try_from(*value).ok(),
            _ => None,
        };
        terminal_path.push(match fixed {
            Some(index) => {
                ScalarReadProjection::Static(CanonicalStructuralPathSegment::FixedIndex(index))
            }
            None => ScalarReadProjection::Index(Box::new(selector)),
        });
        cursor += 1;
    }
    let actual = terminal_semantics::structural_read_scalar_type(
        structural_types.iter(),
        parameter.structural_type,
        &terminal_path,
    );
    let expected = crate::emission::scalar_types::terminal_scalar_type(*primitive_type)?;
    if actual != Some(expected) {
        return unsupported("indexed contract path does not select its retained scalar type");
    }
    // A closed selection is the same declared observation as a member path,
    // not a dynamic-read-shaped term that makes its pure domain opaque.
    let fixed = terminal_path
        .iter()
        .map(|step| match step {
            ScalarReadProjection::Static(step) => Some(step.clone()),
            _ => None,
        })
        .collect::<Option<Vec<_>>>();
    let term = match (fixed, expected) {
        (Some(path), ScalarType::Integer(integer)) => {
            ScalarTerm::integer_field_path(parameter.place, path, integer)
        }
        (Some(path), ScalarType::Boolean) => ScalarTerm::boolean_field_path(parameter.place, path),
        (Some(path), ScalarType::IeeeFloat(format)) => {
            ScalarTerm::ieee_float_field_path(parameter.place, path, format)
        }
        (None, _) => ScalarTerm::StructuralRead {
            root: parameter.place,
            path: terminal_path,
            scalar_type: expected,
        },
    };
    term.validate()
        .map_err(LoweringError::InvalidCrashPredicate)?;
    Ok(term)
}

pub(crate) fn lower_structural_member_term(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    expected: ScalarType,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let (root, terminal_path, actual) = lower_structural_member_path(
        identities,
        parameter_position,
        path,
        parameters,
        structural_types,
    )?;
    if actual.scalar_type() != Some(expected) {
        return unsupported(
            "structural scalar contract path does not end at the retained scalar type",
        );
    }
    Ok(match expected {
        ScalarType::Boolean => ScalarTerm::boolean_field_path(root, terminal_path),
        ScalarType::Integer(integer_type) => {
            ScalarTerm::integer_field_path(root, terminal_path, integer_type)
        }
        ScalarType::IeeeFloat(format) => {
            ScalarTerm::ieee_float_field_path(root, terminal_path, format)
        }
    })
}

pub(crate) fn lower_ieee_float_field(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    field: &checked_trees::CheckedStructuralParameterField,
    format: IeeeFloatFormat,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<IeeeFloatStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
        identities,
        field.parameter_position,
        &field.path,
        parameters,
        structural_types,
    )?;
    if actual != StructuralFieldType::IeeeFloat(format) {
        return unsupported("structural IEEE predicate leaf has the wrong retained format");
    }
    IeeeFloatStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
}

pub(crate) fn lower_byte_sequence_field(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    field: &checked_trees::CheckedStructuralParameterField,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ByteSequenceStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
        identities,
        field.parameter_position,
        &field.path,
        parameters,
        structural_types,
    )?;
    // Content predicates constrain the bytes, not their equality carrier.
    if actual.byte_sequence_carrier().is_none() {
        return unsupported("structural byte-sequence predicate leaf has the wrong retained type");
    }
    ByteSequenceStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
}

fn lower_sum_path(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    root: PlaceId,
    mut structural_type: StructuralTypeId,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<(StructuralCaseSubject, StructuralTypeId), LoweringError> {
    let mut terminal_path = Vec::with_capacity(path.len());
    let mut selected_case_fields = None;
    for segment in path {
        let declaration = terminal_psi::find_structural_type(structural_types, structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural sum predicate path type is absent",
            ))?;
        if let checked_trees::CheckedStructuralPredicatePathSegment::Case(identity) = segment {
            if selected_case_fields.is_some() {
                return unsupported("structural sum predicate has adjacent case selections");
            }
            let cases = match &declaration.shape {
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                    cases
                }
                _ => return unsupported("structural sum predicate case receiver is not a sum"),
            };
            let case = terminal_psi::structural_case_by_identity(
                cases,
                identities.intern(&identity.spelling()),
            )
            .ok_or(LoweringError::Unsupported(
                "structural sum predicate case is absent",
            ))?;
            terminal_path.push(CanonicalStructuralPathSegment::Case(case.id));
            selected_case_fields = Some(&case.fields);
            continue;
        }
        if let checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index) =
            segment
        {
            // The subject path may end at an inline element: `options[0]`
            // names the element's own sum value for case membership.
            if selected_case_fields.is_some() {
                return unsupported("structural sum predicate has an indexed case selection");
            }
            let element = indexed_element_type(&declaration.shape, *element_index)?;
            terminal_path.push(CanonicalStructuralPathSegment::FixedIndex(*element_index));
            structural_type = element;
            continue;
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            return unsupported(
                "structural contract runtime index requires occurrence-bound evidence",
            );
        };
        let fields = if let Some(fields) = selected_case_fields.take() {
            fields
        } else {
            match &declaration.shape {
                terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                _ => {
                    return unsupported("structural sum predicate path receiver is not a record");
                }
            }
        };
        let field =
            terminal_psi::field_by_identity(fields, identities.intern(&identity.spelling()))
                .filter(|field| !field.relevance.is_erased())
                .ok_or(LoweringError::Unsupported(
                    "structural sum predicate path field is absent or erased",
                ))?;
        let Some(next) = field.field_type.as_structural() else {
            return unsupported("structural sum predicate path does not reach a structural value");
        };
        terminal_path.push(CanonicalStructuralPathSegment::Field(field.id));
        structural_type = next;
    }
    if selected_case_fields.is_some() {
        return unsupported("structural sum predicate case selection has no payload field");
    }
    if !matches!(
        terminal_psi::find_structural_type(structural_types, structural_type)
            .map(|declaration| &declaration.shape),
        Some(terminal_psi::sum_or_mixed_shape_pattern!())
    ) {
        return unsupported("structural sum predicate subject is not a sum");
    }
    Ok((
        StructuralCaseSubject::new(root, terminal_path),
        structural_type,
    ))
}

/// Preserve an exact selected case without asserting any payload equation.
/// Entry requirements, normal guarantees and crash predicates share one path
/// resolver. Only guarantees supply a declared result; the result is not an
/// entry parameter and cannot lend a coincidentally equal parameter ordinal.
pub(crate) fn lower_structural_case_membership(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    subject: &checked_trees::CheckedStructuralCaseSubject,
    case_identity: &checked_trees::CheckedMemberIdentity,
    parameters: &[StructuralParameterDeclaration],
    result: Option<&terminal_psi::StructuralResultDeclaration>,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<semantic_vocabulary::Proposition, LoweringError> {
    let (root, structural_type) = match subject.root {
        checked_trees::CheckedStructuralCaseRoot::Parameter { position } => {
            let parameter = parameters
                .iter()
                .find(|parameter| parameter.position == position)
                .ok_or(LoweringError::Unsupported(
                    "case membership parameter is absent",
                ))?;
            (parameter.place, parameter.structural_type)
        }
        checked_trees::CheckedStructuralCaseRoot::Result => {
            let result = result.ok_or(LoweringError::Unsupported(
                "case membership result is outside the contract namespace",
            ))?;
            (result.place, result.structural_type)
        }
    };
    let (subject, structural_type) = lower_sum_path(
        identities,
        root,
        structural_type,
        &subject.path,
        structural_types,
    )?;
    let declaration = terminal_psi::find_structural_type(structural_types, structural_type).ok_or(
        LoweringError::Unsupported("case membership declaration is absent"),
    )?;
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return unsupported("case membership declaration is not a sum");
    };
    let mut matching = cases
        .iter()
        .filter(|case| case.identity == identities.intern(&case_identity.spelling()));
    let case = matching
        .next()
        .ok_or(LoweringError::Unsupported("case membership was redirected"))?;
    if matching.next().is_some() {
        return unsupported("case membership identity is ambiguous");
    }
    Ok(semantic_vocabulary::Proposition::StructuralCaseMembership {
        subject,
        case: case.id,
    })
}
