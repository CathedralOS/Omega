//! Bind source field identities to the exact emitted receiver declaration.

use super::super::{
    CheckedUnitStructuralPathSegment, StructuralFieldId, StructuralFieldType, StructuralTypeId,
    StructuralTypeShape,
};
use super::{
    LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, unsupported,
};
use semantic_vocabulary::StructuralCaseId;
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct StructuralScalarFieldBinding {
    source_position: u32,
    source: PlaceId,
    structural_type: StructuralTypeId,
    declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
    case_payloads: CasePayloadAccess,
}

/// Whether a read through a case segment may observe this root's payload.
///
/// Terminal has no case-qualified place read: a payload is observable only as
/// a parameter of the block a `StructuralCase` dispatch selected for that case,
/// so the verifier's own tag dispatch is the case knowledge. A read therefore
/// resolves to an established payload value, is deferred for the guard
/// decision lowering that will introduce the dispatch, or is refused.
#[derive(Clone, Default)]
enum CasePayloadAccess {
    #[default]
    Unavailable,
    Deferred,
    Established(Vec<EstablishedCasePayload>),
}

/// One payload field bound by a dominating `StructuralCase` successor, at its
/// dense position in the scalar namespace of the code it dominates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EstablishedCasePayload {
    pub(crate) source: PlaceId,
    pub(crate) case: StructuralCaseId,
    pub(crate) field: StructuralFieldId,
    pub(crate) position: usize,
}

/// A resolved case-qualified payload read.
pub(crate) enum CasePayloadRead {
    /// Awaiting a dispatch on `source` selecting `case`; the lowered read keeps
    /// the canonical `[Case, Field]` path until that dispatch substitutes it.
    Deferred {
        source: PlaceId,
        case: StructuralCaseId,
        field: StructuralFieldId,
    },
    /// The dominating dispatch's payload parameter.
    Established { position: usize },
}

/// Let guard lowering keep case-qualified reads for its dispatch to bind.
pub(crate) fn defer_case_payloads(fields: &mut [StructuralScalarFieldBinding]) {
    for field in fields {
        field.case_payloads = CasePayloadAccess::Deferred;
    }
}

/// Bind reads of payloads a dominating dispatch selected on every incoming path.
pub(crate) fn establish_case_payloads(
    fields: &mut [StructuralScalarFieldBinding],
    established: &[EstablishedCasePayload],
) {
    for field in fields {
        let rows = established
            .iter()
            .filter(|row| row.source == field.source)
            .copied()
            .collect::<Vec<_>>();
        if !rows.is_empty() {
            field.case_payloads = CasePayloadAccess::Established(rows);
        }
    }
}

impl StructuralScalarFieldBinding {
    pub(crate) fn collect(
        parameters: &[(u32, StructuralParameterDeclaration)],
        types: &[StructuralTypeDeclaration],
    ) -> Vec<Self> {
        // Share declarations between roots and resolve only observed paths. A
        // recursive leaf inventory would expand unused nested record contents.
        let declarations: std::sync::Arc<[StructuralTypeDeclaration]> = types.into();
        let mut bindings = Vec::new();
        for (position, parameter) in parameters {
            if !matches!(
                parameter.access,
                StructuralAccess::Owned
                    | StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
            ) {
                continue;
            }
            bindings.push(StructuralScalarFieldBinding {
                source_position: *position,
                source: parameter.place,
                structural_type: parameter.structural_type,
                declarations: std::sync::Arc::clone(&declarations),
                case_payloads: CasePayloadAccess::Unavailable,
            });
        }
        bindings
    }
}

pub(crate) fn resolve(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<
    (
        PlaceId,
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
        StructuralFieldId,
    ),
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "runtime field observation has no exact readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("runtime field observation has ambiguous bindings");
    }
    let mut structural_type = binding.structural_type;
    let mut carrier_path = Vec::with_capacity(path.len().saturating_sub(1));
    let mut visited = Vec::new();
    for (ordinal, segment) in path.iter().enumerate() {
        if visited.contains(&structural_type) {
            return unsupported("runtime field observation has a cyclic carrier");
        }
        visited.push(structural_type);
        let mut declarations = binding
            .declarations
            .iter()
            .filter(|declaration| declaration.id == structural_type);
        let declaration = declarations.next().ok_or(LoweringError::Unsupported(
            "runtime field observation lost its carrier declaration",
        ))?;
        if declarations.next().is_some() {
            return unsupported("runtime field observation has ambiguous carrier declarations");
        }
        if let checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index) =
            segment
        {
            // One literal fixed-array index may end the carrier, mirroring
            // the bounded store projection grammar: the leaf field still
            // belongs to the element's own record declaration.
            if ordinal + 2 != path.len() {
                return unsupported(
                    "runtime field observation requires a field leaf after its index",
                );
            }
            let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
                return unsupported("runtime field observation indexes a non-array carrier");
            };
            if *element_index >= *length {
                return unsupported("runtime field observation fixed index is out of bounds");
            }
            carrier_path.push(
                semantic_vocabulary::CanonicalStructuralPathSegment::FixedIndex(*element_index),
            );
            structural_type = *element;
            continue;
        }
        let checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) = segment else {
            return unsupported("runtime field observation requires a record-only field path");
        };
        let StructuralTypeShape::Record { fields } = &declaration.shape else {
            return unsupported("runtime field observation requires a record carrier");
        };
        let mut selected = fields.iter().filter(|field| field.identity == *identity);
        let field = selected.next().ok_or(LoweringError::Unsupported(
            "runtime field observation lost its declared field",
        ))?;
        if selected.next().is_some()
            || field.relevance.is_erased()
            || fields
                .iter()
                .filter(|candidate| candidate.id == field.id)
                .count()
                != 1
        {
            return unsupported("runtime field observation has an erased or ambiguous field");
        }
        if ordinal + 1 == path.len() {
            let declared = match field.field_type {
                StructuralFieldType::Scalar(scalar) => scalar,
                StructuralFieldType::BoundedInteger(integer) => {
                    ScalarType::Integer(integer.integer_type())
                }
                _ => return unsupported("runtime field observation requires a scalar leaf"),
            };
            if declared != scalar_type {
                return unsupported("runtime field observation changes its declared scalar type");
            }
            return Ok((binding.source, carrier_path, field.id));
        }
        let StructuralFieldType::Structural(child) = field.field_type else {
            return unsupported("runtime field observation requires a structural carrier field");
        };
        carrier_path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
            field.id,
        ));
        structural_type = child;
    }
    unsupported("runtime field observation requires a nonempty field path")
}

/// Resolve `[Case(case), Field(field)]` below one sum root. The selected case
/// must belong to the root's exact declared sum and the leaf must be a
/// relevant scalar payload of the requested carrier; the case itself is known
/// only through the binding's `CasePayloadAccess`.
pub(crate) fn resolve_case_payload(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<CasePayloadRead, LoweringError> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "case payload observation has no exact readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("case payload observation has ambiguous bindings");
    }
    let [
        checked_trees::CheckedStructuralPredicatePathSegment::Case(case_identity),
        checked_trees::CheckedStructuralPredicatePathSegment::Field(field_identity),
    ] = path
    else {
        return unsupported("case payload observation requires one case and one payload field");
    };
    let mut declarations = binding
        .declarations
        .iter()
        .filter(|declaration| declaration.id == binding.structural_type);
    let declaration = declarations.next().ok_or(LoweringError::Unsupported(
        "case payload observation lost its sum declaration",
    ))?;
    if declarations.next().is_some() {
        return unsupported("case payload observation has ambiguous sum declarations");
    }
    let (StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. }) =
        &declaration.shape
    else {
        return unsupported("case payload observation requires a sum root");
    };
    let mut selected = cases.iter().filter(|case| case.identity == *case_identity);
    let case = selected.next().ok_or(LoweringError::Unsupported(
        "case payload observation lost its declared case",
    ))?;
    let mut payload = case
        .fields
        .iter()
        .filter(|field| field.identity == *field_identity);
    let field = payload.next().ok_or(LoweringError::Unsupported(
        "case payload observation lost its declared payload field",
    ))?;
    if selected.next().is_some() || payload.next().is_some() || field.relevance.is_erased() {
        return unsupported("case payload observation has an erased or ambiguous payload");
    }
    let declared = match field.field_type {
        StructuralFieldType::Scalar(scalar) => scalar,
        StructuralFieldType::BoundedInteger(integer) => ScalarType::Integer(integer.integer_type()),
        _ => return unsupported("case payload observation requires a scalar payload"),
    };
    if declared != scalar_type {
        return unsupported("case payload observation changes its declared scalar type");
    }
    match &binding.case_payloads {
        CasePayloadAccess::Deferred => Ok(CasePayloadRead::Deferred {
            source: binding.source,
            case: case.id,
            field: field.id,
        }),
        CasePayloadAccess::Established(rows) => rows
            .iter()
            .find(|row| row.case == case.id && row.field == field.id)
            .map(|row| CasePayloadRead::Established {
                position: row.position,
            })
            .ok_or(LoweringError::Unsupported(
                "case payload observation requires an established case",
            )),
        CasePayloadAccess::Unavailable => {
            unsupported("case payload observation requires an established case")
        }
    }
}

pub(crate) fn resolve_byte_length(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    (
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        StructuralFieldId,
    ),
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "byte field length has no readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("byte field length has ambiguous bindings");
    }
    let Some((checked_trees::CheckedStructuralPredicatePathSegment::Field(identity), carrier)) =
        path.split_last()
    else {
        return unsupported("byte field length requires an exact field endpoint");
    };
    let carrier = carrier
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            _ => unsupported("byte field length has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (path, field) = crate::emission::structural_scalar_store::lower_structural_field_path(
        binding.structural_type,
        &carrier,
        identity,
        &binding.declarations,
    )?;
    // Raw fixed arrays have static capacity, not live metadata; a borrowed
    // view leaf carries its extent on the view descriptor instead.
    if !matches!(
        field.field_type,
        StructuralFieldType::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                | terminal_psi::ByteSequenceCarrier::BorrowedView
        )
    ) {
        return unsupported("byte field length requires a bounded-owned byte carrier");
    }
    Ok((binding.source, path, field.id))
}

/// Resolve an indexed read that lands on a fixed-array record field: the path
/// reaches the array itself and carries its declared element scalar. A leaf
/// that is not a fixed array returns `None`, so byte-sequence carriers keep
/// `resolve_byte_length`'s read family.
pub(crate) fn resolve_indexed_array(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> Result<
    Option<(
        PlaceId,
        Vec<terminal_psi::StructuralPathSegment>,
        ScalarType,
    )>,
    LoweringError,
> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "indexed field read has no readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("indexed field read has ambiguous bindings");
    }
    let carrier = path
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            _ => unsupported("indexed field read has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    crate::emission::primitive_store::lower_indexed_path(
        binding.structural_type,
        &carrier,
        &binding.declarations,
    )
    .map(|(path, scalar_type)| Some((binding.source, path, scalar_type)))
    .or(Ok(None))
}

pub(crate) fn resolve_primitive(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<(PlaceId, Vec<terminal_psi::StructuralPathSegment>), LoweringError> {
    let mut matching = fields
        .iter()
        .filter(|field| field.source_position == position);
    let binding = matching.next().ok_or(LoweringError::Unsupported(
        "primitive observation has no readable binding",
    ))?;
    if matching.next().is_some() {
        return unsupported("primitive observation has ambiguous bindings");
    }
    let path = path
        .iter()
        .map(|segment| match segment {
            checked_trees::CheckedStructuralPredicatePathSegment::Field(identity) => {
                Ok(CheckedUnitStructuralPathSegment::Field(identity.clone()))
            }
            checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(index) => {
                Ok(CheckedUnitStructuralPathSegment::FixedIndex(*index))
            }
            _ => unsupported("primitive observation has an unsupported case path"),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (path, declared_type) = crate::emission::primitive_store::lower_path(
        binding.structural_type,
        &path,
        &binding.declarations,
    )?;
    if declared_type != scalar_type {
        return unsupported("primitive observation changes its declared scalar type");
    }
    Ok((binding.source, path))
}
