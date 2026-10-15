//! Reference formation keeps its original storage and states its observation type.
//!
//! Psi establishes representation validity and access before publication. Fixed
//! aggregate storage views retain their byte region across subsequent views;
//! primitive views additionally require a total storage interpretation here.
//! Geometry is not source authority, lifetime or qualification evidence.

use semantic_vocabulary::{ScalarType, StructuralTypeId};

/// Materialize a shared element descriptor without copying its backing.
/// Stored descriptors carry their own permission; a canonical source needs
/// the access of its authenticated occurrence. Readability alone cannot copy
/// an exclusive descriptor. Availability and result custody remain separate.
pub fn shared_descriptor_copy_matches<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    occurrence_access: Option<StructuralAccess>,
    destination: StructuralTypeId,
) -> bool {
    let Some(StructuralTypeShape::ElementView {
        element: source_element,
        access: source_access,
    }) = types
        .clone()
        .find(|row| row.id == source)
        .map(|row| &row.shape)
    else {
        return false;
    };
    let Some(StructuralTypeShape::ElementView {
        element: destination_element,
        access: Some(StructuralAccess::SharedBorrow),
    }) = types
        .clone()
        .find(|row| row.id == destination)
        .map(|row| &row.shape)
    else {
        return false;
    };
    source_element == destination_element
        && source_access.or(occurrence_access) == Some(StructuralAccess::SharedBorrow)
}

/// Shared references can be copied even when their materialized carriers have
/// affine retirement custody. Fixed arrays inherit this distinction from their
/// elements, including nested arrays; the referent itself is never copied.
/// Nominal payloads do not inherit copy permission from their physical fields
/// or cleanup metadata. Their ordinary multiplicity remains authoritative.
/// This declaration judgment does not establish live backing, fresh carrier
/// identity, memberships or parent loans; each copy still checks those.
pub fn is_shared_reference_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    mut structural_type: StructuralTypeId,
) -> bool {
    // Every step descends one stored array element. A walk longer than the
    // catalog is cyclic; absent or ambiguous identities also fail closed.
    for _ in 0..types.clone().count() {
        let mut matching = types.clone().filter(|row| row.id == structural_type);
        let Some(declaration) = matching.next() else {
            return false;
        };
        if matching.next().is_some() || declaration.quotient_carrier.is_some() {
            return false;
        }
        match declaration.shape {
            StructuralTypeShape::Reference { access, .. } => {
                return access == StructuralAccess::SharedBorrow;
            }
            StructuralTypeShape::ElementView {
                access: Some(access),
                ..
            } => {
                return access == StructuralAccess::SharedBorrow;
            }
            StructuralTypeShape::FixedArray { element, .. } => structural_type = element,
            _ => return false,
        }
    }
    false
}

/// Resolve the declaration of one retained storage interpretation. This checks
/// its vocabulary and codec only, never source representation validity, extent,
/// access or lifetime. Result custody must match it to an actual admitted
/// formation before the path can describe a returned loan.
pub fn representation_view_declaration<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    view: StructuralTypeId,
    stored: Option<ScalarType>,
) -> Option<&'types StructuralTypeDeclaration> {
    let mut matching = types.filter(|declaration| declaration.id == view);
    let declaration = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    match (stored, &declaration.shape) {
        (Some(stored), StructuralTypeShape::PrimitiveScalar(logical))
            if reference_storage_view_is_compatible(stored, *logical) => {}
        (None, StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {}
        _ => return None,
    }
    Some(declaration)
}

/// Reference provenance may end in a marker naming ordinary borrowed storage.
/// Only a reference-typed endpoint crosses a descriptor there. Formation and
/// result-origin consumers use this same geometry before resolving live loans;
/// internal crossings, access, bounds and backing identity remain their checks.
/// Unknown geometry retains the original path so it cannot bypass validation.
pub fn reference_source_storage_path<'types, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &'path [terminal_psi::StructuralPathSegment],
) -> &'path [terminal_psi::StructuralPathSegment] {
    if let Some((terminal_psi::StructuralPathSegment::Referent, prefix)) = path.split_last()
        && (crate::runtime_structural_path_tip(types.clone(), root, prefix).is_some_and(|tip| {
            !matches!(
                tip.shape,
                terminal_psi::StructuralTypeShape::Reference { .. }
            )
        }) || crate::runtime_structural_field(types, root, prefix)
            .is_some_and(|field| field.field_type.byte_sequence_carrier().is_some()))
    {
        prefix
    } else {
        path
    }
}

/// Join an exact projected storage endpoint to a reference representation.
/// Inline owners have no nominal leaf type: compare their carrier directly
/// without replacing the root or inventing a catalog declaration. Membership,
/// access and lifetime admission remain independent of this geometry.
pub fn reference_source_projection_compatible<'types>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    reference: StructuralTypeId,
) -> bool {
    let path = reference_source_storage_path(types.clone(), root, path);
    // Shared formation observes a restricted inline scalar through its read
    // carrier. It does not require a standalone declaration for that field,
    // nor may it manufacture a stronger receiving restriction. Exclusive
    // formation still retains the exact receiving contract below.
    if let Some(StructuralTypeShape::Reference {
        access: StructuralAccess::SharedBorrow,
        referent,
    }) = types
        .clone()
        .find(|row| row.id == reference)
        .map(|row| &row.shape)
        && let Some(target) = types.clone().find(|row| row.id == *referent)
        && matches!(target.shape, StructuralTypeShape::PrimitiveScalar(_))
    {
        let Some(source) = crate::borrowed_primitive_projection_declaration(
            types.clone(),
            root,
            path,
            Some(*referent),
        ) else {
            return false;
        };
        return (source == *referent || target.scalar_restriction.is_none())
            && reference_source_compatible(types, source, reference);
    }
    if let Some(source) = crate::runtime_structural_path_tip(types.clone(), root, path) {
        return reference_source_compatible(types, source.id, reference);
    }
    if let Some(source) = crate::primitive_projection_declaration(types.clone(), root, path) {
        return reference_source_compatible(types, source, reference);
    }
    let Some(field) = crate::runtime_structural_field(types.clone(), root, path) else {
        return false;
    };
    let Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity }) =
        field.field_type.byte_sequence_carrier()
    else {
        return false;
    };
    let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) = types
        .clone()
        .find(|row| row.id == reference)
        .map(|row| &row.shape)
    else {
        return false;
    };
    types
        .clone()
        .find(|row| row.id == *referent)
        .is_some_and(|row| {
            row.shape
                == terminal_psi::StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                )
        })
}

/// Authenticate declared access independently of the selected endpoint's
/// nominal identity. Owning a shared descriptor does not own its backing;
/// reference and indexed-view crossings retain their own access ceilings.
/// Availability, active cases, bounds, memberships and live loans are separate
/// operation judgments, not consequences of this declaration walk.
pub fn reference_source_projection_accessible<'types>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
    mut authority: terminal_psi::StructuralAccess,
    path: &[terminal_psi::StructuralPathSegment],
    requested: terminal_psi::StructuralAccess,
) -> bool {
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralPathSegment, StructuralTypeShape,
    };
    if authority == StructuralAccess::Owned
        && multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
        && crate::descriptor_results::is_shared_descriptor(types.clone(), root)
    {
        authority = StructuralAccess::SharedBorrow;
    }
    if !requested.admits(authority) {
        return false;
    }
    let path = reference_source_storage_path(types.clone(), root, path);
    let mut region_start = 0;
    for (position, segment) in path.iter().enumerate() {
        let prefix = &path[..position];
        match segment {
            StructuralPathSegment::Referent => {
                let Some(StructuralTypeShape::Reference { access, .. }) =
                    crate::runtime_structural_path_tip(types.clone(), root, prefix)
                        .map(|declaration| &declaration.shape)
                else {
                    return false;
                };
                if !StructuralAccess::SharedBorrow.admits(authority)
                    || !requested.admits_referent(*access)
                {
                    return false;
                }
                authority = *access;
                region_start = position + 1;
            }
            StructuralPathSegment::FixedIndex(_) | StructuralPathSegment::RuntimeIndex { .. } => {
                let declaration = crate::runtime_structural_path_tip(types.clone(), root, prefix);
                if let Some(StructuralTypeShape::ElementView { access, .. }) =
                    declaration.map(|row| &row.shape)
                {
                    match access {
                        Some(access) if !requested.admits_referent(*access) => return false,
                        // Canonical descriptors rely on an explicit occurrence;
                        // storing that geometry alone grants no mutable loan.
                        None if position != region_start
                            && requested != StructuralAccess::SharedBorrow =>
                        {
                            return false;
                        }
                        _ => {}
                    }
                }
                // Inline fields carry their own access contract. Their
                // canonical geometry declaration can erase that access, so it
                // must not replace the selected field's backing authority.
                let carrier = crate::runtime_structural_field(types.clone(), root, prefix)
                    .and_then(|field| field.field_type.byte_sequence_carrier())
                    .or_else(|| {
                        declaration.and_then(|row| match row.shape {
                            StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                            _ => None,
                        })
                    });
                if let Some(
                    ByteSequenceCarrier::BorrowedView {
                        access: Some(access),
                    }
                    | ByteSequenceCarrier::BoundedView {
                        access: Some(access),
                        ..
                    },
                ) = carrier
                    && !requested.admits_referent(access)
                {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

/// Permission to use a selected descriptor's backing, rather than to borrow
/// or replace its storage slot. Container ownership cannot upgrade a stored
/// shared view; canonical descriptors rely on their explicit root/reference
/// occurrence. Availability and overlapping loans remain separate judgments.
pub fn descriptor_source_projection_accessible<'types>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
    authority: terminal_psi::StructuralAccess,
    path: &[terminal_psi::StructuralPathSegment],
    requested: terminal_psi::StructuralAccess,
) -> bool {
    use terminal_psi::{
        ByteSequenceCarrier, StructuralAccess, StructuralPathSegment, StructuralTypeShape,
    };
    if !reference_source_projection_accessible(
        types.clone(),
        root,
        multiplicity,
        authority,
        path,
        requested,
    ) {
        return false;
    }
    let path = reference_source_storage_path(types.clone(), root, path);
    let region_start = path
        .iter()
        .rposition(|segment| *segment == StructuralPathSegment::Referent)
        .map_or(0, |position| position + 1);
    if let Some(StructuralTypeShape::ElementView { access, .. }) =
        crate::runtime_structural_path_tip(types.clone(), root, path).map(|row| &row.shape)
    {
        match access {
            Some(access) if !requested.admits_referent(*access) => return false,
            None if path.len() != region_start && requested != StructuralAccess::SharedBorrow => {
                return false;
            }
            _ => {}
        }
    }
    let carrier = crate::runtime_structural_field(types.clone(), root, path)
        .and_then(|field| field.field_type.byte_sequence_carrier())
        .or_else(|| {
            crate::runtime_structural_path_tip(types.clone(), root, path).and_then(|row| match row
                .shape
            {
                StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                _ => None,
            })
        });
    if let Some(
        ByteSequenceCarrier::BorrowedView { access }
        | ByteSequenceCarrier::BoundedView { access, .. },
    ) = carrier
    {
        match access {
            Some(access) if !requested.admits_referent(access) => return false,
            None if path.len() != region_start && requested != StructuralAccess::SharedBorrow => {
                return false;
            }
            _ => {}
        }
    }
    true
}

/// An inline bounded owner has predicate storage, not a nominal membership
/// roster. Capturing its address may present those predicates as primitive,
/// route-free referent domains. Exclusive loans must preserve the complete
/// field invariant; shared loans may observe a weaker contract. This relation
/// neither proves live access nor creates an invariant window for field writes.
pub fn reference_byte_field_compatible<'types, 'domains>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domains terminal_psi::StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    reference: StructuralTypeId,
    memberships: &[terminal_psi::StructuralPathQualification],
) -> bool {
    let Some(terminal_psi::StructuralTypeShape::Reference { referent, access }) = types
        .clone()
        .find(|declaration| declaration.id == reference)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(terminal_psi::StructuralTypeShape::ByteSequence(
        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
    )) = types
        .clone()
        .find(|declaration| declaration.id == *referent)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(field) = crate::runtime_structural_field(types, root, path) else {
        return false;
    };
    if field.field_type.byte_sequence_carrier()
        != Some(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            capacity: *capacity,
        })
        || memberships
            .iter()
            .any(|row| row.path != [terminal_psi::StructuralPathSegment::Referent])
    {
        return false;
    }
    let qualifications = memberships.iter().map(|row| row.domain).collect::<Vec<_>>();
    let Some(required) =
        crate::byte_qualification_predicates_in(domains, *referent, &qualifications)
    else {
        return false;
    };
    let predicates = field.field_type.byte_predicates();
    predicates.implies(required)
        && (*access == terminal_psi::StructuralAccess::SharedBorrow || required.implies(predicates))
}
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
        (None, terminal_psi::record_or_fixed_array_shape_pattern!()) => {
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
    if matches!(
        source_shape,
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
    ) && types
        .clone()
        .find(|row| row.id == *referent)
        .is_some_and(|row| row.shape == *source_shape)
    {
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
