//! The shape of each structural type declaration: references, records,
//! fixed arrays, sums and mixed payloads over known types.

use super::{has_structural_type, require_unique_nonempty_identities};
use crate::codec_error::{CodecError, malformed};
use terminal_psi::StructuralTypeDeclaration;
use terminal_psi::{StructuralFieldType, StructuralTypeShape, TerminalModule};

/// One structural type declaration's shape is well founded: its referent,
/// fields, element, cases and mixed payloads name known types with the
/// access, multiplicity and layout each shape allows.
pub(super) fn validate_type_shape(
    module: &TerminalModule,
    declaration: &StructuralTypeDeclaration,
) -> Result<(), CodecError> {
    match &declaration.shape {
        StructuralTypeShape::Reference { referent, access } => {
            if !has_structural_type(module, *referent)
                || *access == terminal_psi::StructuralAccess::Owned
            {
                return malformed("reference type requires a known referent and borrowed access");
            }
        }
        StructuralTypeShape::PrimitiveScalar(_) => {}
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView) => {}
        StructuralTypeShape::ByteSequence(_) => {
            return malformed("first-class byte-sequence type must be a borrowed view");
        }
        StructuralTypeShape::Record { fields } => {
            require_unique_nonempty_identities(
                fields.iter().map(|field| field.identity.as_str()),
                "structural field identity",
            )?;
            for field in fields {
                match &field.field_type {
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
                            && !module
                                .machines
                                .iter()
                                .any(|machine| machine.attachment == Some(declaration.id)) =>
                    {
                        return malformed(
                            "provider-backed attachment specialization is incomplete",
                        );
                    }
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::BoundedInteger(_)
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
                        | StructuralFieldType::BoundedInteger(_)
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
                    | StructuralFieldType::BoundedInteger(_)
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
