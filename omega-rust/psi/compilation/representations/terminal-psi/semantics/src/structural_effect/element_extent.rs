//! Exact extent observations of established immutable element-view producers,
//! and the scalar leaf an element read yields.

use semantic_vocabulary::{
    IntegerSign, IntegerType, IntegerValue, Proposition, ScalarTerm, ScalarType, StructuralTypeId,
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
    let StructuralEffectObservation::ElementViewLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let StructuralEffectObservation::ElementViewSubslice {
        destination,
        start,
        end,
        ..
    } = producer
    else {
        return Ok(None);
    };
    if source != destination {
        return Ok(None);
    }
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let value = |identity| ScalarTerm::value(identity, ScalarType::Integer(integer_type));
    let difference = ScalarTerm::exact_integer_subtract(integer_type, value(*end), value(*start))
        .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(value(*result), difference)))
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
    let (StructuralEffectObservation::ElementViewLengthRead { source, result }
    | StructuralEffectObservation::ByteSequenceLengthRead { source, result }) = observation
    else {
        return Ok(None);
    };
    let StructuralEffectObservation::ElementViewEstablished {
        destination,
        region: terminal_psi::ElementViewRegion::WholeCollection,
        ..
    } = producer
    else {
        return Ok(None);
    };
    if source != destination {
        return Ok(None);
    }
    let integer_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64 is valid");
    let count = ScalarTerm::integer(
        integer_type,
        IntegerValue::Unsigned(u128::from(collection_length)),
    )
    .map_err(OperationSemanticError::InvalidProposition)?;
    Ok(Some(Proposition::Equal(
        ScalarTerm::value(*result, ScalarType::Integer(integer_type)),
        count,
    )))
}

/// The scalar an `ElementViewRead` yields at `path` below one element of type
/// `element`. An empty path reads a scalar element whole. Otherwise every step
/// but the last is a static structural projection (a record field holding a
/// structural child, or a literal index inside a fixed array's extent), and
/// the last step names the leaf: a non-erased scalar record field, or a
/// structural child or array element whose declaration is a primitive scalar.
/// Runtime indexes, referents and byte windows name no element leaf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ElementViewLeaf {
    pub scalar_type: ScalarType,
    pub bounds: Option<semantic_vocabulary::BoundedIntegerType>,
}
pub fn element_view_leaf(
    types: &[StructuralTypeDeclaration],
    element: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<ElementViewLeaf> {
    if path.iter().any(|segment| {
        !matches!(
            segment,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    let plain = |scalar_type| ElementViewLeaf {
        scalar_type,
        bounds: None,
    };
    let primitive = |identity: StructuralTypeId| {
        types
            .iter()
            .find(|row| row.id == identity)
            .and_then(|row| match row.shape {
                StructuralTypeShape::PrimitiveScalar(scalar) => Some(plain(scalar)),
                _ => None,
            })
    };
    let Some((last, prefix)) = path.split_last() else {
        return primitive(element);
    };
    let parent = crate::runtime_structural_path_tip(types.iter(), element, prefix)?;
    match (last, &parent.shape) {
        (StructuralPathSegment::Field(identity), StructuralTypeShape::Record { fields }) => {
            let field = fields
                .iter()
                .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
            match field.field_type {
                terminal_psi::StructuralFieldType::Structural(child) => primitive(child),
                terminal_psi::StructuralFieldType::BoundedInteger(bounds) => {
                    Some(ElementViewLeaf {
                        scalar_type: ScalarType::Integer(bounds.integer_type()),
                        bounds: Some(bounds),
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
            StructuralTypeShape::FixedArray { element, length },
        ) if index < length => primitive(*element),
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
    let leaf = element_view_leaf(types, element, path)?;
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).ok()?);
    let index = Proposition::LessThan(
        ScalarTerm::value(index, count),
        ScalarTerm::value(length, count),
    );
    let Some(bounds) = leaf.bounds else {
        return Some(index);
    };
    let stored = ScalarTerm::value(value, leaf.scalar_type);
    Some(Proposition::Conjunction(vec![
        index,
        Proposition::LessOrEqual(
            ScalarTerm::integer(bounds.integer_type(), bounds.minimum()).ok()?,
            stored.clone(),
        ),
        Proposition::LessOrEqual(
            stored,
            ScalarTerm::integer(bounds.integer_type(), bounds.maximum()).ok()?,
        ),
    ]))
}
