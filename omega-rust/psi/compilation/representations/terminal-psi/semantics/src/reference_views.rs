//! Reference formation keeps its original storage and states its observation type.
//!
//! Exact referent identity remains the aggregate contract. A shared primitive
//! reference may instead expose a total representation view: every source bit
//! pattern must be valid at the target. This is not source authority, lifetime,
//! qualification evidence or permission to write through the viewed carrier.

use semantic_vocabulary::{ScalarType, StructuralTypeId};

/// Exact byte footprint of a closed scalar storage interpretation. This is
/// independent of the logical reference type and grants no access authority.
pub fn reference_storage_byte_width(stored: ScalarType) -> Option<u8> {
    match stored {
        ScalarType::Integer(integer) if !integer.is_address() && integer.bits() % 8 == 0 => {
            u8::try_from(integer.bits() / 8).ok()
        }
        ScalarType::Boolean => Some(1),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(4),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(8),
        _ => None,
    }
}

/// Whether decoding every stored representation yields the logical carrier.
/// Narrow integers extend their physical interpretation, rather than reading
/// at the logical width. This predicate is not a write-fit proof: an exclusive
/// reference still needs the source check for each performed encoded write.
pub fn reference_storage_view_is_compatible(stored: ScalarType, logical: ScalarType) -> bool {
    if reference_storage_byte_width(stored).is_none() {
        return false;
    }
    if scalar_reference_view_is_total(stored, logical) {
        return true;
    }
    let (ScalarType::Integer(stored), ScalarType::Integer(logical)) = (stored, logical) else {
        return false;
    };
    if logical.is_address() || logical.bits() < stored.bits() {
        return false;
    }
    match (stored.sign(), logical.sign()) {
        (semantic_vocabulary::IntegerSign::Signed, semantic_vocabulary::IntegerSign::Unsigned) => {
            false
        }
        (semantic_vocabulary::IntegerSign::Unsigned, semantic_vocabulary::IntegerSign::Signed) => {
            logical.bits() > stored.bits()
        }
        _ => true,
    }
}

/// Check the closed source/view shape relationship without confusing it with
/// custody. Byte-region views begin at an actual byte endpoint; a nominal
/// encoded scalar keeps its original cell. The operation's checked geometry,
/// source access, bounds and loan lineage remain separate obligations.
pub fn reference_storage_source_compatible<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    reference: StructuralTypeId,
    stored: Option<ScalarType>,
) -> bool {
    let Some(StructuralTypeShape::Reference { referent, .. }) = types
        .clone()
        .find(|declaration| declaration.id == reference)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(StructuralTypeShape::PrimitiveScalar(source)) = types
        .clone()
        .find(|declaration| declaration.id == source)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let byte_endpoint = matches!(source, ScalarType::Integer(integer)
        if !integer.is_address() && integer.bits() == 8
            && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned);
    let Some(target) = types
        .find(|declaration| declaration.id == *referent)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    match (stored, target) {
        (Some(stored), StructuralTypeShape::PrimitiveScalar(logical)) => {
            (byte_endpoint
                || source == logical
                || scalar_reference_view_is_total(*source, *logical))
                && reference_storage_view_is_compatible(stored, *logical)
        }
        (None, StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {
            byte_endpoint
        }
        _ => false,
    }
}
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration, StructuralTypeShape};

use crate::scalar_representation_view::{integer_bits, integer_from_bits};
use crate::{ScalarRepresentationViewPair, ScalarViewValue};

/// Whether every representation of `source` is a valid observation at `target`.
/// Address carriers never acquire integer representation authority by width.
/// Integer-to-Boolean needs a landing proof and is not a total shared view.
/// Nonidentity integer views use the existing at-most-64-bit storage bit map.
pub fn scalar_reference_view_is_total(source: ScalarType, target: ScalarType) -> bool {
    if source == target {
        return true;
    }
    if let (ScalarType::Integer(source), ScalarType::Integer(target)) = (source, target) {
        return !source.is_address()
            && !target.is_address()
            && source.bits() == target.bits()
            && source.bits() <= 64;
    }
    ScalarRepresentationViewPair::classify(source, target)
        .is_some_and(|pair| !pair.requires_landing())
}

/// Observe a validated source scalar without modifying its backing value.
/// The bit map is shared with scalar representation operations; integer
/// sign changes use that owner's exact fixed-width bit interpretation.
pub fn scalar_reference_view_value(
    source: ScalarType,
    target: ScalarType,
    value: ScalarViewValue,
) -> Option<ScalarViewValue> {
    if !scalar_reference_view_is_total(source, target) {
        return None;
    }
    if source == target {
        return match (source, value) {
            (ScalarType::Boolean, ScalarViewValue::Boolean(_)) => Some(value),
            (ScalarType::Integer(integer), ScalarViewValue::Integer(value))
                if integer.admits(value) =>
            {
                Some(ScalarViewValue::Integer(value))
            }
            (ScalarType::IeeeFloat(format), ScalarViewValue::IeeeFloat(value))
                if value.format() == format =>
            {
                Some(ScalarViewValue::IeeeFloat(value))
            }
            _ => None,
        };
    }
    if let (
        ScalarType::Integer(source),
        ScalarType::Integer(target),
        ScalarViewValue::Integer(value),
    ) = (source, target, value)
    {
        return Some(ScalarViewValue::Integer(integer_from_bits(
            target,
            integer_bits(source, value)?,
        )));
    }
    ScalarRepresentationViewPair::classify(source, target)?.view(value)
}

/// Reconstruct one reference's source-to-observation representation contract.
/// `reference` names the result carrier, not its referent. Callers independently
/// validate source access, exact occurrence, qualifications and live backing.
pub fn reference_source_compatible<'types>(
    mut types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    reference: StructuralTypeId,
) -> bool {
    let Some(StructuralTypeShape::Reference { referent, access }) = types
        .clone()
        .find(|declaration| declaration.id == reference)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(source_shape) = types
        .clone()
        .find(|declaration| declaration.id == source)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    if source == *referent {
        return true;
    }
    if *access != StructuralAccess::SharedBorrow {
        return false;
    }
    let Some(StructuralTypeShape::PrimitiveScalar(target)) = types
        .find(|declaration| declaration.id == *referent)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    matches!(source_shape, StructuralTypeShape::PrimitiveScalar(source)
        if scalar_reference_view_is_total(*source, *target))
}
