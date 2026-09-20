//! Structural member paths, terms, float and byte-sequence fields and sum
//! subjects.

use crate::proofs::{
    ByteSequenceStructuralField, CanonicalStructuralPathSegment, IeeeFloatFormat,
    IeeeFloatStructuralField, LoweringError, PlaceId, ScalarTerm, ScalarType,
    StructuralCaseSubject, StructuralFieldType, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, unsupported,
};

pub(crate) fn lower_structural_member_path(
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
    if path.is_empty() {
        return unsupported("structural scalar contract has an empty member path");
    }
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural scalar contract names a non-structural parameter",
        ))?;
    let mut structural_type = parameter.structural_type;
    let mut terminal_path = Vec::with_capacity(path.len());
    let mut selected_case_fields = None;
    for (index, segment) in path.iter().enumerate() {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
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
            let case = cases
                .iter()
                .find(|candidate| candidate.identity == *identity)
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
            // A literal fixed-array index selects an inline element; it can
            // appear anywhere below the root but cannot terminate the path,
            // since the leaf must stay a field-typed value.
            if selected_case_fields.is_some() || index + 1 == path.len() {
                return unsupported("structural scalar contract has a malformed indexed path");
            }
            let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                return unsupported(
                    "structural scalar contract index receiver is not a fixed array",
                );
            };
            if *element_index >= *length {
                return unsupported("structural scalar contract fixed index is out of bounds");
            }
            terminal_path.push(CanonicalStructuralPathSegment::FixedIndex(*element_index));
            structural_type = *element;
            continue;
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            unreachable!("case and index paths handled above")
        };
        let fields = if let Some(fields) = selected_case_fields.take() {
            fields
        } else {
            match &declaration.shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => {
                    return unsupported(
                        "structural scalar contract field receiver is not a record",
                    );
                }
            }
        };
        let field = fields
            .iter()
            .find(|candidate| candidate.identity == *identity)
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

pub(crate) fn lower_structural_member_term(
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    expected: ScalarType,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ScalarTerm, LoweringError> {
    let (root, terminal_path, actual) =
        lower_structural_member_path(parameter_position, path, parameters, structural_types)?;
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
        ScalarType::IeeeFloat(_) => {
            return unsupported(
                "generic scalar crash terms do not carry IEEE float structural fields",
            );
        }
    })
}

pub(crate) fn lower_ieee_float_field(
    field: &checked_trees::CheckedStructuralParameterField,
    format: IeeeFloatFormat,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<IeeeFloatStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
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
    field: &checked_trees::CheckedStructuralParameterField,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<ByteSequenceStructuralField, LoweringError> {
    let (root, path, actual) = lower_structural_member_path(
        field.parameter_position,
        &field.path,
        parameters,
        structural_types,
    )?;
    if !matches!(actual, StructuralFieldType::ByteSequence(_)) {
        return unsupported("structural byte-sequence predicate leaf has the wrong retained type");
    }
    ByteSequenceStructuralField::new(root, path).map_err(LoweringError::InvalidCrashPredicate)
}

pub(crate) fn lower_structural_sum_subject(
    subject: &checked_trees::CheckedStructuralParameterField,
    parameters: &[StructuralParameterDeclaration],
    structural_types: &[StructuralTypeDeclaration],
) -> Result<(StructuralCaseSubject, StructuralTypeId), LoweringError> {
    let parameter = parameters
        .iter()
        .find(|parameter| parameter.position == subject.parameter_position)
        .ok_or(LoweringError::Unsupported(
            "structural sum predicate names a non-structural parameter",
        ))?;
    let mut structural_type = parameter.structural_type;
    let mut terminal_path = Vec::with_capacity(subject.path.len());
    let mut selected_case_fields = None;
    for segment in &subject.path {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
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
            let case = cases
                .iter()
                .find(|candidate| candidate.identity == *identity)
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
            let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                return unsupported("structural sum predicate index receiver is not a fixed array");
            };
            if *element_index >= *length {
                return unsupported("structural sum predicate fixed index is out of bounds");
            }
            terminal_path.push(CanonicalStructuralPathSegment::FixedIndex(*element_index));
            structural_type = *element;
            continue;
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            unreachable!("case and index paths handled above")
        };
        let fields = if let Some(fields) = selected_case_fields.take() {
            fields
        } else {
            match &declaration.shape {
                StructuralTypeShape::Record { fields }
                | StructuralTypeShape::Mixed { fields, .. } => fields,
                _ => {
                    return unsupported("structural sum predicate path receiver is not a record");
                }
            }
        };
        let field = fields
            .iter()
            .find(|candidate| candidate.identity == *identity)
            .filter(|field| !field.relevance.is_erased())
            .ok_or(LoweringError::Unsupported(
                "structural sum predicate path field is absent or erased",
            ))?;
        let StructuralFieldType::Structural(next) = field.field_type else {
            return unsupported("structural sum predicate path does not reach a structural value");
        };
        terminal_path.push(CanonicalStructuralPathSegment::Field(field.id));
        structural_type = next;
    }
    if selected_case_fields.is_some() {
        return unsupported("structural sum predicate case selection has no payload field");
    }
    if !matches!(
        structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .map(|declaration| &declaration.shape),
        Some(StructuralTypeShape::Sum { .. } | StructuralTypeShape::Mixed { .. })
    ) {
        return unsupported("structural sum predicate subject is not a sum");
    }
    Ok((
        StructuralCaseSubject::new(parameter.place, terminal_path),
        structural_type,
    ))
}
