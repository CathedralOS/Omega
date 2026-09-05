//! Producer-local stronger-alias bound eligibility.

use semantic_vocabulary::{ScalarTerm, ScalarType};

pub(super) fn select<'a>(
    root: &ScalarTerm,
    alias: &ScalarTerm,
    bound_left: &'a ScalarTerm,
    bound_right: &'a ScalarTerm,
) -> Option<(&'a ScalarTerm, usize)> {
    let (literal, endpoint) = if bound_left == alias {
        (bound_right, 0)
    } else if bound_right == alias {
        (bound_left, 1)
    } else {
        return None;
    };
    let (integer_type, _) = literal.integer_value()?;
    (root.scalar_type() == ScalarType::Integer(integer_type)).then_some((literal, endpoint))
}
