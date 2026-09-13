//! Producer-local checked affine mapping of one retained scalar endpoint.

use semantic_vocabulary::{IntegerValue, ScalarTerm};
use proof_admission::CheckedIntegerAffineForm;

pub(super) fn mapped(form: &CheckedIntegerAffineForm, bound: &ScalarTerm) -> Option<ScalarTerm> {
    let (bound_type, IntegerValue::Signed(bound)) = bound.integer_value()? else {
        return None;
    };
    if bound_type != form.integer_type() {
        return None;
    }
    let mapped = form
        .coefficient()
        .checked_mul(bound)?
        .checked_add(form.offset())?;
    ScalarTerm::integer(form.integer_type(), IntegerValue::Signed(mapped)).ok()
}
