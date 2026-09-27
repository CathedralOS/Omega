//! `Optimization::GlobalValueNumbering` evidence matrix legs.

use super::super::verified_compatible_policy_cse_unit;
use super::{
    SelectionMatrix, assert_disabled_leg, assert_empty_selection_leg,
    assert_full_entrance_leg, assert_negative_leg,
};
use crate::rules::SameBlockProofCertifiedCompatiblePolicyScalarCseRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    SameBlockProofCertifiedCompatiblePolicyScalarCseRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::GlobalValueNumbering,
    expected_rule,
    // The proof-certified `right + left` exact add repeats the earlier
    // `left + right` wrapping add under the compatible-policy lattice: one
    // substitution, one commit.
    expected_commits: 1,
    positive: verified_compatible_policy_cse_unit,
    sibling: Optimization::CopyPropagation,
};

#[test]
fn full_entrance_optimizes_and_publishes() {
    assert_full_entrance_leg(&CASE);
}

#[test]
fn disabled_sibling_selection_leaves_the_unit_untouched() {
    assert_disabled_leg(&CASE);
}

#[test]
fn empty_selection_leaves_the_unit_untouched() {
    assert_empty_selection_leg(&CASE);
}

#[test]
fn negative_leaves_an_empty_unit_untouched() {
    assert_negative_leg(&CASE);
}
