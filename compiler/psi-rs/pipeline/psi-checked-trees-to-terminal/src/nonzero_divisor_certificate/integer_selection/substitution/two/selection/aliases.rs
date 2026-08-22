//! Producer-local fixed two-equality alias eligibility.

use psi_core::ScalarTerm;

pub(super) fn outer(
    goal_left: &ScalarTerm,
    goal_right: &ScalarTerm,
    old: &ScalarTerm,
    middle_alias: &ScalarTerm,
) -> Option<usize> {
    let endpoint = if old == goal_left {
        0
    } else if old == goal_right {
        1
    } else {
        return None;
    };
    (matches!(old, ScalarTerm::Value { .. })
        && matches!(middle_alias, ScalarTerm::Value { .. })
        && old != middle_alias
        && old.scalar_type() == middle_alias.scalar_type())
    .then_some(endpoint)
}

pub(super) fn inner<'a>(
    old: &ScalarTerm,
    middle_alias: &ScalarTerm,
    inner_left: &'a ScalarTerm,
    inner_right: &'a ScalarTerm,
) -> Option<&'a ScalarTerm> {
    let target_alias = if inner_left == middle_alias {
        inner_right
    } else if inner_right == middle_alias {
        inner_left
    } else {
        return None;
    };
    (matches!(target_alias, ScalarTerm::Value { .. })
        && target_alias != old
        && target_alias != middle_alias
        && target_alias.scalar_type() == old.scalar_type())
    .then_some(target_alias)
}
