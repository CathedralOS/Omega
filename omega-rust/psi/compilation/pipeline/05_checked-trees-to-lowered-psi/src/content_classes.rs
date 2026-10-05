//! Reading the checked content classification of a type.
//!
//! Whether a type's contents are plain owned storage under numeric or
//! represented constraints, whether they are finite linear owned storage,
//! whether a record's fields are each owned or a shared view, whether the
//! type is a closed primitive array, and what capacity a bounded byte buffer
//! declares are answers checking already recorded, one row per type
//! reference, in `CheckFacts::content_classes`. Lowering reads the row; it
//! does not re-enter checking's recursive contents walk, which would both
//! re-run that walk at every call site and put the admitted content shapes
//! behind a second caller that could drift from the rows the checked program
//! was validated against.

use checked_trees::CheckedTrees;
use typed_trees::types::TypeReferenceHandle;

fn classes(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> checked_trees::ContentClassRow {
    checked.facts.content_classes.classes(reference)
}

/// No-code owned storage that tolerates primitive numeric restrictions.
pub(crate) fn plain_owned_with_numeric_constraints(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    classes(checked, reference).plain_owned_with_numeric_constraints
}

/// An owned snapshot whose consumers retain its exact represented
/// constraints, including finite byte owners' byte predicates.
pub(crate) fn plain_owned_with_represented_constraints(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    classes(checked, reference).plain_owned_with_represented_constraints
}

/// Finite owned storage a linear selection or call can carry whole.
pub(crate) fn linear_owned_contents(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    classes(checked, reference).linear_owned_contents
}

/// A record whose fields are each either plain owned contents or a shared
/// `&[T]`/`&'a V` view descriptor.
pub(crate) fn owned_or_shared_view_fields(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    classes(checked, reference).owned_or_shared_view_fields
}

/// A concrete primitive array type with no qualifications or ownership
/// authority.
pub(crate) fn closed_primitive_array(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> bool {
    classes(checked, reference).closed_primitive_array
}

/// The declared capacity of a bounded byte buffer (`[u8; N] in D`), or `None`
/// when the type is not one.
pub(crate) fn bounded_byte_capacity(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<usize> {
    classes(checked, reference).bounded_byte_capacity
}
