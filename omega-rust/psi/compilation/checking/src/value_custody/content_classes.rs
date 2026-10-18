//! Classifying every type reference's ownership and content class once.
//!
//! The content predicates lowering consumes — plain owned contents
//! under numeric and under represented constraints, finite linear owned
//! contents, borrow-free storage, owned-or-shared-view record fields, closed
//! primitive arrays, and bounded byte buffer capacity — are each a pure
//! function of the frozen type table. Several run
//! [`crate::value_custody::storage_contents`]'s recursive `ContentType`
//! resolution, which walks each data definition's members, peels constraint
//! shells, and consults the declared-domain intervals and scalar field
//! domain predicates in [`crate::proof_contracts`]. That reach is also why
//! the classification stays here rather than moving to `typed-trees`: the
//! judgment is checking's, not the type table's.
//!
//! This owner answers them for the whole table at once and hands the rows
//! to `CheckFacts`, so lowering reads a row instead of re-entering checking.

use checked_trees::{ContentClassFacts, ContentClassRow};
use typed_trees::types::TypeReferenceHandle;

/// Classify every type reference the table names.
///
/// The walk is over [`typed_trees::types::TypeReferenceTable::type_reference_rows`],
/// not over checking's own traversal: the row set must not depend on which
/// references checking happened to visit, or a reference lowering asks about
/// would read a hole. Row 0 is the classification of the invalid handle, so
/// an index the table does not name reads the same answer the classifier
/// gives a reference that names nothing.
pub fn classify_program_content_classes(program: &dyn crate::CheckingInput) -> ContentClassFacts {
    crate::classify_type_references(program, content_classes, ContentClassFacts::with_rows)
}

/// One type reference's content classification.
fn content_classes(
    program: &dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> ContentClassRow {
    ContentClassRow {
        plain_owned_with_numeric_constraints: crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::NumericOwned,
        ),
        plain_owned_with_represented_constraints: crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::RepresentedOwned,
        ),
        stored_value_contents: crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::StoredValue,
        ),
        linear_owned_contents: crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::LinearOwned,
        ),
        borrow_free_contents: crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::BorrowFree,
        ),
        owned_or_shared_view_fields: crate::has_owned_or_shared_view_fields(program, reference),
        owned_or_shared_view_contents: crate::has_contents(
            program,
            reference,
            &[],
            crate::value_custody::storage_contents::ContentsRequirement::OwnedOrSharedView,
        ),
        closed_primitive_array: crate::is_closed_primitive_array_type(program, reference),
        bounded_byte_capacity: crate::bounded_byte_buffer_capacity(program, reference),
    }
}
