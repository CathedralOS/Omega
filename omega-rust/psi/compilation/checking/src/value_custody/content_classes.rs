//! Classifying every type reference's ownership and content class once.
//!
//! The six content predicates lowering used to re-ask — plain owned contents
//! under numeric and under represented constraints, finite linear owned
//! contents, owned-or-shared-view record fields, closed primitive arrays, and
//! bounded byte buffer capacity — are each a pure function of the frozen type
//! table. They are not cheap: four of them run
//! [`crate::value_custody::storage_contents`]'s recursive `ContentType`
//! resolution, which walks each data definition's members, peels constraint
//! shells, and consults the declared-domain intervals and scalar field
//! domain predicates in [`crate::proof_contracts`]. That reach is also why
//! the classification stays here rather than moving to `typed-trees`: the
//! judgment is checking's, not the type table's.
//!
//! This owner answers all six for the whole table at once and hands the rows
//! to `CheckFacts`, so lowering reads a row instead of re-entering checking.

use checked_trees::{ContentClassFacts, ContentClassRow};
use typed_trees::TypedTrees;
use typed_trees::types::TypeReferenceHandle;

/// Classify every type reference the table names.
///
/// The walk is over [`typed_trees::types::TypeReferenceTable::type_reference_rows`],
/// not over checking's own traversal: the row set must not depend on which
/// references checking happened to visit, or a reference lowering asks about
/// would read a hole. Row 0 is the classification of the invalid handle, so
/// an index the table does not name reads the same answer the classifier
/// gives a reference that names nothing.
pub fn classify_program_content_classes(program: &TypedTrees) -> ContentClassFacts {
    let table = &program.type_reference_table;
    let highest = table
        .type_reference_rows()
        .map(|(handle, _)| handle.arena_index())
        .max()
        .unwrap_or(0);
    let mut rows =
        vec![content_classes(program, TypeReferenceHandle::invalid()); highest as usize + 1];
    for (handle, _) in table.type_reference_rows() {
        rows[handle.arena_index() as usize] = content_classes(program, handle);
    }
    ContentClassFacts::with_rows(rows)
}

/// One type reference's content classification.
fn content_classes(program: &TypedTrees, reference: TypeReferenceHandle) -> ContentClassRow {
    ContentClassRow {
        plain_owned_with_numeric_constraints:
            crate::has_plain_owned_contents_with_numeric_constraints(program, reference),
        plain_owned_with_represented_constraints:
            crate::has_plain_owned_contents_with_represented_constraints(program, reference),
        linear_owned_contents: crate::has_linear_owned_contents(program, reference),
        owned_or_shared_view_fields: crate::has_owned_or_shared_view_fields(program, reference),
        closed_primitive_array: crate::is_closed_primitive_array_type(program, reference),
        bounded_byte_capacity: crate::bounded_byte_buffer_capacity(program, reference),
    }
}
