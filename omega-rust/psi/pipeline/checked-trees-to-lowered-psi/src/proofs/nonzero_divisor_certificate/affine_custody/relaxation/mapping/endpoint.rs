//! Producer-local exact affine-root endpoint custody.

use semantic_vocabulary::{Proposition, ScalarTerm};
use proof_admission::CheckedIntegerAffineForm;

pub(super) struct RootEndpoint<'a> {
    pub(super) bound: &'a ScalarTerm,
    pub(super) root_is_lower: bool,
}

pub(super) fn select<'a>(
    form: &CheckedIntegerAffineForm,
    root_bound: &'a Proposition,
) -> Option<RootEndpoint<'a>> {
    let Proposition::LessOrEqual(left, right) = root_bound else {
        return None;
    };
    if left == form.root() {
        Some(RootEndpoint {
            bound: right,
            root_is_lower: false,
        })
    } else if right == form.root() {
        Some(RootEndpoint {
            bound: left,
            root_is_lower: true,
        })
    } else {
        None
    }
}
