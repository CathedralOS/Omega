//! Exact extent observations of established immutable element-view producers,
//! and the scalar leaf an element read yields.

use semantic_vocabulary::{
    IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

use super::StructuralEffectObservation;
use crate::OperationSemanticError;

/// Returns the subslice's direct extent denotation at a matching length read:
/// the measured element count is `end - start`. The caller must establish the
/// exact defining producer and its dominance; the producer's two bounds remain
/// independent required proof obligations. No equation is returned for other
/// producers or a different measured view.
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

/// Relate a measured element view to its whole collection's declared extent:
/// an `EstablishElementView` over a `FixedArray` names the array's static
/// element count. The caller must establish the exact producer/result join,
/// source type, and dominance before using this fact. A byte-length read uses
/// the same count only when validation established an exact `u8` collection
/// and byte-view result; no other element width becomes a byte extent.
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

/// The scalar an `ElementViewRead` yields at `path` below one element of type
/// `element`. An empty path reads a scalar element whole. Otherwise every step
/// but the last is a structural projection (a record field holding a
/// structural child, or an index inside a fixed array's extent), and
/// the last step names the leaf: a non-erased scalar record field, or a
/// structural child or array element whose declaration is a primitive scalar.
/// Runtime selectors carry separate operation obligations. Referents and byte
/// windows require their own storage/custody operations, not an inline leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElementViewLeaf {
    pub scalar_type: ScalarType,
    pub restriction: Option<terminal_psi::StructuralFieldType>,
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
                StructuralTypeShape::PrimitiveScalar(scalar) => match &row.scalar_restriction {
                    None => Some(plain(scalar)),
                    Some(terminal_psi::StructuralFieldType::BoundedInteger(bounds)) => {
                        Some(ElementViewLeaf {
                            scalar_type: scalar,
                            restriction: Some(terminal_psi::StructuralFieldType::BoundedInteger(
                                *bounds,
                            )),
                        })
                    }
                    Some(restriction @ terminal_psi::StructuralFieldType::RefinedScalar(_))
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
        (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields, .. }) => {
            let field = terminal_psi::live_field_by_identity(fields, *identity)?;
            match field.field_type {
                terminal_psi::StructuralFieldType::Structural(child) => primitive(child),
                terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                    Some(ElementViewLeaf {
                        scalar_type: ScalarType::Integer(bounds.integer_type()),
                        restriction: Some(terminal_psi::StructuralFieldType::BoundedInteger(
                            bounds,
                        )),
                    })
                }
                terminal_psi::StructuralFieldType::RefinedScalar(ref restriction) => {
                    Some(ElementViewLeaf {
                        scalar_type: restriction.carrier(),
                        restriction: Some(terminal_psi::StructuralFieldType::RefinedScalar(
                            restriction.clone(),
                        )),
                    })
                }
                terminal_psi::StructuralFieldType::Scalar(scalar) => Some(plain(scalar)),
                terminal_psi::StructuralFieldType::IeeeFloat(format) => {
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
/// The single write certificate covers both the exact element index and the
/// declared target leaf restriction. Formation grants no value-range evidence.
pub fn element_view_write_goal(
    types: &[StructuralTypeDeclaration],
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
    index: semantic_vocabulary::ValueId,
    length: semantic_vocabulary::ValueId,
    value: semantic_vocabulary::ValueId,
) -> Option<Proposition> {
    // Writes still use a single static descendant leaf. Dynamic writes need
    // independently carried descendant bounds, not this one certificate.
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
