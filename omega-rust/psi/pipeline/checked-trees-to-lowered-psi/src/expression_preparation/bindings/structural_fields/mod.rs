//! Bind source field identities to the exact emitted receiver declaration.

use super::super::{
    CheckedUnitStructuralPathSegment, StructuralFieldId, StructuralFieldType, StructuralTypeId,
    StructuralTypeShape,
};
use super::{
    LoweringError, PlaceId, ScalarType, StructuralAccess, StructuralParameterDeclaration,
    StructuralTypeDeclaration, unsupported,
};
#[cfg(test)]
mod tests;

#[derive(Clone)]
pub(crate) struct StructuralScalarFieldBinding {
    source_position: u32,
    source: PlaceId,
    structural_type: StructuralTypeId,
    declarations: std::sync::Arc<[StructuralTypeDeclaration]>,
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
    // Raw fixed arrays have static capacity, not bounded-owned live metadata.
    if !matches!(
        field.field_type,
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
    ) {
        return unsupported("byte field length requires a bounded-owned byte carrier");
    }
    Ok((binding.source, path, field.id))
}

pub(crate) fn resolve_primitive(
    fields: &[StructuralScalarFieldBinding],
    position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
    scalar_type: ScalarType,
) -> Result<
    (
        PlaceId,
        Vec<semantic_vocabulary::CanonicalStructuralPathSegment>,
    ),
    LoweringError,
> {
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
