//! Reading the checked borrow custody of a carrier type.
//!
//! Which loan leaf a reference carrier stores, which referent that leaf
//! names, and whether the carrier's lifetime obligation ends exactly once are
//! answers checking already recorded, one row per type reference, in
//! `CheckFacts::reference_custody`. Lowering reads the row; it does not
//! re-enter checking's classifier, which would put the admitted carrier and
//! completion shapes behind a second caller that could drift from the rows
//! the checked program was validated against.
//!
//! A type reference's row is keyed by its handle. Normal-result origins come
//! from the checked execution plan, captured against current carrier contents.
//! A second syntax-only state classifier would reject valid compositions and
//! cannot authenticate those current origins.

use checked_trees::{CheckedStructuralAccess, CheckedTrees};
use language_semantics::Multiplicity;
use typed_trees::types::TypeReferenceHandle;

fn custody(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> checked_trees::ReferenceCustodyRow {
    checked.facts.reference_custody.custody(reference)
}

/// The referent beneath any `&mut T` carrier, constraint shells peeled.
pub(crate) fn mutable_borrow_parts(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    custody(checked, reference).mutable_borrow_parts()
}

/// Whether closed aggregate construction of this type carries real reference
/// leaves.
pub(crate) fn is_reference_record(checked: &CheckedTrees, reference: TypeReferenceHandle) -> bool {
    custody(checked, reference).is_reference_record
}

/// Custody of a materialized carrier of this type.
pub(crate) fn result_multiplicity(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Multiplicity {
    custody(checked, reference).result_multiplicity
}
