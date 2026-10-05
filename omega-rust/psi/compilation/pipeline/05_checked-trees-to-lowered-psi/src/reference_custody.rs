//! Reading the checked borrow custody of a carrier type, and of a state's
//! reference result.
//!
//! Which loan leaf a reference carrier stores, which referent that leaf
//! names, and whether the carrier's lifetime obligation ends exactly once are
//! answers checking already recorded, one row per type reference, in
//! `CheckFacts::reference_custody`. Which expression produces a state's
//! returned value, and whether that state's completion carries complete
//! reference origins or names an ingress parameter or leaf, are recorded the
//! same way, one row per declared state, in
//! `CheckFacts::state_reference_custody`. Lowering reads the row; it does not
//! re-enter checking's classifier, which would put the admitted carrier and
//! completion shapes behind a second caller that could drift from the rows
//! the checked program was validated against.
//!
//! A type reference's row is keyed by its handle; a state's row is keyed by
//! its symbol, which is the key lowering's own `state_declaration` resolves
//! a state through.

use checked_trees::{CheckedStructuralAccess, CheckedTrees};
use language_semantics::Multiplicity;
use typed_trees::expression::ExpressionHandle;
use typed_trees::types::TypeReferenceHandle;

fn custody(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> checked_trees::ReferenceCustodyRow {
    checked.facts.reference_custody.custody(reference)
}

/// The exclusive primitive stored leaf this carrier spells — the whole-return
/// reference-result shape — as `(referent, access)`.
pub(crate) fn primitive_loan_parts(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    custody(checked, reference).primitive_loan_parts()
}

/// The one stored carrier leaf this type holds, exclusive or shared.
pub(crate) fn stored_loan_parts(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    custody(checked, reference).stored_loan_parts()
}

/// The referent beneath any `&mut T` carrier, constraint shells peeled.
pub(crate) fn mutable_borrow_parts(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    custody(checked, reference).mutable_borrow_parts()
}

/// The named referent beneath a `&'a V` shared-borrowed view.
pub(crate) fn shared_borrow_parts(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Option<(TypeReferenceHandle, CheckedStructuralAccess)> {
    custody(checked, reference).shared_borrow_parts()
}

/// Whether closed aggregate construction of this type carries real reference
/// leaves.
pub(crate) fn is_reference_record(checked: &CheckedTrees, reference: TypeReferenceHandle) -> bool {
    custody(checked, reference).is_reference_record
}

/// Custody of a structural binding that denotes the referent directly.
pub(crate) fn binding_multiplicity(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Multiplicity {
    custody(checked, reference).binding_multiplicity
}

/// Custody of a materialized carrier of this type.
pub(crate) fn result_multiplicity(
    checked: &CheckedTrees,
    reference: TypeReferenceHandle,
) -> Multiplicity {
    custody(checked, reference).result_multiplicity
}

fn state_custody(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
) -> checked_trees::StateReferenceCustodyRow {
    checked.facts.state_reference_custody.custody(state.symbol)
}

/// The expression producing this state's returned value: its tail expression
/// statement, or the exit arm of a terminal return transition. Absent when
/// the tail statement is neither.
pub(crate) fn tail_return_expression(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
) -> Option<ExpressionHandle> {
    let expression = state_custody(checked, state).tail_return_expression;
    expression.is_valid().then_some(expression)
}

/// Whether every normal completion of this state contributes an exact
/// caller-relative reference origin, so its returned-leaf roster is complete.
pub(crate) fn returns_complete_reference_origins(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
) -> bool {
    state_custody(checked, state).returns_complete_reference_origins
}
