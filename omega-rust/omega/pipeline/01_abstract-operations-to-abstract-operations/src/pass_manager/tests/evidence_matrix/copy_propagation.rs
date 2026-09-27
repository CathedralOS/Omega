//! `Optimization::CopyPropagation` evidence matrix legs.

use super::super::{VerifiedPsiOptimizationUnit, verified_merge_parameter_unit};
use super::{
    SelectionMatrix, assert_disabled_leg, assert_empty_selection_leg,
    assert_full_entrance_leg, assert_measured_budget_leg, assert_negative_leg,
};
use crate::rules::RedundantBlockParameterRule;
use optimization_core::{Optimization, OptimizationRuleIdentity};

fn expected_rule() -> OptimizationRuleIdentity {
    RedundantBlockParameterRule::contract().identity()
}

fn positive() -> VerifiedPsiOptimizationUnit {
    verified_merge_parameter_unit(true)
}

const CASE: SelectionMatrix = SelectionMatrix {
    selection: Optimization::CopyPropagation,
    expected_rule,
    expected_commits: 1,
    positive,
    sibling: Optimization::DeadPureScalarElimination,
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

#[test]
fn measured_budget_admits_exact_usage_and_refuses_one_less() {
    assert_measured_budget_leg(&CASE);
}
