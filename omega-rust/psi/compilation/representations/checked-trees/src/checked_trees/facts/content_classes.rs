//! Ownership and content classification of every type reference, recorded once.
//!
//! Whether a type's contents are plain owned storage under numeric or
//! represented constraints, whether they are finite linear owned storage,
//! whether a record's fields are each owned or a shared view, whether the
//! type is a closed primitive array, and what capacity a bounded byte buffer
//! declares are all questions about the frozen type table alone. Checking
//! answers them while it validates the same carriers; lowering used to
//! re-enter `validation` and re-run the same recursive contents walk once per
//! call site, which put checking's admitted content shapes behind a
//! checking-stage entry point that lowering has no business calling.
//!
//! These rows are that answer, recorded. The table is dense by
//! `TypeReferenceHandle::arena_index` because the type reference *is* the
//! key: a map keyed by a compiler-owned handle would only reproduce the
//! arena's own addressing with a hash. Row 0 is the invalid handle's row,
//! which is also the answer for any handle the table does not name, so a
//! missing row reads as the classifier's own answer for a reference that
//! names nothing rather than a wrong classification.

use typed_trees::types::TypeReferenceHandle;

/// One type reference's content classification.
///
/// The content predicates are recorded as the `bool` they return, not as a ZII
/// handle: there is no leaf family here whose absence a zero handle could
/// denote. `bounded_byte_capacity` keeps its `Option` for the same
/// reason in the other direction — a `[u8; 0] in D` owner declares a real
/// capacity of zero, so zero cannot double as the absence of a bounded byte
/// buffer and collapsing the two would admit an unbounded carrier as a
/// zero-capacity one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentClassRow {
    /// No-code owned storage that tolerates primitive numeric restrictions.
    pub plain_owned_with_numeric_constraints: bool,
    /// An owned snapshot whose consumers retain its exact represented
    /// constraints, including finite byte owners' byte predicates.
    pub plain_owned_with_represented_constraints: bool,
    /// Finite owned storage a linear selection or call can carry whole,
    /// including nominal-cleanup owners.
    pub linear_owned_contents: bool,
    /// Finite stored values, including all reference descriptor access kinds.
    /// Exact owned and borrowed custody remains with the transport operation.
    pub stored_value_contents: bool,
    /// Finite storage without retained reference descriptors. Linear custody,
    /// cleanup and represented predicates stay with its owner; lending that
    /// storage grants neither copying nor disposal authority.
    pub borrow_free_contents: bool,
    /// A record whose fields are each either plain owned contents or a
    /// shared `&[T]`/`&'a V` view descriptor.
    pub owned_or_shared_view_fields: bool,
    /// Finite recursively owned storage with shared-view descriptor leaves.
    /// Moving storage does not move its referents or end its carried loans.
    pub owned_or_shared_view_contents: bool,
    /// A concrete primitive array type with no qualifications or ownership
    /// authority.
    pub closed_primitive_array: bool,
    /// The declared capacity of a bounded byte buffer (`[u8; N] in D`), or
    /// `None` when the type is not one. A capacity is a count, not a handle,
    /// and zero is a legitimate one.
    pub bounded_byte_capacity: Option<usize>,
}

/// Every type reference's content classification, dense by arena index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentClassFacts {
    /// Indexed by `TypeReferenceHandle::arena_index`. Index 0 is the invalid
    /// handle, and unoccupied interior indices carry that same row.
    rows: Vec<ContentClassRow>,
}

impl ContentClassFacts {
    /// Adopt a classification built over the whole type reference table.
    /// `rows` is positional: `rows[i]` classifies arena index `i`, and
    /// `rows[0]` is the row every unnamed handle reads.
    pub fn with_rows(rows: Vec<ContentClassRow>) -> Self {
        Self { rows }
    }

    /// `reference`'s content classification. A handle the table does not
    /// name reads the invalid handle's row, which denies every class.
    pub fn classes(&self, reference: TypeReferenceHandle) -> ContentClassRow {
        self.rows
            .get(reference.arena_index() as usize)
            .or_else(|| self.rows.first())
            .copied()
            .unwrap_or_default()
    }

    /// How many arena indices the classification names. Zero before checking
    /// records it, which is why the classification pass is unconditional in
    /// `check_program` rather than guarded by a caller's request.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
