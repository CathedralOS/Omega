//! Retained-shape matchers: each checks one reported placement, storage
//! relation or identity against the declarations the row claims — the row's
//! self-reported form is never authority.

use std::collections::BTreeSet;

use abstract_operations::{AbstractFunction, AbstractResult};
use calling_conventions::{ValueLocation, ValueShape};
use semantic_vocabulary::{OperationId, StructuralTypeId};
use target_operations::{TargetStructuralArgument, TargetUnitScalarHomeRequirement};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralOperationResult,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralResultDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

use super::canonical_roots::RootDeclaration;
use super::structural_shapes;
use super::structural_signatures;

/// The retained caller roster is the callee's declared roster plus exactly the
/// domains this call's own establishment rows mint; Terminal verification
/// replays each row's route. Minted membership has no storage, so the result
/// home stays the declared result's home. Any other caller domain refuses.
pub(super) fn declared_or_minted_qualifications(
    declared: &StructuralResultDeclaration,
    expected: &StructuralOperationResult,
) -> bool {
    let minted = expected
        .qualification_establishments
        .iter()
        .map(|establishment| establishment.domain);
    let roster = declared
        .qualifications
        .iter()
        .copied()
        .chain(minted)
        .collect::<BTreeSet<_>>();
    let retained = expected
        .qualifications
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    retained.len() == expected.qualifications.len() && retained == roster
}

/// A scalar result retains its home at this operation with the declared value,
/// the callee's declared type and that type's fixed-native shape. The call's
/// separately retained claim transfers preserve consumed structural arguments.
pub(super) fn scalar_result(
    psi_operation: OperationId,
    expected: &AbstractResult,
    actual: &TargetUnitScalarHomeRequirement,
    callee: &AbstractFunction,
) -> Result<(), OperationId> {
    let declared = callee.result.scalar().ok_or(psi_operation)?;
    if actual.defining_operation != psi_operation
        || actual.source_value != expected.value
        || actual.scalar_type != expected.scalar_type
        || structural_signatures::fixed_native_scalar_shape(expected.scalar_type)
            != Some(actual.shape)
        || actual.scalar_type != declared.scalar_type
    {
        return Err(psi_operation);
    }
    Ok(())
}

/// A fixed byte array lends its original backing through a view descriptor.
/// The projected storage type therefore differs from the callee's view type;
/// reconstruct the adaptation instead of requiring identity or trusting the
/// producer's length and stride. Ordinary pointer arguments have no adaptation.
pub(super) fn matches_projected_carrier(
    actual: &TargetStructuralArgument,
    projected: semantic_vocabulary::StructuralTypeId,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    use terminal_psi::{ByteSequenceCarrier, StructuralTypeShape};
    if actual.structural_type == projected {
        return actual.fixed_array_length.is_none() && actual.element_stride.is_none();
    }
    let find_shape = |identity| {
        declarations
            .iter()
            .find(|declaration| declaration.id == identity)
            .map(|declaration| &declaration.shape)
    };
    let Some(StructuralTypeShape::FixedArray { element, length }) = find_shape(projected) else {
        return false;
    };
    actual.access == StructuralAccess::MutableBorrow
        && *length > 0
        && matches!(
            find_shape(*element),
            Some(StructuralTypeShape::PrimitiveScalar(
                semantic_vocabulary::ScalarType::Integer(integer)
            )) if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                && integer.bits() == 8 && !integer.is_address()
        )
        && matches!(
            find_shape(actual.structural_type),
            Some(StructuralTypeShape::ByteSequence(
                ByteSequenceCarrier::BorrowedView { .. }
            ))
        )
        && actual.fixed_array_length == Some(*length)
        && actual.element_stride == Some(1)
}

/// Whole owners and inline fields share the same live length + bytes layout.
/// Reconstruct their storage independently of the call-local view descriptor;
/// source custody is checked separately against the exact arrival or producer.
pub(super) fn matches_bounded_byte_storage(
    actual: &TargetStructuralArgument,
    root: &RootDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let geometry = if actual.path.is_empty() && actual.access == StructuralAccess::SharedBorrow {
        declarations
            .iter()
            .find_map(|declaration| match declaration.shape {
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                ) if declaration.id == root.structural_type => Some((0, capacity)),
                _ => None,
            })
    } else {
        structural_shapes::bounded_byte_field_geometry(
            root.structural_type,
            &actual.path,
            declarations,
        )
        .ok()
    };
    let Some((field_offset, field_capacity)) = geometry else {
        return false;
    };
    let is_owner = declarations.iter().any(|declaration| {
        declaration.id == actual.structural_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                )
            )
    });
    (matches!(
        actual.access,
        StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
    ) || is_owner && actual.access == StructuralAccess::WriteOnlyBorrow)
        && matches!(
            (root.access, actual.access),
            (
                StructuralAccess::MutableBorrow,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow,
            ) | (StructuralAccess::Owned, StructuralAccess::SharedBorrow)
                | (
                    StructuralAccess::SharedBorrow,
                    StructuralAccess::SharedBorrow
                )
                | (
                    StructuralAccess::WriteOnlyBorrow,
                    StructuralAccess::WriteOnlyBorrow
                )
        )
        && declarations.iter().any(|declaration| {
            declaration.id == actual.structural_type
                && match &declaration.shape {
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { .. },
                    ) => true,
                    // A bounded view borrows the whole field: the declared
                    // capacity must equal the field's fixed extent.
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { capacity },
                    )
                    | StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedView { capacity, .. },
                    ) => *capacity == field_capacity,
                    _ => false,
                }
        })
        && actual.source_byte_offset == field_offset
        && actual.root_structural_type == root.structural_type
        && actual.indices.is_empty()
        && actual.fixed_array_length.is_none()
        && actual.element_stride.is_none()
}

/// A stored shared view field lends its own two-word descriptor to a shared
/// view formal: the argument addresses the field inside its original root.
pub(super) fn matches_stored_view_field(
    actual: &TargetStructuralArgument,
    root: &RootDeclaration,
    declarations: &[StructuralTypeDeclaration],
) -> bool {
    let Some((StructuralPathSegment::Field(identity), prefix)) = actual.path.split_last() else {
        return false;
    };
    let shared_field = terminal_semantics::runtime_structural_path_tip(
        declarations.iter(),
        root.structural_type,
        prefix,
    )
    .is_some_and(|parent| match &parent.shape {
        StructuralTypeShape::Record { fields } => fields.iter().any(|field| {
            field.identity == *identity
                && !field.relevance.is_erased()
                && field.field_type.byte_sequence_carrier()
                    == Some(terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: Some(StructuralAccess::SharedBorrow),
                    })
        }),
        _ => false,
    });
    let shared_formal = declarations.iter().any(|declaration| {
        declaration.id == actual.structural_type
            && matches!(
                declaration.shape,
                StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView {
                        access: None | Some(StructuralAccess::SharedBorrow),
                    }
                )
            )
    });
    shared_field
        && shared_formal
        && actual.access == StructuralAccess::SharedBorrow
        && matches!(
            root.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        )
        && structural_shapes::borrowed_view_field_offset(
            root.structural_type,
            &actual.path,
            declarations,
        )
        .ok()
            == Some(actual.source_byte_offset)
        && actual.fixed_array_length.is_none()
        && actual.element_stride.is_none()
}

pub(super) fn matches_argument_identity(
    actual: &TargetStructuralArgument,
    semantic: &StructuralArgument,
    declared: &StructuralParameterDeclaration,
) -> bool {
    actual.place == semantic.place
        && actual.path == semantic.path
        && actual.access == semantic.access
        && actual.access == declared.access
        && actual.structural_type == declared.structural_type
}

/// A static element path — `Field` and `FixedIndex` segments only —
/// projects to its leaf type, the leaf's own reconstructed storage shape,
/// and its byte offset inside the referent; any other segment kind has no
/// static projection.
pub(super) fn static_element_projection(
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    declarations: &[StructuralTypeDeclaration],
) -> Option<(StructuralTypeId, ValueShape, u32)> {
    if path.iter().any(|segment| {
        !matches!(
            segment,
            StructuralPathSegment::Field(_) | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return None;
    }
    let (element_type, element_offset) =
        structural_shapes::project_static_path(root, path, declarations).ok()?;
    let element_shape = structural_shapes::reconstruct(element_type, declarations).ok()?;
    Some((element_type, element_shape, element_offset))
}

/// A one-register placement occupying exactly the declared shape's byte
/// extent — the scalar lane's sole-occupancy proof.
pub(super) fn single_register_sized(locations: &[ValueLocation], byte_size: u32) -> bool {
    matches!(
        locations,
        [ValueLocation::Register {
            value_byte_offset: 0,
            byte_size: placed_size,
            ..
        }] if *placed_size == byte_size
    )
}

/// The byte size a one-slot offset-zero placement carries — register and
/// stack occupancy name the same placed word; any other occupancy has none
/// to compare.
pub(super) fn placed_word_byte_size(locations: &[ValueLocation]) -> Option<u32> {
    match locations {
        [
            ValueLocation::Register {
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ]
        | [
            ValueLocation::Stack {
                value_byte_offset: 0,
                byte_size,
                ..
            },
        ] => Some(*byte_size),
        _ => None,
    }
}
