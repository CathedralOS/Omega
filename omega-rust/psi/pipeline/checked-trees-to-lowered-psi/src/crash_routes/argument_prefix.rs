//! Rebase callee crash paths onto the exact caller structural argument source.

use crate::attached_unit::primitive_locals::PrimitiveLocal;
use crate::{LoweringError, unsupported};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralPlaceKind};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralFieldType,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

pub(crate) fn structural_crash_route_argument_prefix(
    argument: &StructuralArgument,
    parameters: &[StructuralParameterDeclaration],
    trivial_affine_locals: &[StructuralPlaceDeclaration],
    structural_results: &[(StructuralPlaceDeclaration, bool)],
    structural_types: &[StructuralTypeDeclaration],
    primitive_locals: &[PrimitiveLocal],
) -> Result<Vec<CanonicalStructuralPathSegment>, LoweringError> {
    let mut structural_type = parameters
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
                .map(|(place, _)| place)
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
    let mut prefix = Vec::with_capacity(argument.path.len());
    for (position, segment) in argument.path.iter().enumerate() {
        let declaration = structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)
            .ok_or(LoweringError::Unsupported(
                "structural crash route argument path type is absent",
            ))?;
        match segment {
            StructuralPathSegment::Field(identity) => {
                let fields = match &declaration.shape {
                    StructuralTypeShape::Record { fields }
                    | StructuralTypeShape::Mixed { fields, .. } => fields,
                    _ => {
                        return unsupported(
                            "structural crash route argument path receiver is not a record",
                        );
                    }
                };
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity)
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
    Ok(prefix)
}
