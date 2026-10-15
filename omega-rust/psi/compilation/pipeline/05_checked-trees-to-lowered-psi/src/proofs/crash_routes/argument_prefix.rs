//! Rebase callee crash paths onto the exact caller structural argument source.

use crate::lowering_error::LoweringError;
use crate::lowering_error::unsupported;
use crate::unit::attached_unit::primitive_locals::PrimitiveLocal;
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralPlaceKind};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape, find_structural_type,
};

pub(crate) fn structural_crash_route_argument_prefix(
    argument: &StructuralArgument,
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(Option<StructuralPlaceDeclaration>, bool)],
    structural_types: &[StructuralTypeDeclaration],
    primitive_locals: &[PrimitiveLocal],
) -> Result<Vec<CanonicalStructuralPathSegment>, LoweringError> {
    let structural_type = parameters
        .iter()
        .find(|parameter| parameter.place == argument.place)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            trivial_affine_locals.iter().find_map(|local| {
                (local.id == argument.place)
                    .then_some(match local.kind {
                        StructuralPlaceKind::TrivialAffineLocal {
                            structural_type, ..
                        } => Some(structural_type),
                        _ => None,
                    })
                    .flatten()
            })
        })
        .or_else(|| {
            structural_results
                .iter()
                .filter_map(|(place, _)| place.as_ref())
                // These are the established caller referents, with their original producers.
                .chain(primitive_locals.iter().map(|local| &local.declaration))
                .find_map(|local| {
                    (local.id == argument.place)
                        .then_some(match local.kind {
                            StructuralPlaceKind::OperationResult {
                                structural_type, ..
                            } => Some(structural_type),
                            _ => None,
                        })
                        .flatten()
                })
        })
        .ok_or(LoweringError::Unsupported(
            "structural crash route argument has no caller structural source",
        ))?;
    structural_crash_route_prefix_from_type(argument, structural_type, structural_types)
}

pub(crate) fn structural_crash_route_prefix_from_type(
    argument: &StructuralArgument,
    mut structural_type: semantic_vocabulary::StructuralTypeId,
    structural_types: &[StructuralTypeDeclaration],
) -> Result<Vec<CanonicalStructuralPathSegment>, LoweringError> {
    let mut prefix = Vec::with_capacity(argument.path.len());
    let mut selected_case: Option<&terminal_psi::StructuralCaseDeclaration> = None;
    for (position, segment) in argument.path.iter().enumerate() {
        let declaration = find_structural_type(structural_types, structural_type).ok_or(
            LoweringError::Unsupported("structural crash route argument path type is absent"),
        )?;
        if selected_case.is_some() && !matches!(segment, StructuralPathSegment::Field(_)) {
            return unsupported("structural crash route case must select its payload field");
        }
        match segment {
            StructuralPathSegment::AllElements => {
                return unsupported("universal qualification is not a crash operand path");
            }
            StructuralPathSegment::RepresentationView { .. } => {
                return unsupported(
                    "returned view origins cannot create canonical crash predicate places",
                );
            }
            StructuralPathSegment::Case(selected) => {
                let cases = match &declaration.shape {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return unsupported("structural crash route case has no source sum"),
                };
                let mut matches = cases.iter().filter(|case| case.id == *selected);
                let case = matches.next().ok_or(LoweringError::Unsupported(
                    "structural crash route case belongs to another declaration",
                ))?;
                if matches.next().is_some() {
                    return unsupported("structural crash route case is ambiguous");
                }
                selected_case = Some(case);
                prefix.push(CanonicalStructuralPathSegment::Case(case.id));
            }
            StructuralPathSegment::FixedByteRange { .. } => {
                return unsupported(
                    "byte window crash predicates require exact view extent custody",
                );
            }
            StructuralPathSegment::Referent => {
                return unsupported(
                    "reference crash paths require retained referent projection custody",
                );
            }
            StructuralPathSegment::RuntimeIndex { .. } => {
                return unsupported(
                    "runtime-indexed crash predicates have no canonical element prefix",
                );
            }
            StructuralPathSegment::Field(identity) => {
                let fields = if let Some(case) = selected_case.take() {
                    &case.fields
                } else {
                    match &declaration.shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                        _ => {
                            return unsupported(
                                "structural crash route argument path receiver is not a record",
                            );
                        }
                    }
                };
                let field = terminal_psi::field_by_identity(fields, *identity)
                    .filter(|field| !field.relevance.is_erased())
                    .ok_or(LoweringError::Unsupported(
                        "structural crash route argument field is absent or erased",
                    ))?;
                prefix.push(CanonicalStructuralPathSegment::Field(field.id));
                match field.field_type {
                    StructuralFieldType::Structural(next) => structural_type = next,
                    StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        ..
                    }) if position + 1 == argument.path.len()
                        && argument.access == StructuralAccess::MutableBorrow =>
                    {
                        return Ok(prefix);
                    }
                    _ => {
                        return unsupported(
                            "structural crash route argument field is not structural",
                        );
                    }
                }
            }
            StructuralPathSegment::FixedIndex(index) => {
                let StructuralTypeShape::FixedArray { element, length } = declaration.shape else {
                    return unsupported(
                        "structural crash route argument fixed index receiver is not an array",
                    );
                };
                if *index >= length {
                    return unsupported(
                        "structural crash route argument fixed index is out of bounds",
                    );
                }
                prefix.push(CanonicalStructuralPathSegment::FixedIndex(*index));
                structural_type = element;
            }
        }
    }
    if selected_case.is_some() {
        return unsupported("structural crash route case has no payload field");
    }
    Ok(prefix)
}
