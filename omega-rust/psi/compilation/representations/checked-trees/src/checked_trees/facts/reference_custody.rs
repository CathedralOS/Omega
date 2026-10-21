//! Borrow-custody classification of every type reference, recorded once.
//!
//! A reference carrier's custody — which loan leaf it stores, which referent
//! that leaf names, and whether its lifetime obligation ends once — is a
//! question about the frozen type table alone. Checking answers it for every
//! type reference while it validates the program; lowering used to re-enter
//! `validation` and ask the same questions again, once per call site, which
//! put the classifier's admitted shapes behind a checking-stage entry point
//! that lowering has no business calling.
//!
//! These rows are that answer, recorded. The table is dense by
//! `TypeReferenceHandle::arena_index` because the type reference *is* the
//! key: a map keyed by a compiler-owned handle would only reproduce the
//! arena's own addressing with a hash. Row 0 is the invalid handle's row,
//! which is also the answer for any handle the table does not name, so a
//! missing row reads as the absence state rather than a wrong classification.

use crate::CheckedStructuralAccess;
use language_semantics::Multiplicity;
use typed_trees::types::TypeReferenceHandle;

/// One type reference's borrow custody. Every handle field is ZII: the zero
/// handle is the absence of that leaf family, never a leaf naming type zero.
/// That encoding rests on the type table's own invariant that a `Reference`
/// node names a live referee, so no admitted leaf family can carry the zero
/// handle as its referent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceCustodyRow {
    /// The primitive referent beneath an exclusive stored `&mut` leaf — the
    /// whole-return reference-result shape. Invalid when the carrier is not
    /// that shape.
    pub primitive_loan_referent: TypeReferenceHandle,
    /// The referent of the one stored carrier leaf this type spells, whether
    /// exclusive or shared, with `stored_loan_access` naming which.
    pub stored_loan_referent: TypeReferenceHandle,
    /// Which loan family `stored_loan_referent` names. Meaningful only while
    /// that handle is valid.
    pub stored_loan_access: CheckedStructuralAccess,
    /// The referent beneath any `&mut T` carrier, constraint shells peeled.
    pub mutable_borrow_referent: TypeReferenceHandle,
    /// The named referent beneath a `&'a V` shared-borrowed view.
    pub shared_borrow_referent: TypeReferenceHandle,
    /// Whether closed aggregate construction of this type carries real
    /// reference leaves.
    pub is_reference_record: bool,
    /// Custody of a structural binding that denotes the referent directly.
    pub binding_multiplicity: Multiplicity,
    /// Custody of a materialized carrier of this type.
    pub result_multiplicity: Multiplicity,
}

impl Default for ReferenceCustodyRow {
    /// The invalid handle's classification: no leaf family, no record
    /// carriers, and the multiplicity `TypedTrees::type_multiplicity` gives
    /// a handle it does not name.
    fn default() -> Self {
        Self {
            primitive_loan_referent: TypeReferenceHandle::invalid(),
            stored_loan_referent: TypeReferenceHandle::invalid(),
            stored_loan_access: CheckedStructuralAccess::Owned,
            mutable_borrow_referent: TypeReferenceHandle::invalid(),
            shared_borrow_referent: TypeReferenceHandle::invalid(),
            is_reference_record: false,
            binding_multiplicity: Multiplicity::Affine,
            result_multiplicity: Multiplicity::Affine,
        }
    }
}

impl ReferenceCustodyRow {
    /// The one stored carrier leaf, as `(referent, access)`.
    pub fn stored_loan_parts(&self) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
        self.stored_loan_referent
            .is_valid()
            .then_some((self.stored_loan_referent, self.stored_loan_access))
    }
}

/// Every type reference's custody row, dense by arena index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReferenceCustodyFacts {
    /// Indexed by `TypeReferenceHandle::arena_index`. Index 0 is the invalid
    /// handle, and unoccupied interior indices carry that same row.
    rows: Vec<ReferenceCustodyRow>,
}

impl ReferenceCustodyFacts {
    /// Adopt a classification built over the whole type reference table.
    /// `rows` is positional: `rows[i]` classifies arena index `i`, and
    /// `rows[0]` is the row every unnamed handle reads.
    pub fn with_rows(rows: Vec<ReferenceCustodyRow>) -> Self {
        Self { rows }
    }

    /// `reference`'s custody. A handle the table does not name reads the
    /// invalid handle's row, which denies every leaf family.
    pub fn custody(&self, reference: TypeReferenceHandle) -> ReferenceCustodyRow {
        self.rows
            .get(reference.arena_index() as usize)
            .or_else(|| self.rows.first())
            .copied()
            .unwrap_or_default()
    }

    /// How many arena indices the classification names. Zero before checking
    /// records it.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}
