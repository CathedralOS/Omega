//! The shape of each structural type declaration: references, records,
//! fixed arrays, sums and mixed payloads over known types.

use super::{has_structural_type, require_unique_nonempty_identities};
use crate::codec_error::{CodecError, malformed};
use semantic_vocabulary::StructuralTypeId;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalModule};

/// One structural type declaration's shape is well founded: its referent,
/// fields, element, cases and mixed payloads name known types with the
/// access, multiplicity and layout each shape allows.
pub(super) fn validate_type_shape(
    module: &TerminalModule,
    declaration: &StructuralTypeDeclaration,
    attachment_storage: &BTreeSet<StructuralTypeId>,
) -> Result<(), CodecError> {
    validate_field_storage_envelope(&declaration.shape)?;
    match &declaration.shape {
        StructuralTypeShape::Reference { referent, access } => {
            if !has_structural_type(module, *referent)
                || *access == terminal_psi::StructuralAccess::Owned
            {
                return malformed("reference type requires a known referent and borrowed access");
            }
        }
        StructuralTypeShape::PrimitiveScalar(_) | StructuralTypeShape::FusedServiceBinding => {}
        // Completed owned concatenations retain their bounded carrier here.
        // Formation and readers independently validate result custody; the
        // carrier declaration alone creates no source or borrowing authority.
        StructuralTypeShape::ByteSequence(_) => {}
        StructuralTypeShape::Record { fields, storage } => {
            if let Some(storage) = storage {
                if storage.byte_alignment == 0 || !storage.byte_alignment.is_power_of_two() {
                    return malformed(
                        "authored record storage requires positive power-of-two alignment",
                    );
                }
            }
            require_unique_nonempty_identities(
                fields.iter().map(|field| field.identity.as_str()),
                "structural field identity",
            )?;
            for field in fields {
                match (storage, field.storage) {
                    (Some(_), None) if !field.relevance.is_erased() => {
                        return malformed("authored record field has no storage interpretation");
                    }
                    (Some(record), Some(field_storage)) => {
                        if field.relevance.is_erased()
                            || field_storage
                                .byte_offset()
                                .is_some_and(|offset| offset > record.byte_size)
                            || field_storage.element_stride() == Some(0)
                        {
                            return malformed(
                                "authored field storage has invalid offset, stride or relevance",
                            );
                        }
                        validate_authored_field_extent(
                            module,
                            field,
                            field_storage,
                            record.byte_size,
                        )?;
                        if let Some(stored) = field_storage.stored_scalar() {
                            let logical = field
                                .field_type
                                .scalar_type()
                                .or_else(|| {
                                    let StructuralFieldType::Structural(identity) =
                                        field.field_type
                                    else {
                                        return None;
                                    };
                                    let StructuralTypeShape::PrimitiveScalar(scalar) = module
                                        .structural_types
                                        .iter()
                                        .find(|declaration| declaration.id == identity)?
                                        .shape
                                    else {
                                        return None;
                                    };
                                    Some(scalar)
                                })
                                .ok_or(CodecError::MalformedStructuralFoundation(
                                    "encoded field requires a scalar referent",
                                ))?;
                            let width = terminal_semantics::reference_storage_byte_width(stored)
                                .ok_or(CodecError::MalformedStructuralFoundation(
                                    "encoded field has no byte-aligned scalar footprint",
                                ))?;
                            if !terminal_semantics::reference_storage_view_is_compatible(
                                stored, logical,
                            ) || field_storage
                                .byte_offset()
                                .and_then(|offset| offset.checked_add(u64::from(width)))
                                .is_none_or(|end| end > record.byte_size)
                            {
                                return malformed(
                                    "encoded field footprint or interpretation is incompatible",
                                );
                            }
                        }
                    }
                    _ => {}
                }
                match &field.field_type {
                    StructuralFieldType::Atomic(_)
                        if field.field_type.atomic_scalar_type().is_none() =>
                    {
                        return malformed("atomic field requires a fixed integer or Boolean cell");
                    }
                    StructuralFieldType::Structural(field_type)
                        if !has_structural_type(module, *field_type) =>
                    {
                        return malformed("structural field references an unknown structural type");
                    }
                    StructuralFieldType::Erased { type_identity } if type_identity.is_empty() => {
                        return malformed(
                            "opaque structural field type must have a nonempty type identity",
                        );
                    }
                    StructuralFieldType::Erased { .. }
                        if !field.relevance.is_erased()
                            && !attachment_storage.contains(&declaration.id) =>
                    {
                        return malformed(
                            "provider-backed attachment specialization is incomplete",
                        );
                    }
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::Atomic(_)
                    | StructuralFieldType::BoundedInteger(_)
                    | StructuralFieldType::RefinedScalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::Structural(_)
                        if field.relevance.is_erased() =>
                    {
                        return malformed(
                            "erased structural field must use its opaque semantic type identity",
                        );
                    }
                    _ => {}
                }
            }
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            if !has_structural_type(module, *element) {
                return malformed("fixed array references an unknown structural element type");
            }
        }
        StructuralTypeShape::ElementView { element } => {
            if !has_structural_type(module, *element) {
                return malformed("element view references an unknown structural element type");
            }
        }
        StructuralTypeShape::Sum { cases } => {
            require_unique_nonempty_identities(
                cases.iter().map(|case| case.identity.as_str()),
                "structural case identity",
            )?;
            if cases.is_empty() {
                return malformed("structural sum must declare at least one case");
            }
            for case in cases {
                require_unique_nonempty_identities(
                    case.fields.iter().map(|field| field.identity.as_str()),
                    "structural case payload field identity",
                )?;
                for field in &case.fields {
                    match &field.field_type {
                        StructuralFieldType::Atomic(_)
                            if field.field_type.atomic_scalar_type().is_none() =>
                        {
                            return malformed(
                                "atomic field requires a fixed integer or Boolean cell",
                            );
                        }
                        StructuralFieldType::Structural(field_type)
                            if !has_structural_type(module, *field_type) =>
                        {
                            return malformed(
                                "structural case payload references an unknown structural type",
                            );
                        }
                        StructuralFieldType::Erased { type_identity }
                            if !field.relevance.is_erased() || type_identity.is_empty() =>
                        {
                            return malformed(
                                "opaque structural case payload must have erased relevance and a nonempty type identity",
                            );
                        }
                        StructuralFieldType::Scalar(_)
                        | StructuralFieldType::Atomic(_)
                        | StructuralFieldType::BoundedInteger(_)
                        | StructuralFieldType::RefinedScalar(_)
                        | StructuralFieldType::IeeeFloat(_)
                        | StructuralFieldType::Structural(_)
                            if field.relevance.is_erased() =>
                        {
                            return malformed(
                                "erased structural case payload must use its opaque semantic type identity",
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        StructuralTypeShape::Mixed { fields, cases } => {
            require_unique_nonempty_identities(
                fields.iter().map(|field| field.identity.as_str()),
                "mixed structural field identity",
            )?;
            require_unique_nonempty_identities(
                cases.iter().map(|case| case.identity.as_str()),
                "mixed structural case identity",
            )?;
            if cases.is_empty() {
                return malformed("mixed structural type must declare at least one case");
            }
            for case in cases {
                require_unique_nonempty_identities(
                    case.fields.iter().map(|field| field.identity.as_str()),
                    "mixed structural case payload field identity",
                )?;
            }
            for field in fields
                .iter()
                .chain(cases.iter().flat_map(|case| &case.fields))
            {
                match &field.field_type {
                    StructuralFieldType::Atomic(_)
                        if field.field_type.atomic_scalar_type().is_none() =>
                    {
                        return malformed("atomic field requires a fixed integer or Boolean cell");
                    }
                    StructuralFieldType::Structural(field_type)
                        if !has_structural_type(module, *field_type) =>
                    {
                        return malformed(
                            "mixed structural field references an unknown structural type",
                        );
                    }
                    StructuralFieldType::Erased { type_identity }
                        if !field.relevance.is_erased() || type_identity.is_empty() =>
                    {
                        return malformed(
                            "opaque mixed structural field must have erased relevance and a nonempty type identity",
                        );
                    }
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::Atomic(_)
                    | StructuralFieldType::BoundedInteger(_)
                    | StructuralFieldType::RefinedScalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::Structural(_)
                        if field.relevance.is_erased() =>
                    {
                        return malformed(
                            "erased mixed structural field must use its opaque semantic type identity",
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

/// Check the entire contiguous extent, not only its starting coordinate.
/// Opaque and fragmented layouts remain unavailable to direct projection;
/// absence of portable geometry never means an invented native pointer width.
fn validate_authored_field_extent(
    module: &TerminalModule,
    field: &terminal_psi::StructuralFieldDeclaration,
    storage: semantic_vocabulary::AuthoredFieldStorage,
    record_size: u64,
) -> Result<(), CodecError> {
    use semantic_vocabulary::AuthoredFieldStorage;

    let scalar = field
        .field_type
        .scalar_type()
        .or_else(|| field.field_type.atomic_scalar_type());
    let declared_layout = || {
        if let Some(scalar) = scalar {
            let width = u64::from(terminal_semantics::reference_storage_byte_width(scalar)?);
            return Some((width, width));
        }
        let StructuralFieldType::Structural(child) = field.field_type else {
            return None;
        };
        terminal_semantics::reference_storage_layout(module.structural_types.iter(), child)
    };
    let (offset, extent) = match storage {
        AuthoredFieldStorage::Fragmented => return Ok(()),
        AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar: Some(stored),
        } => {
            let width = terminal_semantics::reference_storage_byte_width(stored).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "encoded field has no contiguous scalar footprint",
                ),
            )?;
            (byte_offset, Some(u64::from(width)))
        }
        AuthoredFieldStorage::Scalar {
            byte_offset,
            stored_scalar: None,
        }
        | AuthoredFieldStorage::Direct { byte_offset } => {
            (byte_offset, declared_layout().map(|(size, _)| size))
        }
        AuthoredFieldStorage::Repeated {
            byte_offset,
            element_stride,
        } => {
            let StructuralFieldType::Structural(array) = field.field_type else {
                return malformed("repeated field requires a fixed array");
            };
            let Some(StructuralTypeShape::FixedArray { element, length }) = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == array)
                .map(|declaration| &declaration.shape)
            else {
                return malformed("repeated field requires a fixed array");
            };
            if let Some((size, alignment)) = terminal_semantics::reference_storage_layout(
                module.structural_types.iter(),
                *element,
            ) {
                if alignment == 0
                    || element_stride < size
                    || element_stride % alignment != 0
                    || byte_offset % alignment != 0
                {
                    return malformed("repeated field stride overlaps or misaligns its elements");
                }
            }
            let extent = length.checked_mul(element_stride).ok_or(
                CodecError::MalformedStructuralFoundation("repeated field extent overflows"),
            )?;
            (byte_offset, Some(extent))
        }
    };
    if extent.is_some_and(|extent| {
        offset
            .checked_add(extent)
            .is_none_or(|end| end > record_size)
    }) {
        return malformed("authored field extent exceeds its record storage");
    }
    Ok(())
}

/// Only the authored record envelope carries field geometry on the wire.
/// Reject metadata in other rosters before an encoder can discard it.
fn validate_field_storage_envelope(shape: &StructuralTypeShape) -> Result<(), CodecError> {
    if matches!(
        shape,
        StructuralTypeShape::Record {
            storage: Some(_),
            ..
        }
    ) {
        return Ok(());
    }
    let direct_storage = shape
        .direct_fields()
        .is_some_and(|fields| fields.iter().any(|field| field.storage.is_some()));
    let case_storage = match shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases
            .iter()
            .flat_map(|case| &case.fields)
            .any(|field| field.storage.is_some()),
        _ => false,
    };
    if direct_storage || case_storage {
        return malformed("field storage requires an authored record envelope");
    }
    Ok(())
}

/// Receiver storage retains nested provider declarations even when no method
/// on the nested record is demanded. This closure admits declarations only:
/// provider roots and calls must still join the calling machine's exact,
/// direct attachment. References and views do not contain receiver storage.
pub(super) fn attachment_storage_types(
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
    attachments: impl Iterator<Item = StructuralTypeId>,
) -> BTreeSet<StructuralTypeId> {
    use terminal_psi::{StructuralFieldType, StructuralTypeShape};
    let mut pending: Vec<_> = attachments.collect();
    let mut storage = BTreeSet::new();
    while let Some(identity) = pending.pop() {
        if !storage.insert(identity) {
            continue;
        }
        let Some(declaration) = types.get(&identity) else {
            // Ordinary graph and attachment validation reject unknown types.
            continue;
        };
        let mut retain_fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
            pending.extend(fields.iter().filter_map(|field| match field.field_type {
                StructuralFieldType::Structural(child) => Some(child),
                _ => None,
            }));
        };
        match &declaration.shape {
            StructuralTypeShape::Record { fields, .. } => retain_fields(fields),
            StructuralTypeShape::Mixed { fields, cases } => {
                retain_fields(fields);
                for case in cases {
                    retain_fields(&case.fields);
                }
            }
            StructuralTypeShape::Sum { cases } => {
                for case in cases {
                    retain_fields(&case.fields);
                }
            }
            StructuralTypeShape::FixedArray { element, .. } => pending.push(*element),
            StructuralTypeShape::PrimitiveScalar(_)
            | StructuralTypeShape::FusedServiceBinding
            | StructuralTypeShape::ByteSequence(_)
            | StructuralTypeShape::Reference { .. }
            | StructuralTypeShape::ElementView { .. } => {}
        }
    }
    storage
}

#[cfg(test)]
mod authored_storage_envelope_tests {
    use super::{validate_authored_field_extent, validate_field_storage_envelope};
    use semantic_vocabulary::{
        AuthoredFieldStorage, AuthoredRecordStorage, ScalarType, StructuralCaseId,
        StructuralFieldId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeShape,
    };

    #[test]
    fn field_geometry_requires_the_record_envelope_in_every_field_roster() {
        let field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).expect("field identity"),
            identity: "value".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
            storage: Some(AuthoredFieldStorage::Fragmented),
        };
        let case = StructuralCaseDeclaration {
            id: StructuralCaseId::new(1).expect("case identity"),
            identity: "present".into(),
            fields: vec![field.clone()],
        };
        for shape in [
            StructuralTypeShape::Record {
                fields: vec![field.clone()],
                storage: None,
            },
            StructuralTypeShape::Sum {
                cases: vec![case.clone()],
            },
            StructuralTypeShape::Mixed {
                fields: vec![field.clone()],
                cases: Vec::new(),
            },
            StructuralTypeShape::Mixed {
                fields: Vec::new(),
                cases: vec![case],
            },
        ] {
            assert!(validate_field_storage_envelope(&shape).is_err());
        }
        let authored = StructuralTypeShape::Record {
            fields: vec![field],
            storage: Some(AuthoredRecordStorage {
                byte_size: 1,
                byte_alignment: 1,
            }),
        };
        assert!(validate_field_storage_envelope(&authored).is_ok());
    }

    #[test]
    fn contiguous_fields_must_fit_the_whole_footprint() {
        let mut module = terminal_psi::TerminalModule::for_entry(
            semantic_vocabulary::MachineId::new(1).expect("entry"),
        );
        let scalar_type = semantic_vocabulary::StructuralTypeId::new(1).expect("scalar type");
        let integer = ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64)
                .expect("signed integer"),
        );
        module
            .structural_types
            .push(terminal_psi::StructuralTypeDeclaration {
                id: scalar_type,
                identity: "integer".into(),
                shape: StructuralTypeShape::PrimitiveScalar(integer),
            });
        let field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).expect("field"),
            identity: "value".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(scalar_type),
            storage: None,
        };
        for storage in [
            AuthoredFieldStorage::Scalar {
                byte_offset: 0,
                stored_scalar: None,
            },
            AuthoredFieldStorage::Direct { byte_offset: 0 },
        ] {
            assert!(validate_authored_field_extent(&module, &field, storage, 1).is_err());
            assert!(validate_authored_field_extent(&module, &field, storage, 8).is_ok());
        }
        let offset = AuthoredFieldStorage::Direct {
            byte_offset: u64::MAX - 3,
        };
        assert!(validate_authored_field_extent(&module, &field, offset, u64::MAX).is_err());
    }

    #[test]
    fn repeated_fields_require_aligned_nonoverlapping_bounded_extents() {
        let mut module = terminal_psi::TerminalModule::for_entry(
            semantic_vocabulary::MachineId::new(1).expect("entry"),
        );
        let element = semantic_vocabulary::StructuralTypeId::new(1).expect("element");
        let array = semantic_vocabulary::StructuralTypeId::new(2).expect("array");
        module
            .structural_types
            .push(terminal_psi::StructuralTypeDeclaration {
                id: element,
                identity: "element".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                    semantic_vocabulary::IntegerType::new(
                        semantic_vocabulary::IntegerSign::Signed,
                        64,
                    )
                    .expect("signed integer"),
                )),
            });
        module
            .structural_types
            .push(terminal_psi::StructuralTypeDeclaration {
                id: array,
                identity: "array".into(),
                shape: StructuralTypeShape::FixedArray { element, length: 2 },
            });
        let field = StructuralFieldDeclaration {
            id: StructuralFieldId::new(1).expect("field"),
            identity: "elements".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Structural(array),
            storage: None,
        };
        let storage = AuthoredFieldStorage::Repeated {
            byte_offset: 0,
            element_stride: 8,
        };
        assert!(validate_authored_field_extent(&module, &field, storage, 16).is_ok());
        assert!(validate_authored_field_extent(&module, &field, storage, 15).is_err());
        for storage in [
            AuthoredFieldStorage::Repeated {
                byte_offset: 0,
                element_stride: 1,
            },
            AuthoredFieldStorage::Repeated {
                byte_offset: 4,
                element_stride: 8,
            },
        ] {
            assert!(validate_authored_field_extent(&module, &field, storage, 24).is_err());
        }
        module.structural_types[1].shape = StructuralTypeShape::FixedArray {
            element,
            length: u64::MAX,
        };
        assert!(validate_authored_field_extent(&module, &field, storage, u64::MAX).is_err());
    }
}
