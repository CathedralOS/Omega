//! `Optimization::SparseConditionalConstantPropagation` evidence matrix legs.

use super::super::verified_exact_add_unit;
use super::{
    SelectionMatrix, assert_disabled_leg, assert_empty_selection_leg,
    assert_full_entrance_leg, assert_negative_leg,
};
use crate::rules::ExactIntegerAddConstantsRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    ExactIntegerAddConstantsRule::contract().identity()
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::SparseConditionalConstantPropagation,
    expected_rule,
    // The proof-certified `7 + 8` exact add folds to its literal in place.
    expected_commits: 1,
    positive: verified_exact_add_unit,
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
