use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

use super::StructuralEffectObservation;
use crate::OperationSemanticError;
use terminal_psi::StructuralFieldType;

pub fn element_subslice_length_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
) -> Result<Option<Proposition>, OperationSemanticError> {
    super::subslice_extent_equation(
        observation,
        producer,
        |observation| match observation {
            StructuralEffectObservation::ElementViewLengthRead { source, result } => {
                Some((source, *result))
            }
            _ => None,
        },
        |producer| match producer {
            StructuralEffectObservation::ElementViewSubslice {
                destination,
                start,
                end,
                ..
            } => Some((*destination, *start, *end)),
            _ => None,
        },
    )
}

pub fn element_establishment_length_equation(
    observation: &StructuralEffectObservation,
    producer: &StructuralEffectObservation,
    collection_length: u64,
) -> Result<Option<Proposition>, OperationSemanticError> {
    let (source, result) = match observation {
        StructuralEffectObservation::ElementViewLengthRead { source, result }
            if source.path.is_empty() =>
        {
            (source.place, *result)
        }
        StructuralEffectObservation::ByteSequenceLengthRead { source, result }
            if source.path.is_empty() =>
        {
            (source.place, *result)
        }
        _ => return Ok(None),
    };
    let StructuralEffectObservation::ElementViewEstablished {
        destination,
        region: terminal_psi::ElementViewRegion::WholeCollection,
        ..
    } = producer
    else {
        return Ok(None);
    };
    if source != *destination {
        return Ok(None);
    }
    super::u64_extent_proposition(result, u128::from(collection_length))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementViewLeaf {
    pub scalar_type: ScalarType,
    pub restriction: Option<StructuralFieldType>,
}
pub fn element_view_leaf(
    types: &[StructuralTypeDeclaration],
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ElementViewLeaf> {
    if path.iter().any(|segment| {
        !matches!(
            segment,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
        )
    }) {
        return None;
    }
    let plain = |scalar_type| ElementViewLeaf {
        scalar_type,
        restriction: None,
    };
    let primitive = |identity: StructuralTypeId| {
        types
            .iter()
            .find(|row| row.id == identity)
            .and_then(|row| match row.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => match &row
                    .scalar_restriction
                {
                    None => Some(plain(scalar)),
                    Some(StructuralFieldType::BoundedInteger(bounds)) => {
                        Some(ElementViewLeaf {
                            scalar_type: scalar,
                            restriction: Some(StructuralFieldType::BoundedInteger(*bounds)),
                        })
                    }
                    Some(restriction @ StructuralFieldType::RefinedScalar(_))
                        if restriction.scalar_type() == Some(scalar) =>
                    {
                        Some(ElementViewLeaf {
                            scalar_type: scalar,
                            restriction: Some(restriction.clone()),
                        })
                    }
                    _ => None,
                },
                _ => None,
            })
    };
    let Some((last, prefix)) = path.split_last() else {
        return primitive(element);
    };
    let parent = crate::runtime_structural_path_tip(types.iter(), element, prefix)?;
    match (last, &parent.shape) {
        (
            StructuralPathSegment::Field(identity),
            StructuralTypeShape::Record { fields, .. },
        ) => {
            let field = terminal_psi::live_field_by_identity(fields, *identity)?;
            match field.field_type {
                StructuralFieldType::Structural(child) => primitive(child),
                StructuralFieldType::BoundedInteger(bounds) => Some(ElementViewLeaf {
                    scalar_type: ScalarType::Integer(bounds.integer_type()),
                    restriction: Some(StructuralFieldType::BoundedInteger(bounds)),
                }),
                StructuralFieldType::RefinedScalar(ref restriction) => {
                    Some(ElementViewLeaf {
                        scalar_type: restriction.carrier(),
                        restriction: Some(StructuralFieldType::RefinedScalar(
                            restriction.clone(),
                        )),
                    })
                }
                StructuralFieldType::Scalar(scalar) => Some(plain(scalar)),
                StructuralFieldType::IeeeFloat(format) => {
                    Some(plain(ScalarType::IeeeFloat(format)))
                }
                _ => None,
            }
        }
        (
            StructuralPathSegment::FixedIndex(index),
            StructuralTypeShape::FixedArray {
                element,
                length: terminal_psi::ArrayExtent::Literal(length),
            },
        ) if index < length => primitive(*element),
        (
            StructuralPathSegment::RuntimeIndex { .. },
            StructuralTypeShape::FixedArray { element, .. },
        ) => primitive(*element),
        _ => None,
    }
}
pub fn element_view_leaf_scalar(
    types: &[StructuralTypeDeclaration],
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ScalarType> {
    element_view_leaf(types, element, path).map(|leaf| leaf.scalar_type)
}
/// A by-value read copies a statically selected scalar from its element.
/// Only that scalar leaf is copied, so a non-copy element may supply
/// one of its scalar fields; reading the whole element is a different
/// operation that its own multiplicity governs.
pub fn element_view_read_scalar(
    types: &[StructuralTypeDeclaration],
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ScalarType> {
    terminal_psi::find_structural_type(types, element)?;
    if !path.iter().all(|segment| {
        matches!(
            segment,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    element_view_leaf_scalar(types, element, path)
}
pub fn element_view_write_goal(
    types: &[StructuralTypeDeclaration],
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
    index: semantic_vocabulary::ValueId,
    length: semantic_vocabulary::ValueId,
    value: semantic_vocabulary::ValueId,
) -> Option<Proposition> {
    if path.iter().any(|segment| segment.runtime_index().is_some()) {
        return None;
    }
    let leaf = element_view_leaf(types, element, path)?;
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).ok()?);
    let index = Proposition::LessThan(
        ScalarTerm::value(index, count),
        ScalarTerm::value(length, count),
    );
    match leaf.restriction {
        None => Some(index),
        Some(restriction) => Some(Proposition::Conjunction(vec![
            index,
            crate::scalar_field_requirement(&restriction, value)?,
        ])),
    }
}
