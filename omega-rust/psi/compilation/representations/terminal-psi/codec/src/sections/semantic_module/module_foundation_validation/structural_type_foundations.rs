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
        StructuralTypeShape::Record { fields } => {
            require_unique_nonempty_identities(
                fields.iter().map(|field| field.identity.as_str()),
                "structural field identity",
            )?;
            for field in fields {
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
            StructuralTypeShape::Record { fields } => retain_fields(fields),
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
