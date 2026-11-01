//! Borrowed views and byte-sequence carriers: which borrow shells present as a
//! slice or stored reference, and the carrier each byte view realizes as.

use super::*;

/// A borrowed `[T]` element view: one reference shell over a non-byte slice.
/// Fixed arrays and vectors own contiguous storage and a slice borrows it,
/// carrying its own extent as a stored runtime length rather than a declared
/// constant, so the view's shape names only its element. Arithmetic-domain
/// and other constraint shells qualify the carrier without changing it. The
/// byte element keeps its established `ByteSequence` carrier, whose length,
/// read and subslice obligations are already reconstructed. The shell's
/// access is reported, not required: a shared `&[T]` and an exclusive
/// `&mut [T]` view share this carrier, and each consumer admits the loan
/// its own custody needs.
pub(crate) fn element_view_shell(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<validation::SliceViewShell> {
    validation::slice_view_shell(program, type_reference, substitutions)
        .filter(|view| program.primitive_type_reference(view.element) != Some(PrimitiveType::U8))
}

/// The element of a borrowed `[T]` element view under any access.
pub(crate) fn borrowed_slice_view_element(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
    substitutions: &[(SymbolHandle, TypeReferenceHandle)],
) -> Option<TypeReferenceHandle> {
    element_view_shell(program, type_reference, substitutions).map(|view| view.element)
}

/// A `&[T]` declared result type, either view family. The byte element's
/// `BorrowedView` carrier and the non-byte element's identity share the same
/// reference-peeling shape. This shared-only query serves immutable view routes.
pub(crate) fn borrowed_slice_view(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    structural_access_for_type_reference(program, type_reference)
        == Some(CheckedStructuralAccess::SharedBorrow)
        && slice_view_carrier(program, type_reference)
}

/// A readable borrowed slice result transports its descriptor and preserves
/// the source access. Shared custody copies the descriptor; mutable custody is
/// affine. A write-only result keeps its separate, unsupported result contract.
pub(crate) fn borrowed_slice_result(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    validation::borrowed_slice_view_result(program, type_reference)
}

/// A `&[T]` or `&[u8]` descriptor carrier under either loan access; the
/// reference shell's access is the loan's, not the carrier's.
pub(crate) fn slice_view_carrier(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    borrowed_slice_view_element(program, type_reference, &[]).is_some()
        || matches!(
            byte_sequence_carrier(program, type_reference, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
        )
}

/// A shared stored-reference result, independent of its referent family.
/// The referent's storage
/// lives in the caller's frame, so the owned-contents custody clause does not
/// apply. `&mut`/`&write` named results stay with their own custody families.
pub(crate) fn borrowed_named_view(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> bool {
    validation::reference_result_custody::stored_loan_parts(program, type_reference)
        .is_some_and(|(_, access)| access == CheckedStructuralAccess::SharedBorrow)
}

pub(crate) use validation::byte_sequence_carrier;
