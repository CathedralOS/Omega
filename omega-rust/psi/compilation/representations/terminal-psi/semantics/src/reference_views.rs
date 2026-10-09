//! Reference formation keeps its original storage and states its observation type.
//!
//! Psi establishes representation validity and access before publication. Fixed
//! aggregate storage views retain their byte region across subsequent views;
//! primitive views additionally require a total storage interpretation here.
//! Geometry is not source authority, lifetime or qualification evidence.

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

/// The byte footprint a thin foreign pointer may expose for a borrowed scalar
/// descriptor lent with `access`. A thin ABI carries no codec, so every
/// admitted backing of the loan must have exactly this footprint.
///
/// A shared loan can observe narrower backing through representation
/// implication: wider integer observations name narrower storage. Its
/// footprint is therefore fixed only when the logical type admits no
/// narrower interpretation.
///
/// A mutable loan received by a callee without a checked body cannot name
/// narrower backing. Mutable views require representation equivalence, and the
/// one narrower source, authored `IntegerAt` storage, admits only writes proved
/// to fit. Such a callee may perform any write of its formal's type, so Psi's
/// invocation admission rejects that loan before Terminal publication
/// (`fail/layouts/encoded_scalar_boundary_loan_requires_fit`). Its footprint
/// is the logical width. Write-only loans keep the shared rule until their
/// boundary admission is settled. This query grants neither access nor
/// lifetime authority and does not replace source/view admission.
pub fn reference_storage_export_byte_width(
    logical: ScalarType,
    access: StructuralAccess,
) -> Option<u8> {
    let width = reference_storage_byte_width(logical)?;
    if access == StructuralAccess::MutableBorrow {
        return Some(width);
    }
    let same_width = |stored| {
        !reference_storage_view_is_compatible(stored, logical)
            || reference_storage_byte_width(stored) == Some(width)
    };
    for stored in [
        ScalarType::Boolean,
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64),
    ] {
        if !same_width(stored) {
            return None;
        }
    }
    // Closed fixed scalar carriers cover byte-aligned integers through 128
    // bits. Check both signs; signedness is not a physical byte footprint.
    for bits in (8..=128).step_by(8) {
        for sign in [
            semantic_vocabulary::IntegerSign::Signed,
            semantic_vocabulary::IntegerSign::Unsigned,
        ] {
            let integer = semantic_vocabulary::IntegerType::new(sign, bits).ok()?;
            if !same_width(ScalarType::Integer(integer)) {
                return None;
            }
        }
    }
    Some(width)
}

/// Check the closed source/view shape relationship without confusing it with
/// custody. Byte-region views begin at an actual byte endpoint; a nominal
/// encoded scalar keeps its original cell. The operation's checked geometry,
/// source access, bounds and loan lineage remain separate obligations.
pub fn reference_storage_source_compatible<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
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
    let Some(source_shape) = types
        .clone()
        .find(|declaration| declaration.id == source)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let byte_endpoint = matches!(source_shape, StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer))
        if !integer.is_address() && integer.bits() == 8
            && integer.sign() == semantic_vocabulary::IntegerSign::Unsigned);
    let Some(target) = types
        .clone()
        .find(|declaration| declaration.id == *referent)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    match (stored, target) {
        (Some(stored), StructuralTypeShape::PrimitiveScalar(logical)) => {
            let StructuralTypeShape::PrimitiveScalar(source) = source_shape else {
                return false;
            };
            (byte_endpoint
                || source == logical
                || scalar_reference_view_is_total(*source, *logical))
                && reference_storage_view_is_compatible(stored, *logical)
        }
        (None, StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {
            byte_endpoint
                || plain_storage(types.clone(), source, &mut Vec::new())
                    && plain_storage(types.clone(), *referent, &mut Vec::new())
                    && crate::reference_storage_layout(types.clone(), source).is_some_and(
                        |layout| {
                            Some(layout)
                                == crate::reference_storage_layout(types.clone(), *referent)
                        },
                    )
        }
        _ => false,
    }
}

/// A checked aggregate re-view retains geometry, not another permission
/// representation. Psi owns its validity-set judgment; Terminal keeps nested
/// references, atomics and dynamic descriptors out of the plain byte route.
fn plain_storage<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    if active.len() >= 128 || active.contains(&root) {
        return false;
    }
    let Some(shape) = types
        .clone()
        .find(|row| row.id == root)
        .map(|row| &row.shape)
    else {
        return false;
    };
    active.push(root);
    let valid = match shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => {
            reference_storage_byte_width(*scalar).is_some()
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            plain_storage(types.clone(), *element, active)
        }
        StructuralTypeShape::Record { fields, .. } => fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .all(|field| match field.field_type {
                terminal_psi::StructuralFieldType::Structural(child) => {
                    plain_storage(types.clone(), child, active)
                }
                terminal_psi::StructuralFieldType::Atomic(_) => false,
                _ => field
                    .field_type
                    .scalar_type()
                    .is_some_and(|scalar| reference_storage_byte_width(scalar).is_some()),
            }),
        _ => false,
    };
    active.pop();
    valid
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
