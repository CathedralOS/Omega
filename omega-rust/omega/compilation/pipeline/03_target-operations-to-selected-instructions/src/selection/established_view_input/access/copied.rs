//! A copied descriptor retains the permission of its selected source slot.
//! Layout validates the complete ordinary/case path; the declared slot, not
//! the unrestricted result annotation, supplies shared backing authority.
use super::*;
use crate::structural_inputs::structural_reference_input as geometry;

pub(super) fn shared_projection(
    function: &LegalizedScalarFunction,
    row: &legalized_operations::LegalizedScalarInstruction,
) -> Option<()> {
    let I::StructuralLeafCopy {
        result,
        source,
        path,
        byte_offset,
        shape,
        indices,
    } = &row.kind
    else {
        return None;
    };
    let signature = function.structural.as_ref()?;
    let types = &signature.structural_types;
    let root = if let Some(parameter) =
        crate::selection::primitive_local_input::read_parameter(function, *source)
    {
        if parameter.access == A::WriteOnlyBorrow
            || parameter.multiplicity == M::Linear
            || !parameter.qualifications.is_empty()
            || !parameter.projected_qualifications.is_empty()
        {
            return None;
        }
        parameter.structural_type
    } else {
        crate::selection::aggregate_result_input::home(function, *source)?
            .1
            .structural_type
    };
    let (selected, offset, elements, shared_field) = match path {
        abstract_operations::StructuralCopyPath::Ordinary(path) => {
            let geometry = geometry::leaf_copy_geometry(root, path, types)?;
            if !geometry.readable {
                return None;
            }
            let shared_field =
                terminal_semantics::shared_byte_field_copy_predicates(types.iter(), root, path)
                    .is_some();
            (
                geometry.selected,
                geometry.byte_offset,
                geometry.elements,
                shared_field,
            )
        }
        abstract_operations::StructuralCopyPath::Case(path) => {
            let (selected, offset) = geometry::case_projection(root, path, types)?;
            let shared_field = match path.split_last() {
                Some((
                    semantic_vocabulary::CanonicalStructuralPathSegment::Field(field),
                    parent,
                )) => terminal_semantics::observed_field_declaration(
                    types.iter(),
                    root,
                    parent,
                    *field,
                )
                .is_some_and(|field| {
                    matches!(
                        field.field_type.byte_sequence_carrier(),
                        Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: None | Some(A::SharedBorrow)
                        })
                    )
                }),
                _ => false,
            };
            (selected, offset, Vec::new(), shared_field)
        }
    };
    if selected != result.structural_type
        || offset != *byte_offset
        || geometry::shape(selected, types)? != *shape
        || !geometry::runtime_indices_match(indices, &elements)
    {
        return None;
    }
    // An access-less structural descriptor identity is insufficient: unlike
    // the direct byte-field declaration, it can also carry a mutable view. A
    // stored descriptor whose declared ceiling is shared, byte or element,
    // cannot.
    (shared_field
        || types.iter().any(|declaration| {
            declaration.id == selected
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(A::SharedBorrow)
                        }
                    ) | StructuralTypeShape::ElementView {
                        access: Some(A::SharedBorrow),
                        ..
                    }
                )
        }))
    .then_some(())
}
