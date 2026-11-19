use crate::descriptors::descriptor_results::descriptor_value_access;
use crate::exactly_one::exactly_one;
use crate::runtime_structural_path_tip;
use semantic_vocabulary::IntegerSign;
use semantic_vocabulary::{ScalarType, StructuralTypeId};

pub fn shared_descriptor_copy_matches<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    occurrence_access: Option<StructuralAccess>,
    destination: StructuralTypeId,
    destination_multiplicity: terminal_psi::StructuralMultiplicity,
) -> bool {
    let Some(source) = crate::structural_declaration(types.clone(), source) else {
        return false;
    };
    let Some(destination) = crate::structural_declaration(types.clone(), destination) else {
        return false;
    };
    descriptor_geometry_matches(&source.shape, &destination.shape)
        && descriptor_declared_access(&source.shape)
            .flatten()
            .or(occurrence_access)
            == Some(StructuralAccess::SharedBorrow)
        && descriptor_value_access(types, destination.id, destination_multiplicity)
            == Some(StructuralAccess::SharedBorrow)
}

pub fn descriptor_value_transfer_matches<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    destination: StructuralTypeId,
    multiplicity: terminal_psi::StructuralMultiplicity,
) -> bool {
    let Some(source_access) = descriptor_value_access(types.clone(), source, multiplicity)
    else {
        return false;
    };
    descriptor_value_access(types.clone(), destination, multiplicity) == Some(source_access)
        && crate::borrowed_referent_presentation(types, source, destination)
            == Some(crate::BorrowedReferentPresentation::Unchanged)
}

pub fn descriptor_reborrow_matches<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    source_access: StructuralAccess,
    source_occurrence_access: Option<StructuralAccess>,
    destination: StructuralTypeId,
    requested_access: StructuralAccess,
) -> bool {
    let Some(source) = crate::structural_declaration(types.clone(), source) else {
        return false;
    };
    let Some(destination) = crate::structural_declaration(types.clone(), destination) else {
        return false;
    };
    descriptor_reborrow_shapes_match(
        &source.shape,
        source_access,
        source_occurrence_access,
        &destination.shape,
        requested_access,
    )
}

pub fn descriptor_reborrow_shapes_match(
    source: &StructuralTypeShape,
    source_access: StructuralAccess,
    source_occurrence_access: Option<StructuralAccess>,
    destination: &StructuralTypeShape,
    requested_access: StructuralAccess,
) -> bool {
    let source_ceiling = descriptor_declared_access(source).flatten();
    let destination_ceiling = descriptor_declared_access(destination).flatten();
    descriptor_geometry_matches(source, destination)
        && requested_access.admits(source_access)
        && source_occurrence_access
            .or(source_ceiling)
            .is_some_and(|access| requested_access.admits_referent(access))
        && source_ceiling.is_none_or(|access| requested_access.admits_referent(access))
        && destination_ceiling.is_none_or(|access| requested_access.admits_referent(access))
}

pub fn stored_reference_access(shape: &StructuralTypeShape) -> Option<StructuralAccess> {
    match shape {
        StructuralTypeShape::Reference { access, .. } => Some(*access),
        _ => descriptor_declared_access(shape).flatten(),
    }
}

pub(crate) fn descriptor_declared_access(
    shape: &StructuralTypeShape,
) -> Option<Option<StructuralAccess>> {
    match shape {
        StructuralTypeShape::ElementView { access, .. }
        | StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView { access }
            | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
        ) => Some(*access),
        _ => None,
    }
}

pub fn descriptor_geometry_matches(
    source: &StructuralTypeShape,
    destination: &StructuralTypeShape,
) -> bool {
    use terminal_psi::ByteSequenceCarrier;
    match (source, destination) {
        (
            StructuralTypeShape::ElementView {
                element: source, ..
            },
            StructuralTypeShape::ElementView {
                element: destination,
                ..
            },
        ) => source == destination,
        (
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }),
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView { .. }),
        ) => true,
        (
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: source,
                ..
            }),
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedView {
                capacity: destination,
                ..
            }),
        ) => source == destination,
        _ => false,
    }
}

pub fn is_copyable_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    structural_type: StructuralTypeId,
) -> bool {
    let mut declarations = types.clone().filter(|row| row.id == structural_type);
    let declared_copyable = declarations
        .next()
        .is_some_and(|row| row.is_copyable() && !row.owns_linear_claim)
        && declarations.next().is_none();
    declared_copyable || is_shared_reference_carrier(types, structural_type)
}

pub fn is_shared_reference_carrier<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    mut structural_type: StructuralTypeId,
) -> bool {
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
            }
            | StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(access),
                }
                | terminal_psi::ByteSequenceCarrier::BoundedView {
                    access: Some(access),
                    ..
                },
            ) => {
                return access == StructuralAccess::SharedBorrow;
            }
            StructuralTypeShape::FixedArray { element, .. } => structural_type = element,
            _ => return false,
        }
    }
    false
}

pub fn representation_view_declaration<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    view: StructuralTypeId,
    stored: Option<ScalarType>,
) -> Option<&'types StructuralTypeDeclaration> {
    let declaration = exactly_one(types, |declaration| declaration.id == view)?;
    match (stored, &declaration.shape) {
        (Some(stored), StructuralTypeShape::PrimitiveScalar(logical))
            if reference_storage_view_is_compatible(stored, *logical) => {}
        (None, StructuralTypeShape::Record { .. } | StructuralTypeShape::FixedArray { .. }) => {
        }
        _ => return None,
    }
    Some(declaration)
}

pub fn reference_source_storage_path<'types, 'path>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &'path [terminal_psi::StructuralPathSegment],
) -> &'path [terminal_psi::StructuralPathSegment] {
    if let Some((terminal_psi::StructuralPathSegment::Referent, prefix)) = path.split_last()
        && (runtime_structural_path_tip(types.clone(), root, prefix).is_some_and(|tip| {
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

pub fn reference_source_projection_compatible<'types>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    reference: StructuralTypeId,
) -> bool {
    let path = reference_source_storage_path(types.clone(), root, path);
    if let Some(StructuralTypeShape::Reference {
        access: StructuralAccess::SharedBorrow,
        referent,
    }) = crate::structural_shape(types.clone(), reference)
        && let Some(target) = crate::structural_declaration(types.clone(), *referent)
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
    if let Some(field) = crate::runtime_structural_field(types.clone(), root, path)
        && let Some(carrier) = field.field_type.byte_sequence_carrier()
        && let Some(StructuralTypeShape::Reference { referent, .. }) =
            crate::structural_shape(types.clone(), reference)
        && crate::structural_declaration(types.clone(), *referent)
            .is_some_and(|row| row.shape == StructuralTypeShape::ByteSequence(carrier))
    {
        return true;
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
    let Some(carrier) = field.field_type.byte_sequence_carrier() else {
        return false;
    };
    let Some(terminal_psi::StructuralTypeShape::Reference { referent, .. }) =
        crate::structural_shape(types.clone(), reference)
    else {
        return false;
    };
    crate::structural_declaration(types.clone(), *referent).is_some_and(|row| {
        row.shape == terminal_psi::StructuralTypeShape::ByteSequence(carrier)
    })
}

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
        && crate::descriptors::descriptor_results::is_shared_descriptor(types.clone(), root)
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
                    runtime_structural_path_tip(types.clone(), root, prefix)
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
            StructuralPathSegment::FixedIndex(_)
            | StructuralPathSegment::RuntimeIndex { .. } => {
                let declaration = runtime_structural_path_tip(types.clone(), root, prefix);
                if let Some(StructuralTypeShape::ElementView { access, .. }) =
                    declaration.map(|row| &row.shape)
                {
                    match access {
                        Some(access) if !requested.admits_referent(*access) => return false,
                        None if position != region_start
                            && requested != StructuralAccess::SharedBorrow =>
                        {
                            return false;
                        }
                        _ => {}
                    }
                }
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
        runtime_structural_path_tip(types.clone(), root, path).map(|row| &row.shape)
    {
        match access {
            Some(access) if !requested.admits_referent(*access) => return false,
            None if path.len() != region_start
                && requested != StructuralAccess::SharedBorrow =>
            {
                return false;
            }
            _ => {}
        }
    }
    let carrier = crate::runtime_structural_field(types.clone(), root, path)
        .and_then(|field| field.field_type.byte_sequence_carrier())
        .or_else(|| {
            runtime_structural_path_tip(types.clone(), root, path).and_then(|row| {
                match row.shape {
                    StructuralTypeShape::ByteSequence(carrier) => Some(carrier),
                    _ => None,
                }
            })
        });
    if let Some(
        ByteSequenceCarrier::BorrowedView { access }
        | ByteSequenceCarrier::BoundedView { access, .. },
    ) = carrier
    {
        match access {
            Some(access) if !requested.admits_referent(access) => return false,
            None if path.len() != region_start
                && requested != StructuralAccess::SharedBorrow =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

pub fn reference_byte_field_compatible<'types, 'domains>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    domains: impl Iterator<Item = &'domains terminal_psi::StructuralDomainDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    reference: StructuralTypeId,
    memberships: &[terminal_psi::StructuralPathQualification],
) -> bool {
    let Some(terminal_psi::StructuralTypeShape::Reference { referent, access }) =
        crate::structural_shape(types.clone(), reference)
    else {
        return false;
    };
    let Some(terminal_psi::StructuralTypeShape::ByteSequence(
        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
    )) = crate::structural_shape(types.clone(), *referent)
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
        && (*access == terminal_psi::StructuralAccess::SharedBorrow
            || required.implies(predicates))
}
pub fn reference_storage_byte_width(stored: ScalarType) -> Option<u8> {
    match stored {
        ScalarType::Integer(integer) if !integer.is_address() => {
            let bits = integer.bits()?;
            (bits % 8 == 0)
                .then(|| u8::try_from(bits / 8).ok())
                .flatten()
        }
        ScalarType::Callable(_) => Some(8),
        ScalarType::Boolean => Some(1),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32) => Some(4),
        ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary64) => Some(8),
        _ => None,
    }
}

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
        (IntegerSign::Signed, IntegerSign::Unsigned) => false,
        (IntegerSign::Unsigned, IntegerSign::Signed) => logical.bits() > stored.bits(),
        _ => true,
    }
}

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
    for bits in (8..=128).step_by(8) {
        for sign in [IntegerSign::Signed, IntegerSign::Unsigned] {
            let integer = semantic_vocabulary::IntegerType::new(sign, bits).ok()?;
            if !same_width(ScalarType::Integer(integer)) {
                return None;
            }
        }
    }
    Some(width)
}

pub fn reference_storage_region_source_compatible<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    reference: StructuralTypeId,
    stored: Option<ScalarType>,
) -> bool {
    crate::ReferenceStorageGeometry::new(types, |application| {
        Err(crate::StorageGeometryError::UnresolvedExtent(application))
    })
    .view_source_compatible(source, path, reference, stored)
}

pub fn reference_storage_source_compatible<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    reference: StructuralTypeId,
    stored: Option<ScalarType>,
) -> bool {
    crate::ReferenceStorageGeometry::new(types, |application| {
        Err(crate::StorageGeometryError::UnresolvedExtent(application))
    })
    .source_compatible(source, reference, stored)
}

pub(crate) fn plain_storage<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
) -> bool {
    storage_scalars_satisfy(types, root, active, &|scalar| {
        reference_storage_byte_width(scalar).is_some()
    })
}

pub(super) fn is_byte_scalar(scalar: ScalarType) -> bool {
    matches!(scalar, ScalarType::Integer(integer) if !integer.is_address()
        && integer.bits() == Some(8) && integer.sign() == IntegerSign::Unsigned)
}

pub(super) fn storage_scalars_satisfy<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    active: &mut Vec<StructuralTypeId>,
    accepts: &impl Fn(ScalarType) -> bool,
) -> bool {
    if active.len() >= 128 || active.contains(&root) {
        return false;
    }
    let Some(shape) = crate::structural_shape(types.clone(), root) else {
        return false;
    };
    active.push(root);
    let valid = match shape {
        StructuralTypeShape::PrimitiveScalar(scalar) => accepts(*scalar),
        StructuralTypeShape::FixedArray { element, .. } => {
            storage_scalars_satisfy(types.clone(), *element, active, accepts)
        }
        StructuralTypeShape::Record { fields, .. } => fields
            .iter()
            .filter(|field| !field.relevance.is_erased())
            .all(|field| match field.field_type {
                terminal_psi::StructuralFieldType::Structural(child) => {
                    storage_scalars_satisfy(types.clone(), child, active, accepts)
                }
                terminal_psi::StructuralFieldType::Atomic(_)
                | terminal_psi::StructuralFieldType::AtomicBoundedInteger(_) => false,
                _ => field.field_type.scalar_type().is_some_and(accepts),
            }),
        _ => false,
    };
    active.pop();
    valid
}
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration, StructuralTypeShape};

use crate::scalars::scalar_representation_view::{integer_bits, integer_from_bits};
use crate::{ScalarRepresentationViewPair, ScalarViewValue};

pub fn scalar_reference_view_is_total(source: ScalarType, target: ScalarType) -> bool {
    if source == target {
        return true;
    }
    if let (ScalarType::Integer(source), ScalarType::Integer(target)) = (source, target) {
        return !source.is_address()
            && !target.is_address()
            && source.bits() == target.bits()
            && source.bits().is_some_and(|bits| bits <= 64);
    }
    ScalarRepresentationViewPair::classify(source, target)
        .is_some_and(|pair| !pair.requires_landing())
}

/// Whether a reference with `access` may present `stored` scalar storage as
/// `presented` (wiki/spec/layouts/recasts.md#shared-and-mutable-views).
///
/// A recast keeps the same backing, so the judgment follows the operations
/// the view enables: a shared reader needs every stored value to read as a
/// valid presented value, a write-only view needs every presented value to
/// leave valid stored bits, and a mutable view needs both. Owned values are
/// never reinterpreted in place. Domain handback obligations on the lender
/// stay with the checker; this judges only representation validity.
pub fn scalar_reference_view_admits(
    access: StructuralAccess,
    stored: ScalarType,
    presented: ScalarType,
) -> bool {
    let reads = || scalar_reference_view_is_total(stored, presented);
    let writes = || scalar_reference_view_is_total(presented, stored);
    match access {
        StructuralAccess::Owned => stored == presented,
        StructuralAccess::SharedBorrow => reads(),
        StructuralAccess::WriteOnlyBorrow => writes(),
        StructuralAccess::MutableBorrow => reads() && writes(),
    }
}

/// The leaf contract an inline field presents to a borrow with `access`
/// that expects `expected`. Every Terminal consumer (native target lowering
/// and the Psi interpreter) resolves borrowed field leaves here, so a
/// cross-carrier scalar view of a field has one admission rule.
pub fn borrowed_field_presentation(
    field: &terminal_psi::StructuralFieldType,
    access: StructuralAccess,
    expected: Option<&StructuralTypeDeclaration>,
) -> Option<terminal_psi::StructuralFieldType> {
    let presentation = field.borrowed_presentation_type(access, expected)?;
    if let Some(expected) = expected
        && presentation != terminal_psi::StructuralFieldType::Structural(expected.id)
        && expected.scalar_restriction.is_none()
        && let StructuralTypeShape::PrimitiveScalar(presented) = expected.shape
        && let Some(StructuralTypeShape::PrimitiveScalar(stored)) =
            presentation.canonical_leaf_shape()
        && scalar_reference_view_admits(access, stored, presented)
    {
        return Some(terminal_psi::StructuralFieldType::Structural(expected.id));
    }
    Some(presentation)
}

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
        )?));
    }
    ScalarRepresentationViewPair::classify(source, target)?.view(value)
}

pub fn shared_byte_descriptor_storage_matches(
    source: terminal_psi::ByteSequenceCarrier,
    referent: terminal_psi::ByteSequenceCarrier,
) -> bool {
    use terminal_psi::ByteSequenceCarrier::BorrowedView;
    source == (BorrowedView { access: None })
        && referent
            == (BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            })
}

fn shared_byte_descriptor_reference_shapes(
    source: &StructuralTypeShape,
    referent: &StructuralTypeShape,
    access: StructuralAccess,
) -> bool {
    access == StructuralAccess::SharedBorrow
        && matches!((source, referent),
            (StructuralTypeShape::ByteSequence(source), StructuralTypeShape::ByteSequence(referent))
                if shared_byte_descriptor_storage_matches(*source, *referent))
}

/// Match a shared borrow of an incoming mutable view's descriptor slot.
/// The original formal supplies the backing permission.
pub fn shared_direct_byte_descriptor_source(
    module: &terminal_psi::TerminalModule,
    machine: &terminal_psi::TerminalMachine,
    source: &terminal_psi::StructuralArgument,
    reference: StructuralTypeId,
) -> Option<bool> {
    let StructuralTypeShape::Reference { access, referent } =
        module.structural_type(reference)?.shape
    else {
        return None;
    };
    let target = module.structural_type(referent)?;
    let direct_byte_view =
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: None,
        });
    if !shared_byte_descriptor_reference_shapes(&direct_byte_view, &target.shape, access) {
        return None;
    }
    let root = machine.structural_place_type(source.place)?;
    let path =
        reference_source_storage_path(module.structural_types.iter(), root, &source.path);
    let actual =
        crate::runtime_structural_path_tip(module.structural_types.iter(), root, path)?;
    if actual.shape != direct_byte_view {
        return None;
    }
    Some(
        source.access == access
            && source.path.is_empty()
            && crate::arrival_parameter(machine, source.place).is_some_and(|parameter| {
                parameter.structural_type == root
                    && parameter.access == StructuralAccess::MutableBorrow
            }),
    )
}

pub fn reference_source_compatible<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    source: StructuralTypeId,
    reference: StructuralTypeId,
) -> bool {
    let Some(StructuralTypeShape::Reference { referent, access }) =
        crate::structural_shape(types.clone(), reference)
    else {
        return false;
    };
    let Some(source_shape) = crate::structural_shape(types.clone(), source) else {
        return false;
    };
    if source == *referent {
        return true;
    }
    if crate::structural_shape(types.clone(), *referent).is_some_and(|target| {
        shared_byte_descriptor_reference_shapes(source_shape, target, *access)
    }) {
        return true;
    }
    if matches!(
        source_shape,
        StructuralTypeShape::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
        )
    ) && crate::structural_declaration(types.clone(), *referent)
        .is_some_and(|row| row.shape == *source_shape)
    {
        return true;
    }
    let Some(StructuralTypeShape::PrimitiveScalar(target)) =
        crate::structural_shape(types, *referent)
    else {
        return false;
    };
    matches!(source_shape, StructuralTypeShape::PrimitiveScalar(source)
        if scalar_reference_view_admits(*access, *source, *target))
}
#[cfg(test)]
mod scalar_view_admission_tests;
